//! Read-only HTTP adapter with embedded Atlas assets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

use axum::Json;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::uri::Authority;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Semaphore};

use crate::format::{DB_PAGE_SIZE, SlottedPage};
use crate::inspection::{CancelToken, DiagnosticRecord, GraphView, QueryError, ResourcePolicy};
use crate::model::{FileId, Oid, PageId, SectorId, SlotId, Vfid, VolId, Vpid};
use crate::projection::{
    CoverageProjection, DeepPageProjection, DiagnosticProjection, OosChainProjection,
    PageProjection, SCHEMA_NAME, SCHEMA_VERSION, SlotProjection, SnapshotProjection,
    coverage_projection, deep_page_projection, diagnostic_projection, file_header_projection,
    oos_chain_projection, outcome_name, page_projection, sector_projection, slot_projection,
    snapshot_id_hex, summary_projection, volume_projection,
};

const MAX_URI_BYTES: usize = 8192;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_HEADER_FIELDS: usize = 64;
const MAX_JSON_BYTES: usize = 64 * 1024;
const MAX_CONCURRENT_REQUESTS: usize = 32;
const DEFAULT_COLLECTION_LIMIT: usize = 100;
const MAX_COLLECTION_LIMIT: usize = 512;
const DEFAULT_SECTOR_COLLECTION_LIMIT: usize = 24;
const MAX_SECTOR_COLLECTION_LIMIT: usize = 64;

#[derive(Clone, Debug)]
pub struct ServeOptions {
    pub listen: SocketAddr,
    pub policy: ResourcePolicy,
}

#[derive(Clone)]
struct WebState {
    session: Arc<RwLock<LiveSession>>,
    enrichment: Arc<Mutex<()>>,
    policy: ResourcePolicy,
    cursor_key: Arc<[u8; 32]>,
    authority: Option<Arc<str>>,
    semaphore: Arc<Semaphore>,
}

struct LiveSession {
    views: BTreeMap<u64, GraphView>,
    jobs: BTreeSet<u64>,
    latest: u64,
}

#[derive(Debug)]
pub enum ServeError {
    RemoteWildcardRequired,
    Io(io::Error),
    Runtime(String),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RemoteWildcardRequired => formatter
                .write_str("remote HTTP requires an explicit --listen 0.0.0.0:PORT listener"),
            Self::Io(error) => write!(formatter, "web I/O failed: {error}"),
            Self::Runtime(message) => write!(formatter, "web runtime failed: {message}"),
        }
    }
}

impl std::error::Error for ServeError {}

impl From<io::Error> for ServeError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn serve(view: GraphView, options: ServeOptions) -> Result<(), ServeError> {
    validate_listener(&options)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(ServeError::Io)?;
    runtime.block_on(serve_async(view, options))
}

async fn serve_async(view: GraphView, options: ServeOptions) -> Result<(), ServeError> {
    let listener = TcpListener::bind(options.listen)
        .await
        .map_err(ServeError::Io)?;
    let local = listener.local_addr().map_err(ServeError::Io)?;
    let cursor_key = Arc::new(generate_cursor_key()?);
    let initial_revision = view.overview().revision.get();
    let mut views = BTreeMap::new();
    views.insert(initial_revision, view);
    let state = WebState {
        session: Arc::new(RwLock::new(LiveSession {
            views,
            jobs: BTreeSet::new(),
            latest: initial_revision,
        })),
        enrichment: Arc::new(Mutex::new(())),
        policy: options.policy,
        cursor_key,
        authority: (!options.listen.ip().is_unspecified()).then(|| Arc::from(local.to_string())),
        semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
    };
    let router = build_router(state);
    print_listener_urls(local);
    if !options.listen.ip().is_loopback() {
        eprintln!(
            "WARNING: unauthenticated plain HTTP is listening on all interfaces. Anyone who can reach this port can inspect metadata and request enrichment."
        );
    }
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| ServeError::Runtime(error.to_string()))
}

fn print_listener_urls(local: SocketAddr) {
    let interface_ips = if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .filter(if_addrs::Interface::is_oper_up)
                .map(|interface| interface.ip())
                .collect()
        })
        .unwrap_or_default();

    eprintln!("Bound to {local}");
    eprintln!("Available at (non-exhaustive list):");
    for url in listener_urls(local, interface_ips) {
        eprintln!("    {url}");
    }
}

fn listener_urls(local: SocketAddr, interface_ips: Vec<IpAddr>) -> Vec<String> {
    let mut addresses = BTreeSet::new();
    if local.ip().is_unspecified() {
        let loopback = match local {
            SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
            SocketAddr::V6(_) => IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        };
        addresses.insert(loopback);
        addresses.extend(
            interface_ips
                .into_iter()
                .filter(|ip| ip.is_ipv4() == local.is_ipv4() && is_publishable_listener_ip(*ip)),
        );
    } else {
        addresses.insert(local.ip());
    }

    addresses
        .into_iter()
        .map(|ip| format!("http://{}", SocketAddr::new(ip, local.port())))
        .collect()
}

fn is_publishable_listener_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => !ip.is_unspecified() && !ip.is_multicast() && ip != Ipv4Addr::BROADCAST,
        IpAddr::V6(ip) => !ip.is_unspecified() && !ip.is_multicast() && !ip.is_unicast_link_local(),
    }
}

fn build_router(state: WebState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.css", get(css))
        .route("/app.js", get(javascript))
        .route("/s/{snapshot}/r/{revision}/volume/{vol}", get(index))
        .route("/s/{snapshot}/r/{revision}/file/{vol}/{file}", get(index))
        .route(
            "/s/{snapshot}/r/{revision}/sector/{vol}/{sector}",
            get(index),
        )
        .route("/s/{snapshot}/r/{revision}/page/{vol}/{page}", get(index))
        .route(
            "/s/{snapshot}/r/{revision}/slot/{vol}/{page}/{slot}",
            get(index),
        )
        .route(
            "/s/{snapshot}/r/{revision}/oos/{vol}/{page}/{slot}",
            get(index),
        )
        .route("/api/v1/session", get(session))
        .route("/api/v1/licenses", get(licenses))
        .route("/api/v1/s/{snapshot}/r/{revision}/overview", get(overview))
        .route("/api/v1/s/{snapshot}/r/{revision}/volumes", get(volumes))
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/sectors/{vol}",
            get(sectors),
        )
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/relationships",
            get(relationships),
        )
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/diagnostics",
            get(diagnostics),
        )
        .route("/api/v1/s/{snapshot}/r/{revision}/coverage", get(coverage))
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/file/{vol}/{file}",
            get(file),
        )
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/sector/{vol}/{sector}",
            get(sector),
        )
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/page/{vol}/{page}",
            get(page),
        )
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/slot/{vol}/{page}/{slot}",
            get(slot),
        )
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/oos/{vol}/{page}/{slot}",
            get(oos),
        )
        .route(
            "/api/v1/s/{snapshot}/r/{revision}/enrichments",
            post(enrich),
        )
        .route("/api/v1/jobs/{job}", get(job))
        .fallback(not_found)
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(MAX_JSON_BYTES))
        .layer(from_fn_with_state(state, request_guard))
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {
                result = tokio::signal::ctrl_c() => { let _ = result; }
                signal = terminate.recv() => { let _ = signal; }
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

async fn request_guard(State(state): State<WebState>, request: Request, next: Next) -> Response {
    let mut response = match guard(&state, &request) {
        Ok(()) => match state.semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                let response = next.run(request).await;
                drop(permit);
                response
            }
            Err(_) => error_response(StatusCode::TOO_MANY_REQUESTS, "resource-admission-refused"),
        },
        Err(error) => error_response(error.status, error.code),
    };
    apply_security_headers(response.headers_mut());
    response
}

#[derive(Clone, Copy)]
struct GuardError {
    status: StatusCode,
    code: &'static str,
}

fn guard(state: &WebState, request: &Request) -> Result<(), GuardError> {
    if request.uri().to_string().len() > MAX_URI_BYTES {
        return Err(GuardError {
            status: StatusCode::URI_TOO_LONG,
            code: "uri-too-long",
        });
    }
    let header_bytes = request
        .headers()
        .iter()
        .try_fold(0_usize, |total, (name, value)| {
            total
                .checked_add(name.as_str().len())?
                .checked_add(value.as_bytes().len())
        });
    if request.headers().len() > MAX_HEADER_FIELDS
        || header_bytes.is_none_or(|bytes| bytes > MAX_HEADER_BYTES)
    {
        return Err(GuardError {
            status: StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            code: "headers-too-large",
        });
    }
    let mut hosts = request.headers().get_all(HOST).iter();
    let host = hosts.next().and_then(|value| value.to_str().ok());
    let valid_authority = host.is_some_and(|value| value.parse::<Authority>().is_ok());
    let expected_authority_matches = state
        .authority
        .as_deref()
        .is_none_or(|authority| host == Some(authority));
    if !valid_authority || !expected_authority_matches || hosts.next().is_some() {
        return Err(GuardError {
            status: StatusCode::MISDIRECTED_REQUEST,
            code: "invalid-host",
        });
    }
    let enrichment_post = request.uri().path().ends_with("/enrichments");
    let method_allowed = if enrichment_post {
        request.method() == axum::http::Method::POST
    } else {
        matches!(
            *request.method(),
            axum::http::Method::GET | axum::http::Method::HEAD
        )
    };
    if !method_allowed {
        return Err(GuardError {
            status: StatusCode::METHOD_NOT_ALLOWED,
            code: "method-not-allowed",
        });
    }
    if request.method() == axum::http::Method::POST {
        let content_type = request.headers().get_all(CONTENT_TYPE);
        let mut content_types = content_type.iter();
        if content_types.next().map(HeaderValue::as_bytes) != Some(b"application/json")
            || content_types.next().is_some()
        {
            return Err(GuardError {
                status: StatusCode::BAD_REQUEST,
                code: "json-content-type-required",
            });
        }
        let origins = request.headers().get_all(ORIGIN);
        let mut values = origins.iter();
        let supplied = values.next().and_then(|value| value.to_str().ok());
        let expected_origin = host.map(|authority| format!("http://{authority}"));
        if supplied != expected_origin.as_deref() || values.next().is_some() {
            return Err(GuardError {
                status: StatusCode::FORBIDDEN,
                code: "origin-rejected",
            });
        }
        let fetch_site = HeaderName::from_static("sec-fetch-site");
        let sites = request.headers().get_all(fetch_site);
        let mut values = sites.iter();
        if values.next().is_some_and(|value| value == "cross-site") || values.next().is_some() {
            return Err(GuardError {
                status: StatusCode::FORBIDDEN,
                code: "fetch-context-rejected",
            });
        }
    }
    Ok(())
}

fn apply_security_headers(headers: &mut axum::http::HeaderMap) {
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=(), payment=(), usb=()",
        ),
    ] {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn css() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!("{APP_CSS}{DISTRIBUTION_CSS}"),
    )
}

async fn javascript() -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        APP_JS,
    )
}

async fn not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, "resource-not-found")
}

#[derive(Serialize)]
struct LicenseDocument {
    schema: &'static str,
    schema_version: u32,
    notice: &'static str,
}

async fn licenses() -> Json<LicenseDocument> {
    Json(LicenseDocument {
        schema: "volmap.licenses",
        schema_version: 1,
        notice: crate::notices::THIRD_PARTY_NOTICES,
    })
}

#[derive(Serialize)]
struct ApiEnvelope<T: Serialize> {
    schema: &'static str,
    schema_version: u32,
    document_type: &'static str,
    snapshot: SnapshotProjection,
    outcome: &'static str,
    coverage: Vec<CoverageProjection>,
    diagnostics: Vec<DiagnosticProjection>,
    data: T,
}

async fn session(State(state): State<WebState>) -> Response {
    let view = match latest_view(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    Json(api_envelope(
        &overview,
        SessionProjection {
            access: "unauthenticated-http",
        },
    ))
    .into_response()
}

#[derive(Serialize)]
struct SessionProjection {
    access: &'static str,
}

fn latest_view(state: &WebState) -> Result<GraphView, GuardError> {
    let session = state.session.read().map_err(|_| GuardError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "session-unavailable",
    })?;
    session
        .views
        .get(&session.latest)
        .cloned()
        .ok_or(GuardError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "session-unavailable",
        })
}

fn revision_view(state: &WebState, snapshot: &str, revision: u64) -> Result<GraphView, GuardError> {
    let session = state.session.read().map_err(|_| GuardError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "session-unavailable",
    })?;
    let view = session.views.get(&revision).cloned().ok_or(GuardError {
        status: StatusCode::NOT_FOUND,
        code: "revision-not-found",
    })?;
    if !matches_revision(&view.overview(), snapshot, revision) {
        return Err(GuardError {
            status: StatusCode::NOT_FOUND,
            code: "revision-not-found",
        });
    }
    Ok(view)
}

fn projected_overview(state: &WebState, view: &GraphView) -> crate::inspection::OverviewView {
    let mut overview = view.overview();
    let terminally_invalidated = state.session.read().map_or(true, |session| {
        session.views.get(&session.latest).is_some_and(|latest| {
            latest.overview().validity == crate::model::SnapshotValidity::Invalidated
        })
    });
    apply_terminal_invalidation(&mut overview, terminally_invalidated);
    overview
}

fn apply_terminal_invalidation(
    overview: &mut crate::inspection::OverviewView,
    terminally_invalidated: bool,
) {
    if terminally_invalidated && overview.validity != crate::model::SnapshotValidity::Invalidated {
        overview.validity = crate::model::SnapshotValidity::Invalidated;
        overview.outcome = crate::diagnostics::InspectionOutcome::Fatal;
        if !overview
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "snapshot.modified")
        {
            overview.diagnostics.push(DiagnosticRecord {
                code: "snapshot.modified",
                severity: "fatal",
                message: "The source changed after this revision was published; retained facts are diagnostic-only.",
                subject: "snapshot".to_owned(),
                rule: "snapshot.file_stamp.stable",
            });
        }
    }
}

async fn overview(
    State(state): State<WebState>,
    Path((snapshot, revision)): Path<(String, u64)>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let canonical = projected_overview(&state, &view);
    Json(api_envelope(&canonical, summary_projection(&canonical))).into_response()
}

async fn volumes(
    State(state): State<WebState>,
    Path((snapshot, revision)): Path<(String, u64)>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    let data = view.volumes().into_iter().map(volume_projection).collect();
    collection_response(&state, &overview, "volumes", query, data)
}

async fn sectors(
    State(state): State<WebState>,
    Path((snapshot, revision, vol)): Path<(String, u64, i16)>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let Some(vol_id) = VolId::new(vol).ok() else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let Some(volume) = view
        .volumes()
        .into_iter()
        .find(|volume| volume.vol_id == vol_id)
    else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let overview = projected_overview(&state, &view);
    let cursor_kind = format!("sectors:{vol}");
    let offset = match query.cursor {
        Some(cursor) => match decode_cursor(&state, &overview, &cursor_kind, &cursor) {
            Some(value) => value,
            None => return error_response(StatusCode::BAD_REQUEST, "invalid-cursor"),
        },
        None => 0,
    };
    let limit = query.limit.unwrap_or(DEFAULT_SECTOR_COLLECTION_LIMIT);
    let Ok(total) = usize::try_from(volume.total_sectors) else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable");
    };
    if limit == 0 || limit > MAX_SECTOR_COLLECTION_LIMIT {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-limit");
    }
    if offset > total {
        return error_response(StatusCode::BAD_REQUEST, "invalid-cursor");
    }
    let Some((start, end)) = sector_collection_window(total, offset, limit) else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-limit");
    };
    let mut items = Vec::with_capacity(end - start);
    for raw_sector in start..end {
        let Ok(raw_sector) = i32::try_from(raw_sector) else {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable");
        };
        let Ok(sector_id) = SectorId::new(raw_sector) else {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable");
        };
        let Ok(sector) = view.sector(vol_id, sector_id) else {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable");
        };
        items.push(sector_projection(sector));
    }
    let next_cursor = if end < total {
        NextCursorProjection::Present {
            value: encode_cursor(&state, &overview, &cursor_kind, end),
        }
    } else {
        NextCursorProjection::End
    };
    Json(api_envelope(
        &overview,
        CollectionProjection { items, next_cursor },
    ))
    .into_response()
}

fn sector_collection_window(total: usize, offset: usize, limit: usize) -> Option<(usize, usize)> {
    if limit == 0 || limit > MAX_SECTOR_COLLECTION_LIMIT || offset > total {
        return None;
    }
    Some((offset, offset.saturating_add(limit).min(total)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct CollectionProjection<T: Serialize> {
    items: Vec<T>,
    next_cursor: NextCursorProjection,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum NextCursorProjection {
    Present { value: String },
    End,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum RelationshipProjection {
    OosChain {
        chain: OosChainProjection,
    },
    OverflowChain {
        chain: crate::projection::OverflowChainProjection,
    },
    RelocationEdge {
        edge: crate::projection::RelocationEdgeProjection,
    },
}

async fn relationships(
    State(state): State<WebState>,
    Path((snapshot, revision)): Path<(String, u64)>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    let mut data = view
        .oos_chains()
        .into_iter()
        .map(|chain| RelationshipProjection::OosChain {
            chain: oos_chain_projection(chain),
        })
        .collect::<Vec<_>>();
    data.extend(view.overflow_chains().into_iter().map(|chain| {
        RelationshipProjection::OverflowChain {
            chain: crate::projection::overflow_chain_projection(chain),
        }
    }));
    data.extend(view.relocation_edges().into_iter().map(|edge| {
        RelationshipProjection::RelocationEdge {
            edge: crate::projection::relocation_edge_projection(edge),
        }
    }));
    collection_response(&state, &overview, "relationships", query, data)
}

async fn diagnostics(
    State(state): State<WebState>,
    Path((snapshot, revision)): Path<(String, u64)>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    let data = overview
        .diagnostics
        .iter()
        .cloned()
        .map(diagnostic_projection)
        .collect();
    collection_response(&state, &overview, "diagnostics", query, data)
}

async fn coverage(
    State(state): State<WebState>,
    Path((snapshot, revision)): Path<(String, u64)>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    let data = overview
        .coverage
        .iter()
        .copied()
        .map(coverage_projection)
        .collect();
    collection_response(&state, &overview, "coverage", query, data)
}

fn collection_response<T: Serialize>(
    state: &WebState,
    overview: &crate::inspection::OverviewView,
    kind: &'static str,
    query: Result<Query<CollectionQuery>, QueryRejection>,
    items: Vec<T>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let limit = query.limit.unwrap_or(DEFAULT_COLLECTION_LIMIT);
    if limit == 0 || limit > MAX_COLLECTION_LIMIT {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-limit");
    }
    let offset = match query.cursor {
        Some(cursor) => match decode_cursor(state, overview, kind, &cursor) {
            Some(value) => value,
            None => return error_response(StatusCode::BAD_REQUEST, "invalid-cursor"),
        },
        None => 0,
    };
    if offset > items.len() {
        return error_response(StatusCode::BAD_REQUEST, "invalid-cursor");
    }
    let total = items.len();
    let end = offset.saturating_add(limit).min(total);
    let page = items.into_iter().skip(offset).take(end - offset).collect();
    let next_cursor = if end < total {
        NextCursorProjection::Present {
            value: encode_cursor(state, overview, kind, end),
        }
    } else {
        NextCursorProjection::End
    };
    Json(api_envelope(
        overview,
        CollectionProjection {
            items: page,
            next_cursor,
        },
    ))
    .into_response()
}

fn encode_cursor(
    state: &WebState,
    overview: &crate::inspection::OverviewView,
    kind: &str,
    offset: usize,
) -> String {
    let payload = u64::try_from(offset).unwrap_or(u64::MAX).to_le_bytes();
    let mac = cursor_mac(state, overview, kind, &payload);
    hex_encode(&payload.into_iter().chain(mac).collect::<Vec<_>>())
}

fn decode_cursor(
    state: &WebState,
    overview: &crate::inspection::OverviewView,
    kind: &str,
    cursor: &str,
) -> Option<usize> {
    let bytes = hex_decode(cursor)?;
    let (payload, supplied_mac) = bytes.split_at_checked(8)?;
    if supplied_mac.len() != 32 {
        return None;
    }
    let expected_mac = cursor_mac(state, overview, kind, payload);
    if !bool::from(supplied_mac.ct_eq(expected_mac.as_slice())) {
        return None;
    }
    let offset = u64::from_le_bytes(payload.try_into().ok()?);
    usize::try_from(offset).ok()
}

fn cursor_mac(
    state: &WebState,
    overview: &crate::inspection::OverviewView,
    kind: &str,
    payload: &[u8],
) -> [u8; 32] {
    let mut key = [0_u8; 64];
    key[..state.cursor_key.len()].copy_from_slice(state.cursor_key.as_ref());
    let mut inner_pad = [0x36_u8; 64];
    let mut outer_pad = [0x5c_u8; 64];
    for ((inner, outer), key_byte) in inner_pad.iter_mut().zip(outer_pad.iter_mut()).zip(key) {
        *inner ^= key_byte;
        *outer ^= key_byte;
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(b"volmap.cursor.v1\0");
    inner.update(kind.as_bytes());
    inner.update([0]);
    inner.update(snapshot_id_hex(overview.snapshot_id).as_bytes());
    inner.update(overview.revision.get().to_le_bytes());
    inner.update(payload);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner);
    outer.finalize().into()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() != 80 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let digits = std::str::from_utf8(digits).ok()?;
            u8::from_str_radix(digits, 16).ok()
        })
        .collect()
}

async fn file(
    State(state): State<WebState>,
    Path((snapshot, revision, vol, file)): Path<(String, u64, i16, i32)>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let Some(vfid) = VolId::new(vol)
        .ok()
        .zip(FileId::new(file).ok())
        .map(|(vol_id, file_id)| Vfid::new(vol_id, file_id))
    else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let header_page = PageId::new(vfid.file_id.get())
        .ok()
        .map(|page_id| Vpid::new(vfid.vol_id, page_id));
    let Some(header) = header_page
        .and_then(|vpid| view.deep_page(vpid))
        .and_then(|deep| deep.file_header)
        .filter(|header| header.vfid() == vfid)
    else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let overview = projected_overview(&state, &view);
    Json(api_envelope(&overview, file_header_projection(header))).into_response()
}

async fn sector(
    State(state): State<WebState>,
    Path((snapshot, revision, vol, sector)): Path<(String, u64, i16, i32)>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    let result = VolId::new(vol)
        .ok()
        .zip(SectorId::new(sector).ok())
        .ok_or(QueryError::EntityNotFound)
        .and_then(|(vol_id, sector_id)| view.sector(vol_id, sector_id));
    match result {
        Ok(value) => Json(api_envelope(&overview, sector_projection(value))).into_response(),
        Err(_) => error_response(StatusCode::NOT_FOUND, "entity-not-found"),
    }
}

async fn page(
    State(state): State<WebState>,
    Path((snapshot, revision, vol, page)): Path<(String, u64, i16, i32)>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    let result = VolId::new(vol)
        .ok()
        .and_then(|vol_id| {
            PageId::new(page)
                .ok()
                .map(|page_id| Vpid::new(vol_id, page_id))
        })
        .ok_or(QueryError::EntityNotFound)
        .and_then(|vpid| view.page(vpid));
    match result {
        Ok(value) => {
            let vpid = value.vpid;
            let deep = view.deep_page(vpid);
            let slotted = deep.as_ref().and_then(|detail| detail.slotted.as_ref());
            let slots = slotted.map_or_else(Vec::new, |slotted| {
                slotted
                    .slots()
                    .iter()
                    .copied()
                    .map(slot_projection)
                    .collect()
            });
            let distribution = slotted.map_or(
                PageDistributionProjection::NotAvailable,
                page_distribution_projection,
            );
            Json(api_envelope(
                &overview,
                PageResourceProjection {
                    page: page_projection(value),
                    deep: deep_page_projection(deep),
                    slots,
                    distribution,
                },
            ))
            .into_response()
        }
        Err(_) => error_response(StatusCode::NOT_FOUND, "entity-not-found"),
    }
}

#[derive(Serialize)]
struct PageResourceProjection {
    page: PageProjection,
    deep: DeepPageProjection,
    slots: Vec<SlotProjection>,
    distribution: PageDistributionProjection,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum PageDistributionProjection {
    NotAvailable,
    Available {
        content_size: u32,
        header: ByteRegionProjection,
        record_extents: Vec<RecordExtentProjection>,
        free_regions: Vec<FreeRegionProjection>,
        slot_directory: ByteRegionProjection,
        slot_entries: Vec<SlotEntryProjection>,
        allocated_record_bytes: u32,
        unoccupied_bytes: u32,
    },
}

#[derive(Clone, Copy, Serialize)]
struct ByteRegionProjection {
    offset: u32,
    length: u32,
}

#[derive(Serialize)]
struct RecordExtentProjection {
    slot_id: u16,
    offset: u32,
    length: u32,
    record_type: &'static str,
}

#[derive(Serialize)]
struct FreeRegionProjection {
    offset: u32,
    length: u32,
    kind: &'static str,
}

#[derive(Serialize)]
struct SlotEntryProjection {
    slot_id: u16,
    offset: u32,
    length: u32,
    state: &'static str,
    record_type: &'static str,
}

fn page_distribution_projection(slotted: &SlottedPage) -> PageDistributionProjection {
    const HEADER_SIZE: u32 = 32;
    const SLOT_ENTRY_SIZE: u32 = 4;

    let content_size = u32::try_from(DB_PAGE_SIZE).expect("DB page size fits u32");
    let slot_directory_length = u32::try_from(slotted.slots().len())
        .expect("validated slot count fits u32")
        * SLOT_ENTRY_SIZE;
    let slot_directory_offset = content_size - slot_directory_length;
    let mut records: Vec<_> = slotted
        .slots()
        .iter()
        .copied()
        .filter(|slot| !slot.is_empty())
        .map(|slot| RecordExtentProjection {
            slot_id: slot.slot_id(),
            offset: u32::from(slot.offset()),
            length: u32::from(slot.length()),
            record_type: slot.record_type().as_str(),
        })
        .collect();
    records.sort_unstable_by_key(|record| (record.offset, record.slot_id));

    let mut free_regions = Vec::new();
    let mut cursor = HEADER_SIZE;
    for record in &records {
        if cursor < record.offset {
            push_free_region(
                &mut free_regions,
                cursor,
                record.offset,
                slot_directory_offset,
                slotted.free_area_offset(),
            );
        }
        cursor = cursor.max(record.offset + record.length);
    }
    if cursor < slot_directory_offset {
        push_free_region(
            &mut free_regions,
            cursor,
            slot_directory_offset,
            slot_directory_offset,
            slotted.free_area_offset(),
        );
    }

    let allocated_record_bytes = records.iter().map(|record| record.length).sum();
    let unoccupied_bytes = free_regions.iter().map(|region| region.length).sum();
    let slot_entries = slotted
        .slots()
        .iter()
        .copied()
        .map(|slot| SlotEntryProjection {
            slot_id: slot.slot_id(),
            offset: content_size - (u32::from(slot.slot_id()) + 1) * SLOT_ENTRY_SIZE,
            length: SLOT_ENTRY_SIZE,
            state: if !slot.is_empty() {
                "allocated"
            } else if matches!(
                slot.record_type(),
                crate::format::RecordType::MarkDeleted
                    | crate::format::RecordType::DeletedWillReuse
            ) {
                "deleted"
            } else {
                "unallocated"
            },
            record_type: slot.record_type().as_str(),
        })
        .collect();

    PageDistributionProjection::Available {
        content_size,
        header: ByteRegionProjection {
            offset: 0,
            length: HEADER_SIZE,
        },
        record_extents: records,
        free_regions,
        slot_directory: ByteRegionProjection {
            offset: slot_directory_offset,
            length: slot_directory_length,
        },
        slot_entries,
        allocated_record_bytes,
        unoccupied_bytes,
    }
}

fn push_free_region(
    regions: &mut Vec<FreeRegionProjection>,
    start: u32,
    end: u32,
    slot_directory_offset: u32,
    free_area_offset: u32,
) {
    if end == slot_directory_offset && start < free_area_offset && free_area_offset < end {
        regions.push(FreeRegionProjection {
            offset: start,
            length: free_area_offset - start,
            kind: "fragmented-free",
        });
        regions.push(FreeRegionProjection {
            offset: free_area_offset,
            length: end - free_area_offset,
            kind: "contiguous-free",
        });
    } else {
        regions.push(FreeRegionProjection {
            offset: start,
            length: end - start,
            kind: if end == slot_directory_offset && start == free_area_offset {
                "contiguous-free"
            } else {
                "fragmented-free"
            },
        });
    }
}

async fn slot(
    State(state): State<WebState>,
    Path((snapshot, revision, vol, page, slot)): Path<(String, u64, i16, i32, i16)>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let Some(oid) = parse_web_oid(vol, page, slot) else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let vpid = Vpid::new(oid.vol_id, oid.page_id);
    let selected = view.deep_page(vpid).and_then(|deep| {
        deep.slotted.and_then(|slotted| {
            usize::try_from(oid.slot_id.get())
                .ok()
                .and_then(|index| slotted.slots().get(index).copied())
        })
    });
    let (Ok(page), Some(selected)) = (view.page(vpid), selected) else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let overview = projected_overview(&state, &view);
    Json(api_envelope(
        &overview,
        SlotResourceProjection {
            page: page_projection(page),
            deep: deep_page_projection(view.deep_page(vpid)),
            selected_slot: slot_projection(selected),
        },
    ))
    .into_response()
}

#[derive(Serialize)]
struct SlotResourceProjection {
    page: PageProjection,
    deep: DeepPageProjection,
    selected_slot: SlotProjection,
}

async fn oos(
    State(state): State<WebState>,
    Path((snapshot, revision, vol, page, slot)): Path<(String, u64, i16, i32, i16)>,
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let Some(oid) = parse_web_oid(vol, page, slot) else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let Some(chain) = view.oos_chain(oid) else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let overview = projected_overview(&state, &view);
    Json(api_envelope(
        &overview,
        OosResourceProjection {
            chain: oos_chain_projection(chain),
        },
    ))
    .into_response()
}

#[derive(Serialize)]
struct OosResourceProjection {
    chain: OosChainProjection,
}

fn parse_web_oid(vol: i16, page: i32, slot: i16) -> Option<Oid> {
    Some(Oid::new(
        VolId::new(vol).ok()?,
        PageId::new(page).ok()?,
        SlotId::new(slot).ok()?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrichmentRequest {
    selector: String,
}

#[derive(Clone, Copy)]
enum EnrichmentTarget {
    Page(Vpid),
    Slot(Oid),
    Oos(Oid),
}

#[derive(Serialize)]
struct EnrichmentProjection {
    job_id: String,
    status: &'static str,
    result_revision: String,
    result: String,
}

async fn enrich(
    State(state): State<WebState>,
    Path((snapshot, revision)): Path<(String, u64)>,
    payload: Result<Json<EnrichmentRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(value) => value,
        Err(error) => {
            let status = if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            };
            return error_response(status, "malformed-request");
        }
    };
    let Ok(_admission) = state.enrichment.try_lock() else {
        return error_response(StatusCode::TOO_MANY_REQUESTS, "resource-admission-refused");
    };
    let base = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let current_revision = match state.session.read() {
        Ok(session) => session.latest,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable"),
    };
    if current_revision != revision {
        return stale_revision_response(revision, current_revision);
    }
    if base.overview().validity == crate::model::SnapshotValidity::Invalidated {
        return invalidated_snapshot_response();
    }
    let Some(target) = parse_enrichment_target(&request.selector) else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-selector");
    };
    let cancel = CancelToken::new();
    let enriched = match target {
        EnrichmentTarget::Page(vpid) => base.enrich_page(vpid, state.policy, &cancel),
        EnrichmentTarget::Slot(oid) => {
            base.enrich_page(Vpid::new(oid.vol_id, oid.page_id), state.policy, &cancel)
        }
        EnrichmentTarget::Oos(oid) => base.enrich_oos(oid, state.policy, &cancel),
    };
    let enriched = match enriched {
        Ok(value) => value,
        Err(crate::inspection::OperationError::ResourceLimit) => {
            return error_response(StatusCode::TOO_MANY_REQUESTS, "resource-admission-refused");
        }
        Err(
            crate::inspection::OperationError::Unsupported
            | crate::inspection::OperationError::Query(_),
        ) => return error_response(StatusCode::NOT_FOUND, "entity-not-found"),
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "enrichment-failed"),
    };
    if let EnrichmentTarget::Slot(oid) = target {
        let slot_exists = enriched
            .deep_page(Vpid::new(oid.vol_id, oid.page_id))
            .and_then(|deep| deep.slotted)
            .is_some_and(|slotted| {
                usize::try_from(oid.slot_id.get())
                    .ok()
                    .is_some_and(|index| index < slotted.slots().len())
            });
        if !slot_exists {
            return error_response(StatusCode::NOT_FOUND, "entity-not-found");
        }
    }
    let overview = enriched.overview();
    let result_revision = overview.revision.get();
    let result = target_result_path(&snapshot, result_revision, target);
    {
        let Ok(mut session) = state.session.write() else {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable");
        };
        if session.latest != revision {
            return stale_revision_response(revision, session.latest);
        }
        session.views.insert(result_revision, enriched);
        session.jobs.insert(result_revision);
        session.latest = result_revision;
    }
    let location = format!("/api/v1/jobs/{result_revision}");
    let Ok(location_header) = HeaderValue::from_str(&location) else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "enrichment-failed");
    };
    let mut response = (
        StatusCode::ACCEPTED,
        Json(api_envelope(
            &overview,
            EnrichmentProjection {
                job_id: result_revision.to_string(),
                status: "completed",
                result_revision: result_revision.to_string(),
                result,
            },
        )),
    )
        .into_response();
    response
        .headers_mut()
        .insert(axum::http::header::LOCATION, location_header);
    response
}

async fn job(State(state): State<WebState>, Path(job): Path<u64>) -> Response {
    let view = {
        let Ok(session) = state.session.read() else {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable");
        };
        if session.jobs.contains(&job) {
            session.views.get(&job).cloned()
        } else {
            None
        }
    };
    let Some(view) = view else {
        return error_response(StatusCode::NOT_FOUND, "job-not-found");
    };
    let overview = projected_overview(&state, &view);
    Json(api_envelope(
        &overview,
        EnrichmentProjection {
            job_id: job.to_string(),
            status: "completed",
            result_revision: job.to_string(),
            result: format!(
                "/s/{}/r/{job}/",
                crate::projection::snapshot_id_hex(overview.snapshot_id)
            ),
        },
    ))
    .into_response()
}

fn parse_enrichment_target(value: &str) -> Option<EnrichmentTarget> {
    let fields = value.split(':').collect::<Vec<_>>();
    match fields.as_slice() {
        ["page", vol, page] => Some(EnrichmentTarget::Page(Vpid::new(
            parse_vol(vol)?,
            parse_page(page)?,
        ))),
        ["slot", vol, page, slot] => Some(EnrichmentTarget::Slot(Oid::new(
            parse_vol(vol)?,
            parse_page(page)?,
            parse_slot(slot)?,
        ))),
        ["oos", vol, page, slot] => Some(EnrichmentTarget::Oos(Oid::new(
            parse_vol(vol)?,
            parse_page(page)?,
            parse_slot(slot)?,
        ))),
        _ => None,
    }
}

fn parse_canonical(value: &str) -> Option<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn parse_vol(value: &str) -> Option<VolId> {
    VolId::new(i16::try_from(parse_canonical(value)?).ok()?).ok()
}

fn parse_page(value: &str) -> Option<PageId> {
    PageId::new(i32::try_from(parse_canonical(value)?).ok()?).ok()
}

fn parse_slot(value: &str) -> Option<SlotId> {
    SlotId::new(i16::try_from(parse_canonical(value)?).ok()?).ok()
}

fn target_result_path(snapshot: &str, revision: u64, target: EnrichmentTarget) -> String {
    match target {
        EnrichmentTarget::Page(vpid) => format!(
            "/s/{snapshot}/r/{revision}/page/{}/{}",
            vpid.vol_id.get(),
            vpid.page_id.get()
        ),
        EnrichmentTarget::Slot(oid) => format!(
            "/s/{snapshot}/r/{revision}/slot/{}/{}/{}",
            oid.vol_id.get(),
            oid.page_id.get(),
            oid.slot_id.get()
        ),
        EnrichmentTarget::Oos(oid) => format!(
            "/s/{snapshot}/r/{revision}/oos/{}/{}/{}",
            oid.vol_id.get(),
            oid.page_id.get(),
            oid.slot_id.get()
        ),
    }
}

fn api_envelope<T: Serialize>(
    overview: &crate::inspection::OverviewView,
    data: T,
) -> ApiEnvelope<T> {
    ApiEnvelope {
        schema: SCHEMA_NAME,
        schema_version: SCHEMA_VERSION,
        document_type: "resource",
        snapshot: SnapshotProjection {
            id: snapshot_id_hex(overview.snapshot_id),
            revision: overview.revision.get().to_string(),
            validity: match overview.validity {
                crate::model::SnapshotValidity::Valid => "valid",
                crate::model::SnapshotValidity::Invalidated => "invalidated",
            },
            format_profile: overview.format_profile,
        },
        outcome: outcome_name(overview.outcome),
        coverage: overview
            .coverage
            .iter()
            .copied()
            .map(coverage_projection)
            .collect(),
        diagnostics: overview
            .diagnostics
            .iter()
            .cloned()
            .map(diagnostic_projection)
            .collect(),
        data,
    }
}

fn matches_revision(
    overview: &crate::inspection::OverviewView,
    snapshot: &str,
    revision: u64,
) -> bool {
    snapshot_id_hex(overview.snapshot_id) == snapshot && overview.revision.get() == revision
}

#[derive(Serialize)]
struct ErrorEnvelope {
    schema: &'static str,
    schema_version: u32,
    document_type: &'static str,
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

fn error_response(status: StatusCode, code: &'static str) -> Response {
    error_response_with_message(status, code, default_error_message(code))
}

fn error_response_with_message(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            schema: SCHEMA_NAME,
            schema_version: SCHEMA_VERSION,
            document_type: "error",
            error: ErrorDetail {
                code,
                message: message.into(),
            },
        }),
    )
        .into_response()
}

fn stale_revision_response(requested_revision: u64, current_revision: u64) -> Response {
    error_response_with_message(
        StatusCode::CONFLICT,
        "base-revision-unusable",
        format!(
            "This page inspection started at revision {requested_revision}, but another inspection already published revision {current_revision}. Reload the latest revision and try again."
        ),
    )
}

fn invalidated_snapshot_response() -> Response {
    error_response_with_message(
        StatusCode::CONFLICT,
        "base-revision-unusable",
        "The source volumes changed after this inspection started, so the snapshot was invalidated and cannot be enriched. Restart Volmap against a stable snapshot.",
    )
}

fn default_error_message(code: &str) -> &'static str {
    match code {
        "base-revision-unusable" => {
            "This inspection revision cannot be enriched because a newer revision exists or the snapshot was invalidated. Reload the latest revision and try again."
        }
        "enrichment-failed" => {
            "Volmap could not inspect the requested page structure because an internal enrichment operation failed."
        }
        "entity-not-found" => {
            "The requested volume, sector, page, slot, or OOS chain does not exist in this revision."
        }
        "fetch-context-rejected" => {
            "The request was blocked because it did not come from this Volmap page."
        }
        "headers-too-large" => "The request headers exceed the server limit.",
        "invalid-collection-limit" => "The requested collection size is outside the allowed range.",
        "invalid-collection-query" => "The collection query is malformed.",
        "invalid-cursor" => {
            "This collection cursor is invalid or belongs to a different inspection revision."
        }
        "invalid-host" => "The request Host does not match this Volmap listener.",
        "invalid-selector" => "The page, slot, or OOS selector is malformed.",
        "job-not-found" => "The requested enrichment job does not exist in this session.",
        "json-content-type-required" => {
            "Enrichment requests must use the application/json content type."
        }
        "malformed-request" => "The request body is malformed or exceeds the accepted JSON shape.",
        "method-not-allowed" => "This HTTP method is not allowed for the requested resource.",
        "origin-rejected" => "The request Origin does not match this Volmap page.",
        "resource-admission-refused" => {
            "Volmap is already processing the allowed amount of inspection work. Wait for it to finish and try again."
        }
        "resource-not-found" => "The requested Volmap resource does not exist.",
        "revision-not-found" => {
            "This inspection revision does not exist in the current Volmap session."
        }
        "session-unavailable" => "The Volmap inspection session is unavailable.",
        "uri-too-long" => "The request URL exceeds the server limit.",
        _ => "The Volmap request failed.",
    }
}

fn validate_listener(options: &ServeOptions) -> Result<(), ServeError> {
    let ip = options.listen.ip();
    if !ip.is_loopback() && ip != IpAddr::V4(Ipv4Addr::UNSPECIFIED) {
        return Err(ServeError::RemoteWildcardRequired);
    }
    Ok(())
}

fn generate_cursor_key() -> Result<[u8; 32], ServeError> {
    let mut random = OpenOptions::new().read(true).open("/dev/urandom")?;
    let mut bytes = [0_u8; 32];
    random.read_exact(&mut bytes)?;
    Ok(bytes)
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer"><title>Volmap Inspector</title><link rel="stylesheet" href="/app.css"></head>
<body><header><strong>VOLMAP</strong><span id="crumb">loading session</span><span class="spacer"></span><button id="licenses">About &amp; licenses</button><span id="outcome">loading</span></header>
<main id="app"><aside><h2>Snapshot hierarchy</h2><div id="volumes"></div></aside><section class="workspace"><nav id="drillBreadcrumb" aria-label="Inspection hierarchy"></nav><div id="workspaceContent"></div></section></main>
<dialog id="infoDialog"><button id="closeInfo">Close</button><pre id="infoContent" class="withheld"></pre></dialog>
<script src="/app.js"></script></body></html>"#;

#[allow(clippy::needless_raw_string_hashes)]
const APP_CSS: &str = r#":root{color-scheme:dark;--bg:#071014;--panel:#0d1820;--line:#29404b;--text:#dce8ec;--muted:#8fa5ae;--cyan:#68d8d0;--unreserved:#24323a;--reserved:#315f8a;--allocated:#2f845e;--system:#7658a5;--finding:#d25569}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.4 system-ui,sans-serif}button{font:inherit}header{position:sticky;top:0;z-index:4;height:54px;display:flex;align-items:center;gap:18px;padding:0 18px;border-bottom:1px solid var(--line);background:#0a1319}header strong{letter-spacing:.08em}.spacer{flex:1}button{padding:8px 11px;background:var(--cyan);color:#071014;border:0;font-weight:700;cursor:pointer}button:focus-visible,[role=gridcell]:focus-visible{outline:2px solid #ffd376;outline-offset:2px}main{min-height:calc(100vh - 54px);display:grid;grid-template-columns:220px minmax(560px,1fr)}aside,.workspace{min-width:0}aside{position:sticky;top:54px;height:calc(100vh - 54px);align-self:start;overflow:auto;border-right:1px solid var(--line)}h1,h2,h3,p{margin:0}h2{font-size:12px;text-transform:uppercase;letter-spacing:.12em;color:var(--muted);padding:14px;border-bottom:1px solid var(--line)}#volumes{padding:10px}.nav{display:block;width:100%;text-align:left;background:transparent;color:var(--text);border:0;padding:7px;margin:0}.nav.active{background:#16303b;color:var(--cyan)}#drillBreadcrumb{display:flex;align-items:center;gap:8px;min-height:50px;padding:9px 18px;border-bottom:1px solid var(--line);color:var(--muted)}#drillBreadcrumb button{padding:6px 9px;border:1px solid var(--line);background:var(--panel);color:var(--cyan)}#drillBreadcrumb .back{margin-right:8px;background:var(--cyan);color:var(--bg)}#workspaceContent{padding-bottom:24px}.workspace-title{display:flex;gap:18px;align-items:end;padding:18px}.workspace-title p,#legend,#mapStatus,.muted{color:var(--muted);font-size:12px}#legend{display:flex;flex-wrap:wrap;justify-content:flex-end;gap:5px 12px;margin-left:auto;max-width:620px}.swatch{display:inline-block;width:9px;height:9px;margin-right:5px;border-radius:2px;background:var(--unreserved)}.swatch.reserved-unallocated,.swatch.free{background:var(--reserved)}.swatch.allocated{background:var(--allocated)}.swatch.system-metadata{background:var(--system)}.swatch.finding{background:transparent;border:2px solid var(--finding)}#volumeMap{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:10px;padding:0 18px 18px}.sector-card{min-width:0;padding:0;border:1px solid var(--line);background:var(--panel);color:var(--text);text-align:left}.sector-card:hover{border-color:var(--cyan)}.sector-heading{display:flex;flex-wrap:wrap;gap:2px 7px;padding:6px 7px;color:var(--muted);font-size:11px}.sector-heading strong{color:var(--text)}.sector-heading span{margin-left:auto}.sector-heading em{flex:1 0 100%;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-style:normal;color:var(--cyan)}.sector-preview-pages,.sector-focus-grid{display:grid;grid-template-columns:repeat(8,1fr)}.sector-preview-pages{gap:2px;padding:0 7px 7px}.page{aspect-ratio:1;min-width:0;margin:0;padding:0;border:1px solid transparent;border-radius:1px;background:var(--unreserved)}.page.unreserved{background:var(--unreserved)}.page.reserved-unallocated{background:var(--reserved)}.page.allocated{background:var(--allocated)}.page.allocated.occupancy-known{background:linear-gradient(to top,var(--allocated) 0 var(--occupied),var(--reserved) var(--occupied) 100%)}.page.allocated.occupancy-unknown{background:repeating-linear-gradient(135deg,var(--allocated) 0 4px,var(--reserved) 4px 8px)}.page.system-metadata{background:var(--system)}.page.finding{outline:2px solid var(--finding);outline-offset:-2px}.preview-page{display:block}.sector-focus{max-width:790px;margin:0 auto;padding:0 18px 30px}.sector-focus-grid{gap:7px}.focus-page{position:relative;color:var(--text);text-align:left}.focus-page.selected{border-color:var(--cyan);box-shadow:0 0 0 1px var(--cyan),0 0 8px var(--cyan)}.focus-page .page-id{position:absolute;left:6px;bottom:5px;font-size:11px}.focus-page .page-kind{position:absolute;top:5px;left:6px;right:6px;overflow:hidden;color:#d5e0e4;font-size:9px;text-overflow:ellipsis;white-space:nowrap}#mapStatus{padding:0 18px 24px}#mapSentinel{height:1px;grid-column:1/-1}.page-workspace{display:grid;grid-template-columns:minmax(300px,1fr) minmax(360px,1.15fr);gap:18px;padding:0 18px 24px}.panel{min-width:0;padding:16px;border:1px solid var(--line);background:var(--panel)}.panel h2{padding:0 0 10px;border:0;color:var(--text);font-size:17px;letter-spacing:0;text-transform:none}.panel h3{margin:18px 0 8px}.page-facts{grid-template-columns:125px 1fr}.structure-facts{margin-top:16px}.slot-map{display:block;width:100%;height:82px;margin:10px 0;border:1px solid var(--line);background:#16242c}.slot-table{width:100%;border-collapse:collapse}.slot-table th,.slot-table td{padding:7px;border-bottom:1px solid var(--line);text-align:right}.slot-table th:first-child,.slot-table td:first-child{text-align:left}.slot-action{padding:3px 6px;background:transparent;color:var(--cyan);border:1px solid var(--line)}.slot-detail{grid-column:1/-1}.status-note{margin-top:12px;padding:9px;border:1px solid var(--line);color:var(--muted)}.error-note{display:grid;gap:5px;margin:12px 18px;padding:12px 14px;border-color:var(--finding);background:#281a20;color:var(--text)}.error-note strong{color:#f0a1af}.error-note small{color:var(--muted);font:11px ui-monospace,monospace}dl{display:grid;grid-template-columns:115px 1fr;gap:7px}dt{color:var(--muted)}dd{margin:0;overflow-wrap:anywhere}.withheld{padding:8px;border:1px solid var(--line);color:var(--muted);font-family:ui-monospace,monospace;white-space:pre-wrap}dialog{max-width:min(860px,90vw);max-height:80vh;background:var(--panel);color:var(--text);border:1px solid var(--cyan)}dialog::backdrop{background:#000b}@media(max-width:900px){main{grid-template-columns:190px 1fr}.page-workspace{grid-template-columns:1fr}}@media(max-width:720px){header{position:static;height:auto;min-height:54px;flex-wrap:wrap;padding:10px 14px}main{display:block}aside{position:static;height:auto;border:0;border-bottom:1px solid var(--line)}.workspace-title{display:block}.workspace-title #legend{justify-content:flex-start;margin:10px 0 0}#volumeMap{grid-template-columns:repeat(2,minmax(135px,1fr));gap:8px;padding:0 12px 16px}.sector-focus-grid{gap:4px}.page-workspace{padding:0 12px 18px}}"#;

const DISTRIBUTION_CSS: &str = r"
.page-distribution{display:grid;gap:14px}
.page-workspace{align-items:start}
.distribution-summary{display:grid;grid-template-columns:repeat(4,minmax(90px,1fr));gap:8px}
.distribution-metric{padding:8px;border:1px solid var(--line);background:#101e25}
.distribution-metric strong,.distribution-metric span{display:block}
.distribution-metric strong{font-size:18px;color:var(--text)}
.distribution-metric span{color:var(--muted);font-size:11px}
.distribution-legend{display:flex;flex-wrap:wrap;gap:7px 14px;color:var(--muted);font-size:11px}
.distribution-legend i{display:inline-block;width:11px;height:11px;margin-right:5px;vertical-align:-2px;border:1px solid #ffffff20}
.region-header{background:#7658a5}
.region-record{background:#2f845e}
.region-fragmented-free{background:#263842}
.region-contiguous-free{background:repeating-linear-gradient(135deg,#254955 0,#254955 5px,#1b3741 5px,#1b3741 10px)}
.region-slot-directory{background:#315f8a}
.full-page-map{position:relative;height:76px;border:1px solid var(--line);background:#111d23;overflow:hidden}
.full-page-map .page-region{position:absolute;inset-block:0;border-right:1px solid #071014}
.full-page-map .page-region:focus-visible{z-index:2;outline:2px solid #ffd376;outline-offset:-2px}
.page-map-axis{display:flex;justify-content:space-between;margin-top:-10px;color:var(--muted);font:10px ui-monospace,monospace}
.distribution-section-title{display:flex;justify-content:space-between;align-items:baseline;gap:10px}
.distribution-section-title h3{margin:0}
.region-list{display:grid;gap:4px;max-height:420px;overflow:auto;padding-right:4px}
.region-row{display:grid;grid-template-columns:minmax(150px,1.2fr) 132px 74px minmax(120px,1fr);gap:9px;align-items:center;padding:6px;border:1px solid #213741;background:#0b171d}
.region-name{display:flex;align-items:center;min-width:0}
.region-name i{flex:0 0 auto;width:10px;height:24px;margin-right:8px}
.region-name span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.region-range,.region-size{color:var(--muted);font:11px ui-monospace,monospace;text-align:right}
.region-lane{position:relative;height:12px;background:#17262e}
.region-lane i{position:absolute;inset-block:0;min-width:2px}
.slot-directory-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:7px;max-height:420px;overflow:auto;padding-right:4px}
.slot-entry{display:grid;grid-template-columns:1fr auto;gap:3px 8px;padding:9px;border:1px solid var(--line);background:#12232b;color:var(--text);text-align:left;font-weight:400}
.slot-entry:hover{border-color:var(--cyan)}
.slot-entry strong{color:var(--text)}
.slot-entry .slot-state{font-size:10px;text-transform:uppercase;letter-spacing:.08em}
.slot-entry small{grid-column:1/-1;color:var(--muted)}
.slot-entry.allocated{border-left:4px solid #2f845e}
.slot-entry.allocated .slot-state{color:#79d4a6}
.slot-entry.unallocated{border-left:4px solid #87733f}
.slot-entry.unallocated .slot-state{color:#d7bd71}
.slot-entry.deleted{border-left:4px solid #a3485a;background:#281a20}
.slot-entry.deleted .slot-state{color:#e68d9d}
@media(max-width:1100px){.distribution-summary{grid-template-columns:repeat(2,1fr)}.region-row{grid-template-columns:minmax(140px,1fr) 125px 65px}.region-lane{grid-column:1/-1}}
@media(max-width:720px){.distribution-summary{grid-template-columns:1fr 1fr}.region-row{grid-template-columns:1fr auto}.region-range{grid-column:1}.region-size{grid-column:2;grid-row:1}.slot-directory-grid{grid-template-columns:1fr}}
";

const APP_JS: &str = r"(()=>{'use strict';
let session=null,currentVolume=null,currentSector=null,currentPage=null,selectedPage=null,selectedSlot=null,currentLevel='volume',volumeView=null,sectorCursor='end',loadedSectors=0,loadingGeneration=null,loadGeneration=0,routeGeneration=0;
const sectorCache=new Map();
const $=id=>document.getElementById(id);
function button(label,action,className=''){const node=document.createElement('button');node.textContent=label;node.className=className;node.onclick=action;return node}
function fieldList(fields){const list=document.createElement('dl');for(const [name,value] of fields){const term=document.createElement('dt'),detail=document.createElement('dd');term.textContent=name;detail.textContent=String(value);list.append(term,detail)}return list}
function parseBrowserRoute(pathname=location.pathname){if(pathname==='/')return{kind:'root'};const parts=pathname.split('/'),number=value=>/^(0|[1-9]\d*)$/.test(value)&&Number.isSafeInteger(Number(value))?Number(value):null;if(parts[0]!==''||parts[1]!=='s'||!/^[0-9a-f]{32}$/.test(parts[2]||'')||parts[3]!=='r'||!/^(0|[1-9]\d*)$/.test(parts[4]||''))return null;const route={snapshot:parts[2],revision:parts[4],kind:parts[5]},vol=number(parts[6]);if(vol===null)return null;route.vol=vol;if(route.kind==='volume'&&parts.length===7)return route;if((route.kind==='sector'||route.kind==='page')&&parts.length===8){const value=number(parts[7]);if(value===null)return null;route[route.kind]=value;return route}if((route.kind==='slot'||route.kind==='oos')&&parts.length===9){const page=number(parts[7]),slot=number(parts[8]);if(page===null||slot===null)return null;route.page=page;route.slot=slot;return route}return null}
function browserRoute(kind){const route={kind,snapshot:session.snapshot.id,revision:String(session.snapshot.revision),vol:currentVolume.vol_id};if(kind==='sector')route.sector=currentSector.sector_id;if(kind==='page')route.page=selectedPage;if(kind==='slot'||kind==='oos'){route.page=selectedPage;route.slot=selectedSlot}return route}
function browserRoutePath(route){const prefix=`/s/${route.snapshot}/r/${route.revision}`;if(route.kind==='volume')return`${prefix}/volume/${route.vol}`;if(route.kind==='sector')return`${prefix}/sector/${route.vol}/${route.sector}`;if(route.kind==='page')return`${prefix}/page/${route.vol}/${route.page}`;return`${prefix}/${route.kind}/${route.vol}/${route.page}/${route.slot}`}
function browserParentPath(route){if(route.kind==='volume')return null;if(route.kind==='sector')return browserRoutePath({...route,kind:'volume'});if(route.kind==='page')return browserRoutePath({...route,kind:'sector',sector:currentSector.sector_id});if(route.kind==='slot')return browserRoutePath({...route,kind:'page'});return browserRoutePath({...route,kind:'slot'})}
function syncBrowserRoute(kind,mode='push'){if(mode==='none')return;const route=browserRoute(kind),path=browserRoutePath(route),parent=browserParentPath(route);if(location.pathname===path){if(!history.state?.volmap)history.replaceState({volmap:true,previous:null,parent},'',path);return}if(mode==='replace')history.replaceState({volmap:true,previous:history.state?.previous||null,parent},'',path);else history.pushState({volmap:true,previous:location.pathname,parent},'',path)}
function installBrowserRouteState(route){const current=browserRoute(route.kind);history.replaceState({volmap:true,previous:null,parent:browserParentPath(current)},'',browserRoutePath(current))}
async function api(path,options={}){const response=await fetch(path,{...options,cache:'no-store',credentials:'same-origin'});if(!response.ok){let payload=null;try{payload=await response.json()}catch{}const detail=payload&&payload.error,reason=detail&&typeof payload.error.message==='string'?payload.error.message:`The server rejected this request (HTTP ${response.status}).`,error=new Error(reason);error.status=response.status;error.code=detail&&typeof detail.code==='string'?detail.code:'http-error';throw error}return response.json()}
function base(){return `/api/v1/s/${session.snapshot.id}/r/${session.snapshot.revision}`}
function updateSession(payload){session.snapshot=payload.snapshot;session.outcome=payload.outcome;$('outcome').textContent=payload.outcome;$('crumb').textContent=`snapshot ${payload.snapshot.id.slice(0,12)} · revision ${payload.snapshot.revision}`}
async function start(){try{const route=parseBrowserRoute();if(!route)throw new Error('invalid inspector URL');session=await api('/api/v1/session');if(route.kind!=='root'){if(route.snapshot!==session.snapshot.id)throw new Error('this URL belongs to a different snapshot');session.snapshot.revision=route.revision}updateSession(session);await loadVolumes(route)}catch(error){renderWorkspaceError(error)}}
async function loadVolumes(route={kind:'root'}){const payload=await api(`${base()}/volumes`),root=$('volumes'),volumes=payload.data.items;updateSession(payload);root.replaceChildren();for(const volume of volumes){const node=button(`volume ${volume.vol_id} · ${volume.total_sectors} sectors`,()=>selectVolume(volume),'nav');node.dataset.volume=String(volume.vol_id);root.append(node)}if(!volumes.length)return;const volume=route.kind==='root'?volumes[0]:volumes.find(value=>value.vol_id===route.vol);if(!volume)throw new Error('the URL volume does not exist in this revision');activateVolume(volume);if(route.kind==='root')await showVolume('replace');else{await restoreBrowserRoute(route);installBrowserRouteState(route)}}
function invalidateVolumeView(){mapObserver.disconnect();volumeView=null;sectorCache.clear();sectorCursor='end';loadedSectors=0;loadGeneration++}
function activateVolume(volume){currentVolume=volume;currentSector=null;currentPage=null;selectedPage=null;selectedSlot=null;invalidateVolumeView();document.querySelectorAll('#volumes .nav').forEach(node=>node.classList.toggle('active',node.dataset.volume===String(volume.vol_id)))}
async function selectVolume(volume,historyMode='push'){activateVolume(volume);await showVolume(historyMode)}
function hierarchyBack(parentPath,action){if(history.state?.previous===parentPath)history.back();else action('replace')}
function renderBreadcrumb(level){currentLevel=level;const root=$('drillBreadcrumb'),route=browserRoute(level);root.replaceChildren();if(level!=='volume'){const parent=browserParentPath(route),actions={sector:mode=>showVolume(mode),page:mode=>showSector(currentSector,mode),slot:mode=>showPage(selectedPage,true,mode),oos:mode=>showSlot(currentPage,selectedSlot,mode)};root.append(button('← Back',()=>hierarchyBack(parent,actions[level]),'back'))}root.append(button(`Volume ${currentVolume.vol_id}`,()=>showVolume()));if(['sector','page','slot','oos'].includes(level)){root.append('›');root.append(button(`Sector ${currentSector.sector_id}`,()=>showSector(currentSector)))}if(['page','slot','oos'].includes(level)){root.append('›');const page=level==='page'?document.createElement('span'):button(`Page ${selectedPage}`,()=>showPage(selectedPage,true));page.textContent=`Page ${selectedPage}`;root.append(page)}if(['slot','oos'].includes(level)){root.append('›');const slot=level==='slot'?document.createElement('span'):button(`Slot ${selectedSlot}`,()=>showSlot(currentPage,selectedSlot));slot.textContent=`Slot ${selectedSlot}`;root.append(slot)}if(level==='oos'){root.append('›');const oos=document.createElement('span');oos.textContent='OOS chain';root.append(oos)}}
function createVolumeView(){sectorCursor=null;loadedSectors=0;const generation=++loadGeneration,root=document.createElement('section'),title=document.createElement('div'),map=document.createElement('div'),status=document.createElement('p');root.className='volume-view';title.className='workspace-title';title.innerHTML=`<div><h1>Volume ${currentVolume.vol_id} · full map</h1><p>${currentVolume.total_sectors} sectors · 64 pages per sector · revision ${session.snapshot.revision}</p></div><div id='legend' aria-label='Page allocation and occupancy legend'><span><i class='swatch unreserved'></i>Unreserved</span><span><i class='swatch reserved-unallocated'></i>Reserved, unallocated</span><span><i class='swatch allocated'></i>Occupied</span><span><i class='swatch free'></i>Slotted free</span><span><i class='swatch system-metadata'></i>System metadata</span><span><i class='swatch finding'></i>Finding outline</span></div>`;map.id='volumeMap';map.setAttribute('aria-label','Full volume sector map');status.id='mapStatus';status.setAttribute('role','status');status.textContent='Loading sector maps…';root.append(title,map,status);volumeView=root;return generation}
async function showVolume(historyMode='push'){mapObserver.disconnect();currentSector=null;currentPage=null;selectedPage=null;selectedSlot=null;renderBreadcrumb('volume');let generation=loadGeneration;if(!volumeView)generation=createVolumeView();$('workspaceContent').replaceChildren(volumeView);syncBrowserRoute('volume',historyMode);if(sectorCursor===null&&loadedSectors===0)await loadSectorBatch(generation);observeMapEnd()}
async function loadSectorBatch(generation=loadGeneration){if(loadingGeneration===generation||sectorCursor==='end'||generation!==loadGeneration)return;loadingGeneration=generation;try{const query=sectorCursor===null?'?limit=24':`?limit=24&cursor=${encodeURIComponent(sectorCursor)}`,payload=await api(`${base()}/sectors/${currentVolume.vol_id}${query}`);if(generation!==loadGeneration)return;for(const sector of payload.data.items)appendSector(sector);loadedSectors+=payload.data.items.length;sectorCursor=payload.data.next_cursor.state==='present'?payload.data.next_cursor.value:'end';$('mapStatus').textContent=sectorCursor==='end'?`All ${loadedSectors} sectors shown · ${loadedSectors*64} pages`:`Showing ${loadedSectors} of ${currentVolume.total_sectors} sectors · scroll to continue`;}catch(error){if(generation===loadGeneration){sectorCursor='end';$('mapStatus').textContent=error.message}}finally{if(loadingGeneration===generation)loadingGeneration=null}}
function pageOccupancyLabel(page){return page.occupancy.state==='known'?`, ${page.occupancy.occupied_percent}% occupied, ${page.occupancy.free_percent}% free`:', occupancy unknown'}
function applyPageFill(node,page){if(page.allocation!=='allocated')return;if(page.occupancy.state==='known'){node.classList.add('occupancy-known');node.style.setProperty('--occupied',`${page.occupancy.occupied_percent}%`)}else node.classList.add('occupancy-unknown')}
function classNameLabel(name){if(name.state==='resolved')return name.value;if(name.state==='unresolved')return`unresolved (${name.reason})`;return`not applicable (${name.reason})`}
function sectorAttributionLabel(sector){const a=sector.attribution;if(!a||a.state==='unclaimed')return'';if(a.state==='mixed')return'mixed';const file=a.file;if(file.class_name.state==='resolved')return file.class_name.value;if(file.class_oid.state==='present')return'unresolved';return file.file_type.state==='known'?`internal · ${file.file_type.value}`:'internal'}
function sectorAttributionDetail(sector){const a=sector.attribution;if(!a||a.state==='unclaimed')return'';if(a.state==='mixed')return`mixed: ${a.claims.length} conflicting file claims`;const file=a.file,role=file.file_type.state==='known'?file.file_type.value:'unavailable';return`${sectorAttributionLabel(sector)} · ${role} · ${a.allocated_pages}/64 allocated`}
function fileAssociationRows(fa){if(fa.state==='none')return[['File','none']];if(fa.state==='mixed-claims')return[['File','mixed claims']];const file=fa.file,rows=[['File',`file:${file.vol_id}:${file.file_id}${fa.state==='reserved-for'?' (reserved, not allocated)':''}`],['File role',file.file_type.state==='known'?file.file_type.value:'unavailable']];if(file.class_oid.state==='present'){const oid=file.class_oid.oid;rows.push(['Class OID',`oid:${oid.vol_id}:${oid.page_id}:${oid.slot_id}`])}rows.push(['Class/table',classNameLabel(file.class_name)]);return rows}
function appendSector(sector){if(sector.pages.length!==64)throw new Error(`sector ${sector.sector_id} did not contain 64 pages`);sectorCache.set(sector.sector_id,sector);const card=button('',()=>showSector(sector),'sector-card');card.id=`sector-${sector.sector_id}`;const tableLabel=sectorAttributionLabel(sector);card.setAttribute('aria-label',`Sector ${sector.sector_id}, ${sector.reserved?'reserved':'unreserved'}${tableLabel?`, ${tableLabel}`:''}, 64 pages`);const heading=document.createElement('span');heading.className='sector-heading';const title=document.createElement('strong');title.textContent=`Sector ${sector.sector_id}`;const state=document.createElement('span');state.textContent=sector.reserved?'reserved':'unreserved';if(tableLabel){const table=document.createElement('em');table.className='sector-table';table.textContent=tableLabel;table.title=sectorAttributionDetail(sector);heading.append(title,state,table)}else heading.append(title,state);const pages=document.createElement('span');pages.className='sector-preview-pages';for(const page of sector.pages){const finding=page.diagnostic.state==='known',node=document.createElement('i');node.className=`page preview-page ${page.allocation}${finding?' finding':''}`;applyPageFill(node,page);pages.append(node)}card.append(heading,pages);volumeView.querySelector('#volumeMap').append(card)}
function moveSectorGrid(event,grid,index){let next=index;if(event.key==='ArrowLeft')next--;else if(event.key==='ArrowRight')next++;else if(event.key==='ArrowUp')next-=8;else if(event.key==='ArrowDown')next+=8;else return;if(next>=0&&next<64){event.preventDefault();grid.children[next].focus()}}
const mapObserver=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)){mapObserver.disconnect();loadSectorBatch().then(observeMapEnd)}});
function observeMapEnd(){mapObserver.disconnect();if(currentLevel!=='volume'||!volumeView)return;const old=volumeView.querySelector('#mapSentinel');if(old)old.remove();if(sectorCursor==='end')return;const sentinel=document.createElement('div');sentinel.id='mapSentinel';volumeView.querySelector('#volumeMap').append(sentinel);mapObserver.observe(sentinel)}
function showSector(sector,historyMode='push'){mapObserver.disconnect();currentSector=sector;currentPage=null;selectedPage=null;selectedSlot=null;renderBreadcrumb('sector');const content=$('workspaceContent'),title=document.createElement('div'),focus=document.createElement('section'),grid=document.createElement('div');title.className='workspace-title';const attributionDetail=sectorAttributionDetail(sector);title.innerHTML=`<div><h1>Sector ${sector.sector_id}</h1><p>64 physical pages · select a page to enlarge</p></div>`;if(attributionDetail){const note=document.createElement('p');note.className='muted';note.textContent=attributionDetail;title.firstElementChild.append(note)}focus.className='sector-focus';grid.className='sector-focus-grid';grid.setAttribute('role','grid');grid.setAttribute('aria-label',`Sector ${sector.sector_id}, 64 physical pages`);sector.pages.forEach((page,index)=>{const finding=page.diagnostic.state==='known',node=button('',()=>showPage(page.page_id),`page focus-page ${page.allocation}${finding?' finding':''}${page.page_id===selectedPage?' selected':''}`),kind=document.createElement('span'),identity=document.createElement('span');kind.className='page-kind';kind.textContent=page.page_type.state==='known'?page.page_type.value:'not inspected';identity.className='page-id';identity.textContent=String(page.page_id);node.append(kind,identity);applyPageFill(node,page);node.setAttribute('role','gridcell');node.setAttribute('aria-label',`Page ${page.page_id}, ${page.allocation}${pageOccupancyLabel(page)}${finding?', finding':''}`);node.onkeydown=event=>moveSectorGrid(event,grid,index);grid.append(node)});focus.append(grid);content.replaceChildren(title,focus);syncBrowserRoute('sector',historyMode)}
function withheld(identity){const note=document.createElement('p');note.className='withheld';note.textContent=`evidence ${identity} · structural ranges only · bytes withheld`;return note}
async function ensureSector(sectorId){if(currentSector?.sector_id===sectorId)return;const payload=await api(`${base()}/sector/${currentVolume.vol_id}/${sectorId}`);currentSector=payload.data}
async function showPage(pageId,skipEnrichment=false,historyMode='push'){try{const payload=await api(`${base()}/page/${currentVolume.vol_id}/${pageId}`),p=payload.data.page,deep=payload.data.deep;await ensureSector(p.sector_id);currentPage=p;selectedPage=p.page_id;selectedSlot=null;const shouldEnrich=!skipEnrichment&&deep.state==='not-enriched'&&p.detail_support.state==='known';renderPageWorkspace(payload,shouldEnrich);syncBrowserRoute('page',historyMode);if(shouldEnrich)await enrichSelectedPage(p);return p}catch(error){renderWorkspaceError(error);return null}}
function appendPrimitiveStructure(root,deep){if(!deep.structure)return;const fields=[];for(const [name,value] of Object.entries(deep.structure)){if(name==='slots'||name==='bytes'||value===null||typeof value==='object')continue;fields.push([name.replaceAll('_',' '),value])}if(fields.length){const title=document.createElement('h3');title.textContent='Decoded structure';root.append(title,fieldList(fields))}}
function distributionLegend(){const root=document.createElement('div');root.className='distribution-legend';for(const [kind,label] of [['header','Slotted header'],['record','Allocated record'],['fragmented-free','Fragmented free'],['contiguous-free','Contiguous free'],['slot-directory','Slot directory']]){const item=document.createElement('span'),swatch=document.createElement('i');swatch.className=`region-${kind}`;item.append(swatch,label);root.append(item)}return root}
function distributionRegions(distribution){const regions=[{...distribution.header,kind:'header',label:'Slotted-page header'},...distribution.record_extents.map(record=>({...record,kind:'record',label:`Slot ${record.slot_id} · ${record.record_type}`})),...distribution.free_regions.map((region,index)=>({...region,label:`${region.kind==='contiguous-free'?'Contiguous':'Fragmented'} free region ${index+1}`})),{...distribution.slot_directory,kind:'slot-directory',label:'Slot directory'}];regions.sort((left,right)=>left.offset-right.offset||left.length-right.length);return regions}
function distributionMetric(value,label){const node=document.createElement('div'),number=document.createElement('strong'),caption=document.createElement('span');node.className='distribution-metric';number.textContent=String(value);caption.textContent=label;node.append(number,caption);return node}
function renderSlottedDistribution(page,slots,distribution){const root=document.createDocumentFragment(),title=document.createElement('h2'),summary=document.createElement('div'),map=document.createElement('div'),axis=document.createElement('div'),regions=distributionRegions(distribution),slotById=new Map(slots.map(slot=>[slot.slot_id,slot])),notAllocated=distribution.slot_entries.filter(entry=>entry.state!=='allocated').length;title.textContent='Full slotted-page distribution';summary.className='distribution-summary';summary.append(distributionMetric(distribution.record_extents.length,'allocated records'),distributionMetric(notAllocated,'slots not allocated'),distributionMetric(distribution.free_regions.length,'free byte regions'),distributionMetric(`${distribution.unoccupied_bytes} B`,'unoccupied bytes'));map.className='full-page-map';map.setAttribute('aria-label',`Complete ${distribution.content_size}-byte slotted-page content map`);for(const region of regions){const node=region.kind==='record'?button('',()=>showSlot(page,region.slot_id)):document.createElement('span'),end=region.offset+region.length,label=`${region.label}: offset ${region.offset}, size ${region.length} bytes, end ${end}`;node.className=`page-region region-${region.kind}`;node.style.left=`${region.offset/distribution.content_size*100}%`;node.style.width=`${region.length/distribution.content_size*100}%`;node.title=label;node.setAttribute('aria-label',label);map.append(node)}axis.className='page-map-axis';for(const value of [0,Math.floor(distribution.content_size/4),Math.floor(distribution.content_size/2),Math.floor(distribution.content_size*3/4),distribution.content_size]){const tick=document.createElement('span');tick.textContent=String(value);axis.append(tick)}root.append(title,summary,distributionLegend(),map,axis,regionList(page,regions,distribution.content_size),slotDirectory(page,distribution.slot_entries,slotById));return root}
function sectionTitle(title,caption){const root=document.createElement('div'),heading=document.createElement('h3'),detail=document.createElement('span');root.className='distribution-section-title';heading.textContent=title;detail.className='muted';detail.textContent=caption;root.append(heading,detail);return root}
function regionList(page,regions,contentSize){const wrapper=document.createElement('section'),list=document.createElement('div');list.className='region-list';for(const region of regions){const row=document.createElement('div'),name=document.createElement('span'),swatch=document.createElement('i'),label=document.createElement('span'),range=document.createElement('span'),size=document.createElement('span'),lane=document.createElement('span'),extent=document.createElement('i'),end=region.offset+region.length;row.className='region-row';name.className='region-name';swatch.className=`region-${region.kind}`;label.textContent=region.label;name.append(swatch,label);range.className='region-range';range.textContent=`${region.offset}–${end}`;size.className='region-size';size.textContent=`${region.length} B`;lane.className='region-lane';extent.className=`region-${region.kind}`;extent.style.left=`${region.offset/contentSize*100}%`;extent.style.width=`${region.length/contentSize*100}%`;lane.append(extent);row.append(name,range,size,lane);if(region.kind==='record'){row.tabIndex=0;row.setAttribute('role','button');row.setAttribute('aria-label',`Inspect ${region.label}`);row.onclick=()=>showSlot(page,region.slot_id);row.onkeydown=event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();showSlot(page,region.slot_id)}}}list.append(row)}wrapper.append(sectionTitle('Physical intervals',`${regions.length} exhaustive non-overlapping regions`),list);return wrapper}
function slotDirectory(page,entries,slotById){const wrapper=document.createElement('section'),grid=document.createElement('div');grid.className='slot-directory-grid';for(const entry of entries){const slot=slotById.get(entry.slot_id),node=button('',()=>showSlot(page,entry.slot_id),`slot-entry ${entry.state}`),name=document.createElement('strong'),state=document.createElement('span'),kind=document.createElement('small'),directory=document.createElement('small'),record=document.createElement('small');name.textContent=`Slot ${entry.slot_id}`;state.className='slot-state';state.textContent=entry.state==='allocated'?'allocated':entry.state==='deleted'?'deleted · not allocated':'not allocated';kind.textContent=`record type · ${entry.record_type}`;directory.textContent=`directory · ${entry.offset}–${entry.offset+entry.length} (${entry.length} B)`;record.textContent=slot&&Number(slot.offset)>0?`record · ${slot.offset}–${Number(slot.offset)+Number(slot.length)} (${slot.length} B)`:`record · no live extent${slot&&Number(slot.length)>0?` · retained length ${slot.length} B`:''}`;node.append(name,state,kind,directory,record);grid.append(node)}wrapper.append(sectionTitle('Slot directory',`${entries.length} entries · allocated, empty, and deleted`),grid);return wrapper}
function renderPageWorkspace(payload,enriching=false){const p=payload.data.page,deep=payload.data.deep,slots=payload.data.slots,distribution=payload.data.distribution,content=$('workspaceContent'),title=document.createElement('div'),layout=document.createElement('div'),facts=document.createElement('section'),distributionPanel=document.createElement('section');renderBreadcrumb('page');title.className='workspace-title';const fa=p.file_association,pageTable=(fa.state==='allocated'||fa.state==='reserved-for')&&fa.file.class_name.state==='resolved'?fa.file.class_name.value:'';title.innerHTML=`<div><h1>Page ${p.page_id}</h1><p>${p.page_type.state==='known'?p.page_type.value:'unknown type'} · detailed structural view</p></div>`;if(pageTable){const note=document.createElement('p');note.className='muted';note.textContent=pageTable;title.firstElementChild.append(note)}layout.className='page-workspace';facts.className='panel';const factsTitle=document.createElement('h2');factsTitle.textContent='Page facts';facts.append(factsTitle,fieldList([['Identity',`page:${p.vol_id}:${p.page_id}`],['Sector',p.sector_id],['Physical type',p.page_type.state==='known'?p.page_type.value:'not inspected'],['Allocation',p.allocation],...fileAssociationRows(p.file_association),['Availability',p.availability],['Detail support',p.detail_support.state==='known'?p.detail_support.value:p.detail_support.state],['Deep state',deep.state],['TDE',p.tde_state]]));appendPrimitiveStructure(facts,deep);facts.append(withheld(`page:${p.vol_id}:${p.page_id}`));distributionPanel.className='panel page-distribution';if(distribution.state==='available')distributionPanel.append(renderSlottedDistribution(p,slots,distribution));else{const slotsTitle=document.createElement('h2'),note=document.createElement('p');slotsTitle.textContent='Slotted-page distribution';note.className='muted';note.textContent=enriching?'Loading structural metadata…':'No validated slot directory is available for this page.';distributionPanel.append(slotsTitle,note)}if(enriching){const note=document.createElement('p');note.className='status-note';note.setAttribute('role','status');note.textContent='Enriching the selected page at a new immutable revision…';facts.append(note)}layout.append(facts,distributionPanel);content.replaceChildren(title,layout)}
function slotTable(page,slots){const table=document.createElement('table'),head=document.createElement('thead'),body=document.createElement('tbody'),header=document.createElement('tr');table.className='slot-table';for(const label of ['Slot','Record type','Offset','Size (bytes)','']){const cell=document.createElement('th');cell.textContent=label;header.append(cell)}head.append(header);for(const slot of slots){const row=document.createElement('tr');for(const value of [slot.slot_id,slot.record_type,slot.offset,slot.length]){const cell=document.createElement('td');cell.textContent=String(value);row.append(cell)}const action=document.createElement('td');action.append(button('Inspect',()=>showSlot(page,slot.slot_id),'slot-action'));row.append(action);body.append(row)}table.append(head,body);return table}
async function enrichSelectedPage(page){try{const receipt=await api(`${base()}/enrichments`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({selector:`page:${page.vol_id}:${page.page_id}`})});updateSession(receipt);invalidateVolumeView();const sectorPayload=await api(`${base()}/sector/${page.vol_id}/${page.sector_id}`);currentSector=sectorPayload.data;await showPage(page.page_id,true,'push')}catch(error){renderWorkspaceError(error)}}
async function showSlot(page,slotId,historyMode='push'){try{const payload=await api(`${base()}/slot/${page.vol_id}/${page.page_id}/${slotId}`),slot=payload.data.selected_slot,root=document.createElement('section');currentPage=page;selectedPage=page.page_id;selectedSlot=slot.slot_id;renderBreadcrumb('slot');root.id='slotDetail';root.className='panel slot-detail';const title=document.createElement('h2');title.textContent=`Slot ${slot.slot_id}`;root.append(title,fieldList([['Identity',`slot:${page.vol_id}:${page.page_id}:${slot.slot_id}`],['Record type',`${slot.record_type} (${slot.record_type_ordinal})`],['Offset',slot.offset],['Size',slot.length]]));if(page.page_type.state==='known'&&page.page_type.value==='oos'&&Number(slot.offset)>0&&slot.record_type==='home')root.append(button('Validate OOS chain',()=>enrichOos(page,slot.slot_id)));root.append(withheld(`slot:${page.vol_id}:${page.page_id}:${slot.slot_id}`));const old=$('slotDetail');if(old)old.remove();document.querySelector('.page-workspace').append(root);syncBrowserRoute('slot',historyMode);return slot}catch(error){renderWorkspaceError(error);return null}}
function renderOosChain(page,slotId,chain){currentPage=page;selectedPage=page.page_id;selectedSlot=slotId;renderBreadcrumb('oos');const root=document.createElement('section'),title=document.createElement('h2');root.id='slotDetail';root.className='panel slot-detail';title.textContent='OOS chain';root.append(title,fieldList([['Identity',`oos:${page.vol_id}:${page.page_id}:${slotId}`],['Complete',chain.complete],['Validated bytes',chain.validated_payload_bytes],['Chunks',chain.chunks.length],['Diagnostic',chain.diagnostic.state==='known'?chain.diagnostic.value:'none']]));root.append(withheld(`oos:${page.vol_id}:${page.page_id}:${slotId}`));const old=$('slotDetail');if(old)old.remove();document.querySelector('.page-workspace').append(root)}
async function showOos(page,slotId,historyMode='push'){try{const payload=await api(`${base()}/oos/${page.vol_id}/${page.page_id}/${slotId}`);renderOosChain(page,slotId,payload.data.chain);syncBrowserRoute('oos',historyMode)}catch(error){renderWorkspaceError(error)}}
async function enrichOos(page,slotId){try{const receipt=await api(`${base()}/enrichments`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({selector:`oos:${page.vol_id}:${page.page_id}:${slotId}`})});updateSession(receipt);invalidateVolumeView();const sectorPayload=await api(`${base()}/sector/${page.vol_id}/${page.sector_id}`);currentSector=sectorPayload.data;const refreshed=await showPage(page.page_id,true,'none');if(refreshed)await showOos(refreshed,slotId,'push')}catch(error){renderWorkspaceError(error)}}
async function restoreBrowserRoute(route){if(route.kind==='volume'){await showVolume('none');return}if(route.kind==='sector'){const payload=await api(`${base()}/sector/${route.vol}/${route.sector}`);showSector(payload.data,'none');return}const page=await showPage(route.page,true,'none');if(!page)return;if(route.kind==='slot')await showSlot(page,route.slot,'none');if(route.kind==='oos')await showOos(page,route.slot,'none')}
async function restoreBrowserLocation(){const generation=++routeGeneration;try{const route=parseBrowserRoute();if(!route)throw new Error('invalid inspector URL');if(route.kind==='root'){session=await api('/api/v1/session');updateSession(session);if(generation===routeGeneration)await loadVolumes(route);return}if(route.snapshot!==session.snapshot.id)throw new Error('this URL belongs to a different snapshot');session.snapshot.revision=route.revision;if(generation===routeGeneration)await loadVolumes(route)}catch(error){if(generation===routeGeneration)renderWorkspaceError(error)}}
function renderWorkspaceError(error){const old=document.querySelector('.error-note'),note=document.createElement('section'),title=document.createElement('strong'),message=document.createElement('span'),detail=document.createElement('small');if(old)old.remove();note.className='status-note error-note';note.setAttribute('role','alert');title.textContent='Could not complete this view';message.textContent=error.message;detail.textContent=error.status?`HTTP ${error.status} · ${error.code||'unknown-error'}`:`Browser error · ${error.code||'unknown-error'}`;note.append(title,message,detail);$('workspaceContent').append(note)}
async function showLicenses(){const payload=await api('/api/v1/licenses');$('infoContent').textContent=payload.notice;$('infoDialog').showModal()}
window.addEventListener('popstate',()=>{if(session)restoreBrowserLocation()});$('closeInfo').addEventListener('click',()=>$('infoDialog').close());$('licenses').addEventListener('click',showLicenses);start()})();";

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};

    fn state() -> WebState {
        WebState {
            session: Arc::new(RwLock::new(LiveSession {
                views: BTreeMap::new(),
                jobs: BTreeSet::new(),
                latest: 0,
            })),
            enrichment: Arc::new(Mutex::new(())),
            policy: ResourcePolicy::new(1024, 1024, 1, 1, 1024).unwrap(),
            cursor_key: Arc::new([7_u8; 32]),
            authority: Some(Arc::from("127.0.0.1:8787")),
            semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    fn wildcard_state() -> WebState {
        WebState {
            authority: None,
            ..state()
        }
    }

    fn request(method: Method, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(HOST, "127.0.0.1:8787")
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn api_requests_are_unauthenticated() {
        let state = state();
        let request = request(Method::GET, "/api/v1/session");
        assert!(guard(&state, &request).is_ok());
    }

    #[test]
    fn wildcard_listener_accepts_any_valid_host_while_loopback_stays_exact() {
        let state = state();
        let mut wrong_host = request(Method::GET, "/api/v1/session");
        wrong_host
            .headers_mut()
            .insert(HOST, HeaderValue::from_static("attacker.test:8787"));
        wrong_host.headers_mut().insert(
            HeaderName::from_static("x-forwarded-host"),
            HeaderValue::from_static("127.0.0.1:8787"),
        );
        let error = guard(&state, &wrong_host).unwrap_err();
        assert_eq!(error.status, StatusCode::MISDIRECTED_REQUEST);
        assert_eq!(error.code, "invalid-host");

        assert!(guard(&wildcard_state(), &wrong_host).is_ok());

        let mut duplicate = request(Method::GET, "/api/v1/session");
        duplicate
            .headers_mut()
            .append(HOST, HeaderValue::from_static("127.0.0.1:8787"));
        assert_eq!(
            guard(&state, &duplicate).unwrap_err().status,
            StatusCode::MISDIRECTED_REQUEST
        );
    }

    #[test]
    fn post_requires_exact_json_origin_and_same_site_context() {
        let state = state();
        let mut valid = request(Method::POST, "/api/v1/s/id/r/0/enrichments");
        valid
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        valid
            .headers_mut()
            .insert(ORIGIN, HeaderValue::from_static("http://127.0.0.1:8787"));
        assert!(guard(&state, &valid).is_ok());

        let mut wildcard = request(Method::POST, "/api/v1/s/id/r/0/enrichments");
        wildcard
            .headers_mut()
            .insert(HOST, HeaderValue::from_static("debug.internal:8787"));
        wildcard
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        wildcard.headers_mut().insert(
            ORIGIN,
            HeaderValue::from_static("http://debug.internal:8787"),
        );
        assert!(guard(&wildcard_state(), &wildcard).is_ok());

        let mut missing_origin = request(Method::POST, "/api/v1/s/id/r/0/enrichments");
        missing_origin
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        assert_eq!(
            guard(&state, &missing_origin).unwrap_err().status,
            StatusCode::FORBIDDEN
        );

        let mut cross_site = valid;
        cross_site.headers_mut().insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("cross-site"),
        );
        assert_eq!(
            guard(&state, &cross_site).unwrap_err().status,
            StatusCode::FORBIDDEN
        );

        let mut content_type = request(Method::POST, "/api/v1/s/id/r/0/enrichments");
        content_type.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        assert_eq!(
            guard(&state, &content_type).unwrap_err().status,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn unsupported_methods_are_rejected_before_routing() {
        let state = state();
        let invalid_post = request(Method::POST, "/api/v1/session");
        let error = guard(&state, &invalid_post).unwrap_err();
        assert_eq!(error.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(error.code, "method-not-allowed");

        let invalid_get = request(Method::GET, "/api/v1/s/id/r/0/enrichments");
        let error = guard(&state, &invalid_get).unwrap_err();
        assert_eq!(error.status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[test]
    fn request_bounds_and_security_headers_fail_closed() {
        let state = state();
        let long_uri = format!("/{}", "x".repeat(MAX_URI_BYTES));
        let request = request(Method::GET, &long_uri);
        assert_eq!(
            guard(&state, &request).unwrap_err().status,
            StatusCode::URI_TOO_LONG
        );

        let mut headers = axum::http::HeaderMap::new();
        apply_security_headers(&mut headers);
        assert_eq!(headers[CACHE_CONTROL], "no-store");
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(headers["referrer-policy"], "no-referrer");
        assert_eq!(headers["cross-origin-resource-policy"], "same-origin");
        assert_eq!(headers["cross-origin-opener-policy"], "same-origin");
        assert!(
            headers[CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("frame-ancestors 'none'")
        );
        assert!(!headers.contains_key("access-control-allow-origin"));
    }

    fn options(listen: &str) -> ServeOptions {
        ServeOptions {
            listen: listen.parse().unwrap(),
            policy: ResourcePolicy::new(1024, 1024, 1, 1, 1024).unwrap(),
        }
    }

    #[test]
    fn remote_http_requires_the_explicit_ipv4_wildcard_listener() {
        assert!(validate_listener(&options("127.0.0.1:8787")).is_ok());
        assert!(validate_listener(&options("0.0.0.0:8787")).is_ok());

        for rejected in ["192.0.2.10:8787", "[::]:8787"] {
            assert!(matches!(
                validate_listener(&options(rejected)),
                Err(ServeError::RemoteWildcardRequired)
            ));
        }
    }

    #[test]
    fn wildcard_listener_urls_are_sorted_deduplicated_and_family_matched() {
        let urls = listener_urls(
            "0.0.0.0:8080".parse().unwrap(),
            vec![
                "192.168.4.2".parse().unwrap(),
                "10.88.0.1".parse().unwrap(),
                "127.0.0.1".parse().unwrap(),
                "192.168.4.2".parse().unwrap(),
                "::1".parse().unwrap(),
                "224.0.0.1".parse().unwrap(),
                "0.0.0.0".parse().unwrap(),
            ],
        );

        assert_eq!(
            urls,
            [
                "http://10.88.0.1:8080",
                "http://127.0.0.1:8080",
                "http://192.168.4.2:8080",
            ]
        );
    }

    #[test]
    fn concrete_and_ipv6_listener_urls_use_clickable_socket_syntax() {
        assert_eq!(
            listener_urls("127.0.0.1:8080".parse().unwrap(), Vec::new()),
            ["http://127.0.0.1:8080"]
        );
        assert_eq!(
            listener_urls("[::1]:8080".parse().unwrap(), Vec::new()),
            ["http://[::1]:8080"]
        );
    }

    #[test]
    fn browser_starts_directly_without_a_credential_gate() {
        assert!(!APP_JS.contains("Authorization"));
        assert!(!APP_JS.contains("Bearer"));
        assert!(!INDEX_HTML.contains("unlockForm"));
        assert!(!INDEX_HTML.contains("Bearer"));
        assert!(!APP_CSS.contains("#unlock"));
        assert!(APP_JS.contains("async function start()"));
        assert!(APP_JS.contains("start()"));
    }

    #[test]
    fn browser_preserves_the_structured_reason_for_a_conflict() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let response = error_response(StatusCode::CONFLICT, "base-revision-unusable");
            let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

            assert_eq!(document["error"]["code"], "base-revision-unusable");
            assert_eq!(
                document["error"]["message"],
                "This inspection revision cannot be enriched because a newer revision exists or the snapshot was invalidated. Reload the latest revision and try again."
            );

            let response = stale_revision_response(4, 5);
            let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                document["error"]["message"],
                "This page inspection started at revision 4, but another inspection already published revision 5. Reload the latest revision and try again."
            );

            let response = invalidated_snapshot_response();
            let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                document["error"]["message"],
                "The source volumes changed after this inspection started, so the snapshot was invalidated and cannot be enriched. Restart Volmap against a stable snapshot."
            );
        });

        assert!(APP_JS.contains("payload.error.message"));
        assert!(APP_JS.contains("error.status"));
        assert!(APP_JS.contains("error.code"));
        assert!(APP_JS.contains("Could not complete this view"));
        assert!(APP_CSS.contains(".error-note"));
    }

    #[test]
    fn browser_contract_exposes_full_volume_sector_mosaic() {
        assert!(APP_JS.contains("map.id='volumeMap'"));
        assert!(APP_JS.contains("Unreserved"));
        assert!(APP_JS.contains("Reserved, unallocated"));
        assert!(APP_JS.contains("Allocated"));
        assert!(APP_JS.contains("System metadata"));
        assert!(APP_JS.contains("Finding outline"));
        assert!(APP_CSS.contains("grid-template-columns:repeat(8,1fr)"));
        assert!(APP_CSS.contains(".page.unreserved"));
        assert!(APP_CSS.contains(".page.reserved-unallocated"));
        assert!(APP_CSS.contains(".page.allocated"));
        assert!(APP_CSS.contains(".page.allocated.occupancy-known"));
        assert!(APP_CSS.contains(".page.allocated.occupancy-unknown"));
        assert!(APP_CSS.contains("var(--occupied)"));
        assert!(APP_CSS.contains(".page.system-metadata"));
        assert!(APP_CSS.contains(".page.finding"));
        assert!(APP_JS.contains("/sectors/${currentVolume.vol_id}"));
        assert!(APP_JS.contains("next_cursor"));
        assert!(APP_JS.contains("pages.length!==64"));
        assert!(APP_JS.contains("function applyPageFill("));
        assert!(APP_JS.contains("page.occupancy.occupied_percent"));
        assert!(APP_JS.contains("applyPageFill(node,page)"));
    }

    #[test]
    fn browser_contract_replaces_workspace_for_sector_and_page_drilldown() {
        assert!(INDEX_HTML.contains("id=\"drillBreadcrumb\""));
        assert!(INDEX_HTML.contains("id=\"workspaceContent\""));
        assert!(APP_CSS.contains(".sector-focus-grid"));
        assert!(APP_CSS.contains(".page-workspace"));
        assert!(APP_CSS.contains(".slot-table"));
        assert!(APP_JS.contains("function showSector("));
        assert!(APP_JS.contains("function showVolume("));
        assert!(APP_JS.contains("async function showPage("));
        assert!(APP_JS.contains("renderPageWorkspace"));
        assert!(APP_JS.contains("deep.state==='not-enriched'"));
        assert!(APP_JS.contains("slot.offset"));
        assert!(APP_JS.contains("slot.length"));
        assert!(APP_JS.contains("renderSlottedDistribution"));
        assert!(APP_JS.contains("64 physical pages"));
    }

    #[test]
    fn browser_contract_uses_revision_pinned_canonical_history() {
        assert!(APP_JS.contains("function parseBrowserRoute("));
        assert!(APP_JS.contains("function syncBrowserRoute("));
        assert!(APP_JS.contains("history.pushState"));
        assert!(APP_JS.contains("history.replaceState"));
        assert!(APP_JS.contains("popstate"));
        assert!(APP_JS.contains("route.snapshot!==session.snapshot.id"));
        assert!(APP_JS.contains("session.snapshot.revision=route.revision"));
        assert!(APP_JS.contains("await restoreBrowserRoute(route)"));
        assert!(APP_JS.contains("showPage(route.page,true,'none')"));
        assert!(!APP_JS.contains("token=${"));
    }

    #[test]
    fn browser_contract_renders_complete_slotted_page_distribution() {
        assert!(DISTRIBUTION_CSS.contains(".page-distribution"));
        assert!(DISTRIBUTION_CSS.contains(".region-fragmented-free"));
        assert!(DISTRIBUTION_CSS.contains(".slot-entry.unallocated"));
        assert!(DISTRIBUTION_CSS.contains(".slot-entry.deleted"));
        assert!(APP_JS.contains("distribution.free_regions"));
        assert!(APP_JS.contains("distribution.slot_entries"));
        assert!(APP_JS.contains("Full slotted-page distribution"));
        assert!(APP_JS.contains("not allocated"));
        assert!(APP_JS.contains("Number(slot.offset)>0&&slot.record_type==='home'"));
        assert!(!APP_JS.contains("width/16384"));
    }

    #[test]
    fn slotted_page_distribution_covers_records_free_space_and_directory_entries() {
        use crate::format::{IO_PAGE_SIZE, PageType, decode_page_envelope, decode_slotted_page};

        let mut bytes = [0_u8; IO_PAGE_SIZE];
        bytes[8..12].copy_from_slice(&7_i32.to_le_bytes());
        bytes[12..14].copy_from_slice(&1_i16.to_le_bytes());
        bytes[14] = PageType::Heap.ordinal();
        let user = &mut bytes[32..IO_PAGE_SIZE - 8];
        user[0..2].copy_from_slice(&4_i16.to_le_bytes());
        user[2..4].copy_from_slice(&2_i16.to_le_bytes());
        user[4..6].copy_from_slice(&1_i16.to_le_bytes());
        user[6..8].copy_from_slice(&8_u16.to_le_bytes());
        user[8..12].copy_from_slice(&16_256_i32.to_le_bytes());
        user[12..16].copy_from_slice(&16_200_i32.to_le_bytes());
        user[16..20].copy_from_slice(&128_i32.to_le_bytes());
        for (slot, offset, length, kind) in [
            (0_usize, 32_u16, 24_u16, 2_u8),
            (1, 0, 0, 9),
            (2, 0, 48, 6),
            (3, 80, 16, 3),
        ] {
            let word = u32::from(offset) | (u32::from(length) << 14) | (u32::from(kind) << 28);
            let start = DB_PAGE_SIZE - 4 * (slot + 1);
            user[start..start + 4].copy_from_slice(&word.to_le_bytes());
        }
        let envelope = decode_page_envelope(
            &bytes,
            Vpid::new(VolId::new(1).unwrap(), PageId::new(7).unwrap()),
        )
        .unwrap();
        let slotted = decode_slotted_page(&envelope).unwrap();

        let PageDistributionProjection::Available {
            content_size,
            header,
            record_extents,
            free_regions,
            slot_directory,
            slot_entries,
            allocated_record_bytes,
            unoccupied_bytes,
        } = page_distribution_projection(&slotted)
        else {
            panic!("slotted page must have a distribution");
        };

        assert_eq!(content_size, 16_344);
        assert_eq!((header.offset, header.length), (0, 32));
        assert_eq!(
            record_extents
                .iter()
                .map(|record| (record.slot_id, record.offset, record.length))
                .collect::<Vec<_>>(),
            vec![(0, 32, 24), (3, 80, 16)]
        );
        assert_eq!(
            free_regions
                .iter()
                .map(|region| (region.offset, region.length, region.kind))
                .collect::<Vec<_>>(),
            vec![
                (56, 24, "fragmented-free"),
                (96, 32, "fragmented-free"),
                (128, 16_200, "contiguous-free"),
            ]
        );
        assert_eq!((slot_directory.offset, slot_directory.length), (16_328, 16));
        assert_eq!(
            slot_entries
                .iter()
                .map(|entry| (entry.slot_id, entry.offset, entry.state))
                .collect::<Vec<_>>(),
            vec![
                (0, 16_340, "allocated"),
                (1, 16_336, "unallocated"),
                (2, 16_332, "deleted"),
                (3, 16_328, "allocated"),
            ]
        );
        assert_eq!(allocated_record_bytes, 40);
        assert_eq!(unoccupied_bytes, 16_256);
        assert_eq!(32 + 40 + 16_256 + 16, content_size);
    }

    #[test]
    fn sector_collection_window_is_bounded_and_complete() {
        assert_eq!(sector_collection_window(130, 0, 24), Some((0, 24)));
        assert_eq!(sector_collection_window(130, 120, 24), Some((120, 130)));
        assert_eq!(sector_collection_window(130, 130, 24), Some((130, 130)));
        assert_eq!(sector_collection_window(130, 131, 24), None);
        assert_eq!(sector_collection_window(130, 0, 0), None);
        assert_eq!(
            sector_collection_window(130, 0, MAX_SECTOR_COLLECTION_LIMIT + 1),
            None
        );
    }

    #[test]
    fn collection_cursor_is_opaque_session_keyed_and_revision_bound() {
        let state = state();
        let mut overview = test_overview();
        let cursor = encode_cursor(&state, &overview, "volumes", 100);

        assert_eq!(cursor.len(), 80);
        assert_eq!(
            decode_cursor(&state, &overview, "volumes", &cursor),
            Some(100)
        );
        assert_eq!(decode_cursor(&state, &overview, "coverage", &cursor), None);

        overview.revision = crate::model::InspectionRevision::new(1);
        assert_eq!(decode_cursor(&state, &overview, "volumes", &cursor), None);

        let mut tampered = cursor.into_bytes();
        tampered[0] = if tampered[0] == b'0' { b'1' } else { b'0' };
        let tampered = String::from_utf8(tampered).unwrap();
        assert_eq!(
            decode_cursor(&state, &test_overview(), "volumes", &tampered),
            None
        );
        assert_eq!(DEFAULT_COLLECTION_LIMIT, 100);
        assert_eq!(MAX_COLLECTION_LIMIT, 512);
    }

    #[test]
    fn collection_projection_enforces_limits_and_continuation() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let state = state();
            let overview = test_overview();
            let response = collection_response(
                &state,
                &overview,
                "test",
                Ok(Query(CollectionQuery {
                    cursor: None,
                    limit: None,
                })),
                (0_u16..101).collect(),
            );
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(document["data"]["items"].as_array().unwrap().len(), 100);
            let cursor = document["data"]["next_cursor"]["value"]
                .as_str()
                .unwrap()
                .to_owned();

            let response = collection_response(
                &state,
                &overview,
                "test",
                Ok(Query(CollectionQuery {
                    cursor: Some(cursor),
                    limit: None,
                })),
                (0_u16..101).collect(),
            );
            let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(document["data"]["items"].as_array().unwrap().len(), 1);
            assert_eq!(document["data"]["next_cursor"]["state"], "end");

            let response = collection_response(
                &state,
                &overview,
                "test",
                Ok(Query(CollectionQuery {
                    cursor: None,
                    limit: Some(MAX_COLLECTION_LIMIT + 1),
                })),
                Vec::<u8>::new(),
            );
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        });
    }

    fn test_overview() -> crate::inspection::OverviewView {
        crate::inspection::OverviewView {
            snapshot_id: crate::model::SnapshotId::from_bytes([7; 16]),
            revision: crate::model::InspectionRevision::new(0),
            validity: crate::model::SnapshotValidity::Valid,
            format_profile: "test",
            input_kind: "test",
            outcome: crate::diagnostics::InspectionOutcome::SuccessLimited,
            volume_count: 1,
            sector_count: 1,
            reserved_sector_count: 1,
            physical_page_count: 64,
            inspected_page_envelopes: 64,
            page_type_counts: Vec::new(),
            tde_opaque_pages: 0,
            coverage: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn terminal_invalidation_overlays_old_revision_facts_once() {
        let mut overview = test_overview();
        apply_terminal_invalidation(&mut overview, false);
        assert_eq!(overview.validity, crate::model::SnapshotValidity::Valid);

        apply_terminal_invalidation(&mut overview, true);
        apply_terminal_invalidation(&mut overview, true);
        assert_eq!(
            overview.validity,
            crate::model::SnapshotValidity::Invalidated
        );
        assert_eq!(
            overview.outcome,
            crate::diagnostics::InspectionOutcome::Fatal
        );
        assert_eq!(
            overview
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "snapshot.modified")
                .count(),
            1
        );
    }
}
