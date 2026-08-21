//! Read-only HTTP adapter with embedded Atlas assets.

mod assets;

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::uri::Authority;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Semaphore};

use crate::follow::{FollowConfig, LiveSource, Reading, WATCH_TIMEOUT};
use crate::format::{DB_PAGE_SIZE, SlottedPage};
use crate::inspection::{
    CancelToken, DiagnosticRecord, GraphView, OpenRequest, QueryError, ResourcePolicy,
};
use crate::model::{FileId, Oid, PageId, SectorId, SlotId, Vfid, VolId, Vpid};
use crate::projection::{
    CoverageProjection, DeepPageProjection, DiagnosticProjection, OosChainProjection,
    PageProjection, SCHEMA_NAME, SCHEMA_VERSION, SlotProjection, SnapshotProjection,
    class_representation_projection, coverage_projection, deep_page_projection,
    diagnostic_projection, file_header_projection, oos_chain_projection, outcome_name,
    page_projection, record_interpretation_projection, relocation_edge_projection,
    sector_projection, slot_projection, snapshot_id_hex, summary_projection, volume_projection,
};

const MAX_URI_BYTES: usize = 8192;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_HEADER_FIELDS: usize = 64;
const MAX_JSON_BYTES: usize = 64 * 1024;
const MAX_CONCURRENT_REQUESTS: usize = 32;
/// Long-poll waiters consume no inspection work, so they are admitted apart
/// from the inspection concurrency limit and cannot starve real requests.
const MAX_CONCURRENT_WATCHERS: usize = 64;
const WATCH_PATH: &str = "/api/v1/live/watch";
const DEFAULT_COLLECTION_LIMIT: usize = 100;
const MAX_COLLECTION_LIMIT: usize = 512;
const DEFAULT_SECTOR_COLLECTION_LIMIT: usize = 24;
const MAX_SECTOR_COLLECTION_LIMIT: usize = 64;

#[derive(Clone, Debug)]
pub struct ServeOptions {
    pub listen: SocketAddr,
    pub policy: ResourcePolicy,
    /// How the input was opened, so a follower can read it again.
    pub request: OpenRequest,
    /// `Some` to follow a live input, `None` to hold one immutable reading.
    pub follow: Option<FollowConfig>,
}

#[derive(Clone)]
struct WebState {
    source: Arc<LiveSource>,
    enrichment: Arc<Mutex<()>>,
    policy: ResourcePolicy,
    cursor_key: Arc<[u8; 32]>,
    authority: Option<Arc<str>>,
    semaphore: Arc<Semaphore>,
    watchers: Arc<Semaphore>,
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
    let config = options.follow.unwrap_or_default();
    let source = LiveSource::new(view, config, options.follow.is_some());
    let state = WebState {
        source: Arc::clone(&source),
        enrichment: Arc::new(Mutex::new(())),
        policy: options.policy,
        cursor_key,
        authority: (!options.listen.ip().is_unspecified()).then(|| Arc::from(local.to_string())),
        semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
        watchers: Arc::new(Semaphore::new(MAX_CONCURRENT_WATCHERS)),
    };
    if options.follow.is_some() {
        tokio::spawn(crate::follow::follow(
            source,
            options.request.clone(),
            options.policy,
        ));
    }
    let router = build_router(state);
    print_listener_urls(local);
    print_follow_state(options.follow.as_ref());
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

fn print_follow_state(follow: Option<&FollowConfig>) {
    match follow {
        Some(config) => eprintln!(
            "Following the input every {} ms; a change publishes a new generation.",
            config.poll_interval.as_millis()
        ),
        None => {
            eprintln!("Holding one immutable reading; a changed input invalidates this session.");
        }
    }
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
        .route("/", get(assets::index))
        .route("/app.css", get(assets::css))
        .route("/distribution.css", get(assets::distribution_css))
        .route("/routes.js", get(assets::routes_javascript))
        .route("/distribution.js", get(assets::distribution_javascript))
        .route("/app.js", get(assets::javascript))
        // A browser location names an entity, not a reading of one. The reading
        // it resolves to is whichever generation is current when it is asked
        // for, which is what keeps a copied link working while a database runs.
        .route("/volume/{vol}", get(assets::index))
        .route("/file/{vol}/{file}", get(assets::index))
        .route("/sector/{vol}/{sector}", get(assets::index))
        .route("/page/{vol}/{page}", get(assets::index))
        .route("/slot/{vol}/{page}/{slot}", get(assets::index))
        .route("/oos/{vol}/{page}/{slot}", get(assets::index))
        .route("/api/v1/session", get(session))
        .route("/api/v1/licenses", get(licenses))
        .route("/api/v1/live/watch", get(watch))
        .route("/api/v1/overview", get(overview))
        .route("/api/v1/volumes", get(volumes))
        .route("/api/v1/sectors/{vol}", get(sectors))
        .route("/api/v1/relationships", get(relationships))
        .route("/api/v1/diagnostics", get(diagnostics))
        .route("/api/v1/coverage", get(coverage))
        .route("/api/v1/file/{vol}/{file}", get(file))
        .route("/api/v1/sector/{vol}/{sector}", get(sector))
        .route("/api/v1/page/{vol}/{page}", get(page))
        .route("/api/v1/slot/{vol}/{page}/{slot}", get(slot))
        .route("/api/v1/oos/{vol}/{page}/{slot}", get(oos))
        .route("/api/v1/enrichments", post(enrich))
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
    let pool = if request.uri().path() == WATCH_PATH {
        state.watchers.clone()
    } else {
        state.semaphore.clone()
    };
    let mut response = match guard(&state, &request) {
        Ok(()) => match pool.try_acquire_owned() {
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

#[derive(Clone, Copy, Debug)]
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

/// One request's answer: the reading it was served from, and that reading's
/// overview with its standing applied.
struct Answer {
    reading: Reading,
    overview: crate::inspection::OverviewView,
}

/// Resolves the reading a fresh request is answered from.
fn answer(state: &WebState) -> Result<Answer, GuardError> {
    let reading = state.source.current().map_err(|_| GuardError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "session-unavailable",
    })?;
    Ok(project(reading))
}

/// Resolves a reading a multi-request collection load is already bound to.
fn retained_answer(state: &WebState, generation: u64) -> Result<Option<Answer>, GuardError> {
    let reading = state.source.retained(generation).map_err(|_| GuardError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "session-unavailable",
    })?;
    Ok(reading.map(project))
}

fn project(reading: Reading) -> Answer {
    let mut overview = reading.view.overview();
    apply_standing(&mut overview, reading.validity);
    Answer { reading, overview }
}

/// Applies to one reading's facts the standing only the live session knows.
///
/// A reading cannot tell whether the input has moved past it, so the session
/// says so here. Superseded is not a failure and must not be reported as one:
/// the facts are exactly what was on disk, they are simply no longer current,
/// and the follower is already reading the input again.
fn apply_standing(
    overview: &mut crate::inspection::OverviewView,
    standing: crate::model::SnapshotValidity,
) {
    if standing != crate::model::SnapshotValidity::Superseded
        || overview.validity == crate::model::SnapshotValidity::Superseded
    {
        return;
    }
    overview.validity = crate::model::SnapshotValidity::Superseded;
    if !overview
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "snapshot.source_advanced")
    {
        overview.diagnostics.push(DiagnosticRecord {
            code: "snapshot.source_advanced",
            severity: "warning",
            message: "The input changed after this generation was read; a newer generation is being read.",
            subject: "snapshot".to_owned(),
            rule: "snapshot.file_stamp.stable",
        });
    }
}

#[derive(Serialize)]
struct SessionProjection {
    access: &'static str,
    follow: FollowProjection,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum FollowProjection {
    Following {
        poll_interval_ms: String,
        retained_generations: String,
    },
    Disabled,
}

fn follow_projection(state: &WebState) -> FollowProjection {
    if !state.source.following() {
        return FollowProjection::Disabled;
    }
    let config = state.source.config();
    FollowProjection::Following {
        poll_interval_ms: config.poll_interval.as_millis().to_string(),
        retained_generations: config.retain.to_string(),
    }
}

async fn session(State(state): State<WebState>) -> Response {
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    Json(api_envelope(
        &answer,
        SessionProjection {
            access: "unauthenticated-http",
            follow: follow_projection(&state),
        },
    ))
    .into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WatchQuery {
    generation: Option<u64>,
}

#[derive(Serialize)]
struct WatchProjection {
    /// Whether the generation moved past the one the caller already had.
    advanced: bool,
    follow: FollowProjection,
}

/// Long-polls until the current generation differs from the caller's.
///
/// This is a poll the caller does not have to repeat on a timer: it returns the
/// moment a re-read is published, and otherwise reports no change once the
/// waiting window closes. It is deliberately not a server-sent-event stream —
/// the release dependency graph is pinned, and a plain wait needs nothing new
/// in it.
async fn watch(
    State(state): State<WebState>,
    query: Result<Query<WatchQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let mut receiver = state.source.subscribe();
    let known = query.generation;
    if state.source.following() && known.is_some_and(|value| value == *receiver.borrow_and_update())
    {
        // Nothing to report yet, so hold the request open instead of asking the
        // caller to come back and ask again.
        let _ = tokio::time::timeout(WATCH_TIMEOUT, receiver.changed()).await;
    }
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    Json(api_envelope(
        &answer,
        WatchProjection {
            advanced: known.is_none_or(|value| value != answer.reading.generation),
            follow: follow_projection(&state),
        },
    ))
    .into_response()
}

async fn overview(State(state): State<WebState>) -> Response {
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    Json(api_envelope(&answer, summary_projection(&answer.overview))).into_response()
}

async fn volumes(
    State(state): State<WebState>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let (answer, offset) = match collection_start(&state, "volumes", query.cursor.as_deref()) {
        Ok(value) => value,
        Err(refusal) => return refusal.response(),
    };
    let data = answer
        .reading
        .view
        .volumes()
        .into_iter()
        .map(volume_projection)
        .collect();
    collection_response(
        &state,
        &answer,
        "volumes",
        offset,
        query.limit.unwrap_or(DEFAULT_COLLECTION_LIMIT),
        data,
    )
}

async fn sectors(
    State(state): State<WebState>,
    Path(vol): Path<i16>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let cursor_kind = format!("sectors:{vol}");
    let (answer, offset) = match collection_start(&state, &cursor_kind, query.cursor.as_deref()) {
        Ok(value) => value,
        Err(refusal) => return refusal.response(),
    };
    let view = &answer.reading.view;
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
            value: encode_cursor(&state, answer.reading.generation, &cursor_kind, end),
        }
    } else {
        NextCursorProjection::End
    };
    Json(api_envelope(
        &answer,
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
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let (answer, offset) = match collection_start(&state, "relationships", query.cursor.as_deref())
    {
        Ok(value) => value,
        Err(refusal) => return refusal.response(),
    };
    let view = &answer.reading.view;
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
    collection_response(
        &state,
        &answer,
        "relationships",
        offset,
        query.limit.unwrap_or(DEFAULT_COLLECTION_LIMIT),
        data,
    )
}

async fn diagnostics(
    State(state): State<WebState>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let (answer, offset) = match collection_start(&state, "diagnostics", query.cursor.as_deref()) {
        Ok(value) => value,
        Err(refusal) => return refusal.response(),
    };
    let data = answer
        .overview
        .diagnostics
        .iter()
        .cloned()
        .map(diagnostic_projection)
        .collect();
    collection_response(
        &state,
        &answer,
        "diagnostics",
        offset,
        query.limit.unwrap_or(DEFAULT_COLLECTION_LIMIT),
        data,
    )
}

async fn coverage(
    State(state): State<WebState>,
    query: Result<Query<CollectionQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-query");
    };
    let (answer, offset) = match collection_start(&state, "coverage", query.cursor.as_deref()) {
        Ok(value) => value,
        Err(refusal) => return refusal.response(),
    };
    let data = answer
        .overview
        .coverage
        .iter()
        .copied()
        .map(coverage_projection)
        .collect();
    collection_response(
        &state,
        &answer,
        "coverage",
        offset,
        query.limit.unwrap_or(DEFAULT_COLLECTION_LIMIT),
        data,
    )
}

/// Resolves the reading a collection load continues on.
///
/// A load that spans several requests finishes on the generation it started on,
/// so a mosaic is never stitched together from two different readings. Once
/// that generation falls out of the retention window the load cannot be
/// continued honestly, and the caller is told to restart it rather than handed
/// a silent seam.
fn collection_start(
    state: &WebState,
    kind: &str,
    cursor: Option<&str>,
) -> Result<(Answer, usize), CollectionRefusal> {
    let Some(cursor) = cursor else {
        return answer(state)
            .map(|answer| (answer, 0))
            .map_err(CollectionRefusal::Unavailable);
    };
    let Some(payload) = decode_cursor(state, kind, cursor) else {
        return Err(CollectionRefusal::Unrecognised);
    };
    let retained =
        retained_answer(state, payload.generation).map_err(CollectionRefusal::Unavailable)?;
    let Some(answer) = retained else {
        return Err(CollectionRefusal::Evicted(payload.generation));
    };
    Ok((answer, payload.offset))
}

/// Why a collection load cannot start.
#[derive(Clone, Copy, Debug)]
enum CollectionRefusal {
    /// The cursor was not issued by this session.
    Unrecognised,
    /// The cursor is this session's, but names a generation it has read past.
    Evicted(u64),
    /// The session state could not be read at all.
    Unavailable(GuardError),
}

impl CollectionRefusal {
    fn response(self) -> Response {
        match self {
            Self::Unrecognised => error_response(StatusCode::BAD_REQUEST, "invalid-cursor"),
            Self::Evicted(generation) => cursor_generation_changed_response(generation),
            Self::Unavailable(error) => error_response(error.status, error.code),
        }
    }
}

fn collection_response<T: Serialize>(
    state: &WebState,
    answer: &Answer,
    kind: &str,
    offset: usize,
    limit: usize,
    items: Vec<T>,
) -> Response {
    if limit == 0 || limit > MAX_COLLECTION_LIMIT {
        return error_response(StatusCode::BAD_REQUEST, "invalid-collection-limit");
    }
    if offset > items.len() {
        return error_response(StatusCode::BAD_REQUEST, "invalid-cursor");
    }
    let total = items.len();
    let end = offset.saturating_add(limit).min(total);
    let page = items.into_iter().skip(offset).take(end - offset).collect();
    let next_cursor = if end < total {
        NextCursorProjection::Present {
            value: encode_cursor(state, answer.reading.generation, kind, end),
        }
    } else {
        NextCursorProjection::End
    };
    Json(api_envelope(
        answer,
        CollectionProjection {
            items: page,
            next_cursor,
        },
    ))
    .into_response()
}

/// The authenticated part of a cursor: a generation and an offset, each a
/// little-endian `u64`.
const CURSOR_PAYLOAD_BYTES: usize = 16;
const CURSOR_MAC_BYTES: usize = 32;
/// A cursor is its payload and tag, hex encoded. Deriving the length here keeps
/// the decoder from drifting out of step with the payload it is decoding.
const CURSOR_HEX_LEN: usize = (CURSOR_PAYLOAD_BYTES + CURSOR_MAC_BYTES) * 2;

/// Where a collection load stands: the generation it reads and how far it got.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CursorPayload {
    generation: u64,
    offset: usize,
}

fn encode_cursor(state: &WebState, generation: u64, kind: &str, offset: usize) -> String {
    let mut payload = [0_u8; CURSOR_PAYLOAD_BYTES];
    payload[..8].copy_from_slice(&generation.to_le_bytes());
    payload[8..].copy_from_slice(&u64::try_from(offset).unwrap_or(u64::MAX).to_le_bytes());
    let mac = cursor_mac(state, kind, &payload);
    hex_encode(&payload.into_iter().chain(mac).collect::<Vec<_>>())
}

/// Reads a cursor this session issued, or `None` if it did not issue it.
///
/// The generation travels inside the authenticated payload rather than the MAC
/// key so that a cursor from a generation this session has since read past is
/// still recognisably its own, and can be answered as stale rather than as a
/// forgery.
fn decode_cursor(state: &WebState, kind: &str, cursor: &str) -> Option<CursorPayload> {
    let bytes = hex_decode(cursor)?;
    let (payload, supplied_mac) = bytes.split_at_checked(CURSOR_PAYLOAD_BYTES)?;
    if supplied_mac.len() != CURSOR_MAC_BYTES {
        return None;
    }
    let expected_mac = cursor_mac(state, kind, payload);
    if !bool::from(supplied_mac.ct_eq(expected_mac.as_slice())) {
        return None;
    }
    let generation = u64::from_le_bytes(payload.get(..8)?.try_into().ok()?);
    let offset = u64::from_le_bytes(payload.get(8..)?.try_into().ok()?);
    Some(CursorPayload {
        generation,
        offset: usize::try_from(offset).ok()?,
    })
}

fn cursor_mac(state: &WebState, kind: &str, payload: &[u8]) -> [u8; 32] {
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
    if value.len() != CURSOR_HEX_LEN || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
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

async fn file(State(state): State<WebState>, Path((vol, file)): Path<(i16, i32)>) -> Response {
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let view = &answer.reading.view;
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
    Json(api_envelope(&answer, file_header_projection(header))).into_response()
}

async fn sector(State(state): State<WebState>, Path((vol, sector)): Path<(i16, i32)>) -> Response {
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let view = &answer.reading.view;
    let result = VolId::new(vol)
        .ok()
        .zip(SectorId::new(sector).ok())
        .ok_or(QueryError::EntityNotFound)
        .and_then(|(vol_id, sector_id)| view.sector(vol_id, sector_id));
    match result {
        Ok(value) => Json(api_envelope(&answer, sector_projection(value))).into_response(),
        Err(_) => error_response(StatusCode::NOT_FOUND, "entity-not-found"),
    }
}

async fn page(State(state): State<WebState>, Path((vol, page)): Path<(i16, i32)>) -> Response {
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let view = &answer.reading.view;
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
                &answer,
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
    Path((vol, page, slot)): Path<(i16, i32, i16)>,
) -> Response {
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let view = &answer.reading.view;
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
    // A relocation's values live in its target, so report the target's
    // interpretation alongside the forward reference rather than nothing.
    let relocation_edge = view.relocation_edge(oid);
    let interpreted = view.slot_interpretation(oid);
    let class_representation = interpreted
        .as_ref()
        .and_then(|interpretation| {
            view.class_representation(interpretation.class_oid, interpretation.representation_id)
        })
        .map(class_representation_projection);
    Json(api_envelope(
        &answer,
        SlotResourceProjection {
            page: page_projection(page),
            deep: deep_page_projection(view.deep_page(vpid)),
            selected_slot: slot_projection(selected),
            relocation_edge: relocation_edge.map(relocation_edge_projection),
            interpretation: interpreted.map(record_interpretation_projection),
            class_representation,
            interpretation_unavailable: view.record_page_interpretation_failure(vpid),
        },
    ))
    .into_response()
}

#[derive(Serialize)]
struct SlotResourceProjection {
    page: PageProjection,
    deep: DeepPageProjection,
    selected_slot: SlotProjection,
    /// Absent until the slot's page has been enriched; the panel then offers
    /// the enrichment rather than showing an empty interpretation.
    relocation_edge: Option<crate::projection::RelocationEdgeProjection>,
    interpretation: Option<crate::projection::RecordInterpretationProjection>,
    class_representation: Option<crate::projection::ClassRepresentationProjection>,
    /// Set when interpretation was requested for this page and degraded as a
    /// whole — a root or system heap, an unreadable class record, an encrypted
    /// page. The panel states the reason instead of silently offering the
    /// enrichment again.
    interpretation_unavailable: Option<&'static str>,
}

async fn oos(
    State(state): State<WebState>,
    Path((vol, page, slot)): Path<(i16, i32, i16)>,
) -> Response {
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let view = &answer.reading.view;
    let Some(oid) = parse_web_oid(vol, page, slot) else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    let Some(chain) = view.oos_chain(oid) else {
        return error_response(StatusCode::NOT_FOUND, "entity-not-found");
    };
    Json(api_envelope(
        &answer,
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
    /// Interpret the records of the page holding this slot. Page granularity is
    /// the unit: one click resolves the class record once and interprets every
    /// home record it covers.
    Record(Oid),
}

#[derive(Serialize)]
struct EnrichmentProjection {
    /// Whether the result was published as a new revision of the generation it
    /// was computed on, or discarded because a re-read overtook it. A discarded
    /// result is not an error: the entity path below still resolves, it simply
    /// answers from the newer reading, so the caller may ask again.
    retained: bool,
    result_revision: String,
    /// The live path the enriched entity is read back from.
    result: String,
}

async fn enrich(
    State(state): State<WebState>,
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
    let opened = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let base = &opened.reading.view;
    if opened.overview.validity == crate::model::SnapshotValidity::Invalidated {
        return invalidated_snapshot_response();
    }
    let Some(target) = parse_enrichment_target(&request.selector) else {
        return error_response(StatusCode::BAD_REQUEST, "invalid-selector");
    };
    let cancel = CancelToken::new();
    let enriched = match run_enrichment(base, target, state.policy, &cancel) {
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
    if let EnrichmentTarget::Slot(oid) | EnrichmentTarget::Record(oid) = target {
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
    let result_revision = enriched.overview().revision.get();
    let result = target_result_path(target);
    let generation = opened.reading.generation;
    let Ok(retained) = state.source.publish_revision(generation, enriched) else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "session-unavailable");
    };
    // Re-resolve so the envelope reports the reading the result actually landed
    // in, which is the newer generation if a re-read overtook the enrichment.
    let answer = match answer(&state) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    Json(api_envelope(
        &answer,
        EnrichmentProjection {
            retained,
            result_revision: result_revision.to_string(),
            result,
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
        ["record", vol, page, slot] => Some(EnrichmentTarget::Record(Oid::new(
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

/// The live path an enrichment result is read back from.
///
/// The path names the entity only. A generation is deliberately absent: the
/// caller reads the entity again and is answered from whatever reading is
/// current, which is the behaviour that keeps a copied link working.
fn target_result_path(target: EnrichmentTarget) -> String {
    match target {
        EnrichmentTarget::Page(vpid) => {
            format!("/page/{}/{}", vpid.vol_id.get(), vpid.page_id.get())
        }
        // An interpretation is read back through the slot it was requested for,
        // so both selectors resolve to the same resource.
        EnrichmentTarget::Slot(oid) | EnrichmentTarget::Record(oid) => format!(
            "/slot/{}/{}/{}",
            oid.vol_id.get(),
            oid.page_id.get(),
            oid.slot_id.get()
        ),
        EnrichmentTarget::Oos(oid) => format!(
            "/oos/{}/{}/{}",
            oid.vol_id.get(),
            oid.page_id.get(),
            oid.slot_id.get()
        ),
    }
}

fn run_enrichment(
    base: &crate::inspection::GraphView,
    target: EnrichmentTarget,
    policy: crate::inspection::ResourcePolicy,
    cancel: &CancelToken,
) -> Result<crate::inspection::GraphView, crate::inspection::OperationError> {
    match target {
        EnrichmentTarget::Page(vpid) => base.enrich_page(vpid, policy, cancel),
        // Selecting a slot establishes its structure and, for a relocation, the
        // edge it carries — the same facts `volmap inspect slot:` establishes.
        EnrichmentTarget::Slot(oid) => enrich_slot_selection(base, oid, policy, cancel),
        EnrichmentTarget::Oos(oid) => base.enrich_oos(oid, policy, cancel),
        EnrichmentTarget::Record(oid) => enrich_record_selection(base, oid, policy, cancel),
    }
}

/// Establishes one slot's structure, and its relocation edge when it has one.
fn enrich_slot_selection(
    base: &crate::inspection::GraphView,
    oid: Oid,
    policy: crate::inspection::ResourcePolicy,
    cancel: &CancelToken,
) -> Result<crate::inspection::GraphView, crate::inspection::OperationError> {
    let vpid = Vpid::new(oid.vol_id, oid.page_id);
    let view = base.enrich_page(vpid, policy, cancel)?;
    if slot_is_relocation(&view, oid) {
        return view.enrich_relocation(oid, policy, cancel);
    }
    Ok(view)
}

fn slot_is_relocation(view: &crate::inspection::GraphView, oid: Oid) -> bool {
    view.deep_page(Vpid::new(oid.vol_id, oid.page_id))
        .and_then(|deep| deep.slotted)
        .and_then(|slotted| {
            usize::try_from(oid.slot_id.get())
                .ok()
                .and_then(|index| slotted.slots().get(index).copied())
        })
        .is_some_and(|slot| slot.record_type() == crate::format::RecordType::Relocation)
}

/// Interprets the page holding `oid`, first making the slot's own structure and
/// any relocation edge available so a relocated record's values are reachable.
fn enrich_record_selection(
    base: &crate::inspection::GraphView,
    oid: Oid,
    policy: crate::inspection::ResourcePolicy,
    cancel: &CancelToken,
) -> Result<crate::inspection::GraphView, crate::inspection::OperationError> {
    let vpid = Vpid::new(oid.vol_id, oid.page_id);
    let view = enrich_slot_selection(base, oid, policy, cancel)?;
    let view = view.enrich_record_page(vpid, policy, cancel)?;
    // Follow the edge so the target page's records are interpreted too.
    match view.relocation_edge(oid).and_then(|edge| edge.target) {
        Some(target) => {
            view.enrich_record_page(Vpid::new(target.vol_id, target.page_id), policy, cancel)
        }
        None => Ok(view),
    }
}

fn api_envelope<T: Serialize>(answer: &Answer, data: T) -> ApiEnvelope<T> {
    let overview = &answer.overview;
    ApiEnvelope {
        schema: SCHEMA_NAME,
        schema_version: SCHEMA_VERSION,
        document_type: "resource",
        snapshot: SnapshotProjection {
            id: snapshot_id_hex(overview.snapshot_id),
            revision: overview.revision.get().to_string(),
            validity: crate::projection::validity_name(overview.validity),
            format_profile: overview.format_profile,
            generation: Some(answer.reading.generation.to_string()),
            observed_at_unix_seconds: Some(answer.reading.observed_at_unix_seconds.to_string()),
            input_modified_unix_seconds: answer
                .reading
                .input_modified_unix_seconds
                .map(|seconds| seconds.to_string()),
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

/// Answers a cursor whose generation has fallen out of the retention window.
///
/// This is distinct from `invalid-cursor`: the cursor is authentic, the session
/// simply cannot serve the reading it names any more, so the caller should
/// restart the load rather than treat its own state as corrupt.
fn cursor_generation_changed_response(requested_generation: u64) -> Response {
    error_response_with_message(
        StatusCode::CONFLICT,
        "cursor-generation-changed",
        format!(
            "This load started at generation {requested_generation}, which is no longer retained. Restart the load at the current generation."
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
            "This reading cannot be enriched because the snapshot was invalidated. Restart Volmap against a stable input, or serve it with follow enabled."
        }
        "cursor-generation-changed" => {
            "The generation this collection load started on is no longer retained. Restart the load at the current generation."
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
            "This collection cursor is invalid or was not issued by this Volmap session."
        }
        "invalid-host" => "The request Host does not match this Volmap listener.",
        "invalid-selector" => "The page, slot, or OOS selector is malformed.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};

    /// A session over one uninteresting volume, for the tests that exercise
    /// the request guards and cursor authenticity and never read a page. A real
    /// reading rather than an empty one, so the state under test is the state
    /// the handlers actually see.
    fn fixture_state(authority: Option<&str>) -> (FixtureDirectory, WebState) {
        use std::io::Write as _;

        let directory = FixtureDirectory::new();
        let volume = directory.path().join("guard");
        let vinf = directory.path().join("guard_vinf");
        fixture_write_volume(&volume, 0, &[]);
        let mut manifest = std::fs::File::create(&vinf).unwrap();
        writeln!(manifest, "0 {}", volume.display()).unwrap();
        drop(manifest);

        let request = OpenRequest {
            input: crate::source::InputSpec::Vinf {
                path: vinf,
                volume_root: None,
            },
            tde_keys_file: None,
            spill_directory: None,
        };
        let policy =
            ResourcePolicy::new(8 * 1024 * 1024, 1024 * 1024, 1, 64, 8 * 1024 * 1024).unwrap();
        let view = crate::inspection::Inspection::open(&request, policy, &CancelToken::new(), None)
            .unwrap()
            .view(crate::inspection::RevisionSelector::Latest)
            .unwrap();
        let state = WebState {
            source: LiveSource::new(view, FollowConfig::default(), false),
            enrichment: Arc::new(Mutex::new(())),
            policy,
            cursor_key: Arc::new([7_u8; 32]),
            authority: authority.map(Arc::from),
            semaphore: Arc::new(Semaphore::new(1)),
            watchers: Arc::new(Semaphore::new(1)),
        };
        (directory, state)
    }

    fn state() -> (FixtureDirectory, WebState) {
        fixture_state(Some("127.0.0.1:8787"))
    }

    fn wildcard_state() -> (FixtureDirectory, WebState) {
        fixture_state(None)
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
        let (_directory, state) = state();
        let request = request(Method::GET, "/api/v1/session");
        assert!(guard(&state, &request).is_ok());
    }

    #[test]
    fn wildcard_listener_accepts_any_valid_host_while_loopback_stays_exact() {
        let (_directory, state) = state();
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

        assert!(guard(&wildcard_state().1, &wrong_host).is_ok());

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
        let (_directory, state) = state();
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
        assert!(guard(&wildcard_state().1, &wildcard).is_ok());

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
        let (_directory, state) = state();
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
        let (_directory, state) = state();
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
            request: OpenRequest {
                input: crate::source::InputSpec::Vinf {
                    path: std::path::PathBuf::from("/nonexistent/volmap-listener-check"),
                    volume_root: None,
                },
                tde_keys_file: None,
                spill_directory: None,
            },
            follow: Some(FollowConfig::default()),
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

    const FIXTURE_TOTAL_SECTORS: u32 = 64;
    const FIXTURE_SECTOR_PAGES: u32 = 64;

    fn fixture_envelope_page(
        vol_id: i16,
        page_id: i32,
        page_type: crate::format::PageType,
    ) -> [u8; crate::format::IO_PAGE_SIZE] {
        let mut page = [0_u8; crate::format::IO_PAGE_SIZE];
        let lsa = u64::try_from(page_id).unwrap().to_le_bytes();
        page[0..8].copy_from_slice(&lsa);
        page[8..12].copy_from_slice(&page_id.to_le_bytes());
        page[12..14].copy_from_slice(&vol_id.to_le_bytes());
        page[14] = page_type.ordinal();
        page[crate::format::IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);
        page
    }

    fn fixture_header_page(vol_id: i16) -> [u8; crate::format::IO_PAGE_SIZE] {
        let mut page = fixture_envelope_page(vol_id, 0, crate::format::PageType::VolumeHeader);
        let user = &mut page[32..crate::format::IO_PAGE_SIZE - 8];
        user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
        user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
        user[28..30].copy_from_slice(&vol_id.to_le_bytes());
        user[40..44].copy_from_slice(&i32::try_from(FIXTURE_SECTOR_PAGES).unwrap().to_le_bytes());
        user[44..48].copy_from_slice(&i32::try_from(FIXTURE_TOTAL_SECTORS).unwrap().to_le_bytes());
        user[48..52].copy_from_slice(&i32::try_from(FIXTURE_TOTAL_SECTORS).unwrap().to_le_bytes());
        user[52..56].copy_from_slice(&(-1_i32).to_le_bytes());
        user[56..60].copy_from_slice(&1_i32.to_le_bytes());
        user[60..64].copy_from_slice(&1_i32.to_le_bytes());
        user[64..68].copy_from_slice(&1_i32.to_le_bytes());
        user[96..100].copy_from_slice(&(-1_i32).to_le_bytes());
        user[100..102].copy_from_slice(&(-1_i16).to_le_bytes());
        user[104..108].copy_from_slice(&(-1_i32).to_le_bytes());
        user[124..126].copy_from_slice(&(-1_i16).to_le_bytes());
        user[126..128].copy_from_slice(&0_i16.to_le_bytes());
        user[128..130].copy_from_slice(&1_i16.to_le_bytes());
        user[130..132].copy_from_slice(&2_i16.to_le_bytes());
        page
    }

    /// Writes one volume holding `pages` at their own page ids, reserving the
    /// sectors those pages fall in.
    fn fixture_write_volume(path: &std::path::Path, vol_id: i16, pages: &[(i32, Vec<u8>)]) {
        use std::os::unix::fs::FileExt as _;

        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap();
        file.set_len(
            u64::from(FIXTURE_TOTAL_SECTORS * FIXTURE_SECTOR_PAGES)
                * crate::format::IO_PAGE_SIZE as u64,
        )
        .unwrap();
        file.write_all_at(&fixture_header_page(vol_id), 0).unwrap();
        let mut reserved: u64 = 1;
        for (page_id, _) in pages {
            reserved |= 1_u64 << (u32::try_from(*page_id).unwrap() / FIXTURE_SECTOR_PAGES);
        }
        let mut bitmap = fixture_envelope_page(vol_id, 1, crate::format::PageType::VolumeBitmap);
        bitmap[32..40].copy_from_slice(&reserved.to_le_bytes());
        file.write_all_at(&bitmap, crate::format::IO_PAGE_SIZE as u64)
            .unwrap();
        for sector in 0..FIXTURE_TOTAL_SECTORS {
            if reserved & (1_u64 << sector) == 0 {
                continue;
            }
            let range = (sector * FIXTURE_SECTOR_PAGES)..((sector + 1) * FIXTURE_SECTOR_PAGES);
            for page_id in range {
                if page_id < 2 {
                    continue;
                }
                let page_id = i32::try_from(page_id).unwrap();
                let bytes = pages
                    .iter()
                    .find(|(candidate, _)| *candidate == page_id)
                    .map_or_else(
                        || {
                            fixture_envelope_page(vol_id, page_id, crate::format::PageType::Unknown)
                                .to_vec()
                        },
                        |(_, bytes)| bytes.clone(),
                    );
                file.write_all_at(
                    &bytes,
                    u64::try_from(page_id).unwrap() * crate::format::IO_PAGE_SIZE as u64,
                )
                .unwrap();
            }
        }
    }

    /// Removes a fixture directory when its test ends, including on panic. A
    /// per-call counter keeps two tests in one thread from colliding, which a
    /// thread-derived name would not.
    struct FixtureDirectory(std::path::PathBuf);

    impl FixtureDirectory {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "volmap-web-interpretation-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture_corpus(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/e1e651de-records/pages")
                .join(name),
        )
        .unwrap()
    }

    /// Builds a snapshot from the pinned record-interpretation corpus: the class
    /// objects sit on volume 0 and the rows on volume 1, exactly as the engine
    /// wrote them, so a class OID has to resolve across volumes.
    fn interpretation_session() -> (FixtureDirectory, WebState, GraphView) {
        use std::io::Write as _;

        let directory = FixtureDirectory::new();
        let volume0 = directory.path().join("interp");
        let volume1 = directory.path().join("interp_x001");
        let vinf = directory.path().join("interp_vinf");
        fixture_write_volume(&volume0, 0, &[(195, fixture_corpus("vol0-page195.bin"))]);
        fixture_write_volume(&volume1, 1, &[(641, fixture_corpus("vol1-page641.bin"))]);
        let mut manifest = std::fs::File::create(&vinf).unwrap();
        writeln!(manifest, "0 {}", volume0.display()).unwrap();
        writeln!(manifest, "1 {}", volume1.display()).unwrap();
        drop(manifest);

        let request = crate::inspection::OpenRequest {
            input: crate::source::InputSpec::Vinf {
                path: vinf,
                volume_root: None,
            },
            tde_keys_file: None,
            spill_directory: None,
        };
        let policy =
            ResourcePolicy::new(8 * 1024 * 1024, 1024 * 1024, 1, 64, 8 * 1024 * 1024).unwrap();
        let view = crate::inspection::Inspection::open(&request, policy, &CancelToken::new(), None)
            .unwrap()
            .view(crate::inspection::RevisionSelector::Latest)
            .unwrap();
        let state = WebState {
            source: LiveSource::new(view.clone(), FollowConfig::default(), false),
            enrichment: Arc::new(Mutex::new(())),
            policy,
            cursor_key: Arc::new([7_u8; 32]),
            authority: Some(Arc::from("127.0.0.1:8787")),
            semaphore: Arc::new(Semaphore::new(1)),
            watchers: Arc::new(Semaphore::new(1)),
        };
        (directory, state, view)
    }

    /// Every live read and every accepted enrichment answers 200: with entity
    /// URLs there is no new revision resource to point a 202 at.
    async fn response_document(response: Response) -> serde_json::Value {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap();
        assert!(
            status == StatusCode::OK,
            "{status}: {}",
            String::from_utf8_lossy(&bytes)
        );
        serde_json::from_slice(&bytes).unwrap()
    }

    /// The value state of one named attribute of a slot resource.
    fn attribute_value(data: &serde_json::Value, name: &str) -> serde_json::Value {
        data["interpretation"]["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|attribute| attribute["name"]["value"] == name)
            .unwrap_or_else(|| panic!("no attribute {name}"))["value"]
            .clone()
    }

    #[test]
    fn clicking_a_record_enriches_its_page_and_returns_interpreted_values() {
        let (_directory, state, _view) = interpretation_session();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // Selecting a slot enriches its page structurally, which is what
            // makes the slot addressable at all.
            let structural = response_document(
                enrich(
                    State(state.clone()),
                    Ok(Json(EnrichmentRequest {
                        selector: "slot:1:641:1".to_owned(),
                    })),
                )
                .await,
            )
            .await;
            let structural_revision: u64 = structural["snapshot"]["revision"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();

            // At that point the panel has the record's extent but no values,
            // so it offers the interpretation rather than inventing one.
            let before =
                response_document(slot(State(state.clone()), Path((1, 641, 1))).await).await;
            assert!(before["data"]["interpretation"].is_null());
            assert!(before["data"]["class_representation"].is_null());
            assert_eq!(before["data"]["selected_slot"]["record_type"], "home");

            let enriched = response_document(
                enrich(
                    State(state.clone()),
                    Ok(Json(EnrichmentRequest {
                        selector: "record:1:641:1".to_owned(),
                    })),
                )
                .await,
            )
            .await;
            let revision: u64 = enriched["snapshot"]["revision"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(
                revision,
                structural_revision + 1,
                "one interpretation click advances the revision exactly once"
            );

            let after =
                response_document(slot(State(state.clone()), Path((1, 641, 1))).await).await;
            let data = &after["data"];
            assert_eq!(
                data["class_representation"]["class_name"]["value"],
                "dba.interp_scalars"
            );
            assert_eq!(data["interpretation"]["bytes"]["state"], "bytes-withheld");
            assert_eq!(attribute_value(data, "id")["value"], "1");
            assert_eq!(attribute_value(data, "c_numeric")["value"], "-12345678.90");
            assert_eq!(attribute_value(data, "c_char")["value"], "fixed8ch");

            // Every record of the page was interpreted by the one click, so the
            // all-NULL row reads back as NULL rather than as missing.
            let sibling =
                response_document(slot(State(state.clone()), Path((1, 641, 2))).await).await;
            assert_eq!(
                attribute_value(&sibling["data"], "c_varchar")["state"],
                "null"
            );

            // No arm of the rendered document may carry value bytes.
            let rendered = serde_json::to_string(&after).unwrap();
            for forbidden in ["\"hex\"", "\"raw\"", "0x"] {
                assert!(!rendered.contains(forbidden), "leaked {forbidden}");
            }
        });
    }

    #[test]
    fn a_page_that_cannot_be_interpreted_states_its_reason_in_the_panel() {
        let (_directory, state, _view) = interpretation_session();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // Page 195 of volume 0 is the class-object page itself: its records
            // live in a NULL-class root heap, which version one does not
            // interpret. Its structure is still inspectable.
            let structural = response_document(
                enrich(
                    State(state.clone()),
                    Ok(Json(EnrichmentRequest {
                        selector: "slot:0:195:2".to_owned(),
                    })),
                )
                .await,
            )
            .await;
            let attempted = response_document(
                enrich(
                    State(state.clone()),
                    Ok(Json(EnrichmentRequest {
                        selector: "record:0:195:2".to_owned(),
                    })),
                )
                .await,
            )
            .await;
            // Nothing is re-reading this session, so both results are published
            // as revisions of the generation they were computed on.
            assert_eq!(structural["data"]["retained"], true);
            assert_eq!(attempted["data"]["retained"], true);

            let document =
                response_document(slot(State(state.clone()), Path((0, 195, 2))).await).await;
            let data = &document["data"];
            // No values, a stated reason, and the structural facts intact.
            assert!(data["interpretation"].is_null());
            let reason = data["interpretation_unavailable"].as_str().unwrap();
            assert!(!reason.is_empty(), "the panel needs a reason to show");
            assert_eq!(data["selected_slot"]["record_type"], "home");
            assert!(!data["selected_slot"]["length"].is_null());
        });
    }

    #[test]
    fn the_record_selector_targets_the_page_holding_one_slot() {
        let Some(EnrichmentTarget::Record(oid)) = parse_enrichment_target("record:1:577:3") else {
            panic!("record selector should parse");
        };
        assert_eq!(
            (oid.vol_id.get(), oid.page_id.get(), oid.slot_id.get()),
            (1, 577, 3)
        );
        // An interpretation is read back through the slot it was asked for, so
        // both selectors resolve to the same resource path.
        assert_eq!(
            target_result_path(EnrichmentTarget::Record(oid)),
            target_result_path(EnrichmentTarget::Slot(oid))
        );
        assert_eq!(
            target_result_path(EnrichmentTarget::Record(oid)),
            "/slot/1/577/3"
        );
    }

    #[test]
    fn malformed_record_selectors_are_refused() {
        for selector in [
            "record:1:577",
            "record:1:577:3:4",
            "record:1:577:-1",
            "record:1:577:03",
            "record::577:3",
            "records:1:577:3",
        ] {
            assert!(
                parse_enrichment_target(selector).is_none(),
                "{selector} should not parse"
            );
        }
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
    fn conflict_errors_preserve_the_structured_reason() {
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
                "This reading cannot be enriched because the snapshot was invalidated. Restart Volmap against a stable input, or serve it with follow enabled."
            );

            let response = cursor_generation_changed_response(4);
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(document["error"]["code"], "cursor-generation-changed");
            assert_eq!(
                document["error"]["message"],
                "This load started at generation 4, which is no longer retained. Restart the load at the current generation."
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
    fn collection_cursor_is_opaque_session_keyed_and_generation_carrying() {
        let (_directory, state) = state();
        let cursor = encode_cursor(&state, 7, "volumes", 100);

        assert_eq!(cursor.len(), CURSOR_HEX_LEN);
        assert_eq!(
            decode_cursor(&state, "volumes", &cursor),
            Some(CursorPayload {
                generation: 7,
                offset: 100,
            })
        );
        // The collection is authenticated too, so a cursor cannot be replayed
        // against a different one.
        assert_eq!(decode_cursor(&state, "coverage", &cursor), None);

        // A cursor issued in an older generation stays recognisably this
        // session's, which is what lets a stale load be told apart from a
        // forged one rather than both answering "invalid".
        let older = encode_cursor(&state, 3, "volumes", 100);
        assert_ne!(older, cursor);
        assert_eq!(
            decode_cursor(&state, "volumes", &older),
            Some(CursorPayload {
                generation: 3,
                offset: 100,
            })
        );

        let mut tampered = cursor.into_bytes();
        tampered[0] = if tampered[0] == b'0' { b'1' } else { b'0' };
        let tampered = String::from_utf8(tampered).unwrap();
        assert_eq!(decode_cursor(&state, "volumes", &tampered), None);
        assert_eq!(DEFAULT_COLLECTION_LIMIT, 100);
        assert_eq!(MAX_COLLECTION_LIMIT, 512);
    }

    /// A load whose generation is still retained continues on it; once that
    /// generation has been read past, the load is answered as stale so the
    /// caller restarts rather than treating its own state as corrupt.
    #[test]
    fn a_cursor_outlives_its_generation_only_while_that_generation_is_retained() {
        let (_directory, state, view) = interpretation_session();
        let cursor = encode_cursor(&state, 0, "volumes", 10);

        let Ok((answer, offset)) = collection_start(&state, "volumes", Some(&cursor)) else {
            panic!("a retained generation should still be continued");
        };
        assert_eq!((answer.reading.generation, offset), (0, 10));

        for _ in 0..FollowConfig::default().retain {
            state
                .source
                .publish(view.clone(), std::time::Duration::ZERO)
                .unwrap();
        }
        let Err(refusal) = collection_start(&state, "volumes", Some(&cursor)) else {
            panic!("an evicted generation should not be continued");
        };
        assert!(matches!(refusal, CollectionRefusal::Evicted(0)));
        assert_eq!(refusal.response().status(), StatusCode::CONFLICT);
    }

    #[test]
    fn collection_projection_enforces_limits_and_continuation() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (_directory, state) = state();
            let answer = answer(&state).unwrap();
            let response = collection_response(
                &state,
                &answer,
                "test",
                0,
                DEFAULT_COLLECTION_LIMIT,
                (0_u16..101).collect::<Vec<_>>(),
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

            // The continuation cursor names this reading's generation, so the
            // second page of the load comes from the same reading as the first.
            let payload = decode_cursor(&state, "test", &cursor).unwrap();
            assert_eq!(payload.generation, answer.reading.generation);
            let response = collection_response(
                &state,
                &answer,
                "test",
                payload.offset,
                DEFAULT_COLLECTION_LIMIT,
                (0_u16..101).collect::<Vec<_>>(),
            );
            let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(document["data"]["items"].as_array().unwrap().len(), 1);
            assert_eq!(document["data"]["next_cursor"]["state"], "end");

            let response = collection_response(
                &state,
                &answer,
                "test",
                0,
                MAX_COLLECTION_LIMIT + 1,
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

    /// A superseded reading keeps every fact it observed and gains exactly one
    /// note saying the input has moved on. Being superseded is not a failure and
    /// must not be reported as one.
    #[test]
    fn superseded_standing_annotates_a_reading_once_without_failing_it() {
        let mut overview = test_overview();
        apply_standing(&mut overview, crate::model::SnapshotValidity::Valid);
        assert_eq!(overview.validity, crate::model::SnapshotValidity::Valid);
        assert!(overview.diagnostics.is_empty());

        apply_standing(&mut overview, crate::model::SnapshotValidity::Superseded);
        apply_standing(&mut overview, crate::model::SnapshotValidity::Superseded);
        assert_eq!(
            overview.validity,
            crate::model::SnapshotValidity::Superseded
        );
        assert_eq!(
            overview.outcome,
            crate::diagnostics::InspectionOutcome::SuccessLimited
        );
        assert_eq!(
            overview
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "snapshot.source_advanced")
                .count(),
            1
        );
    }

    // ---- Live follow, proven over a real socket ---------------------------

    /// A live session listening on a real port, with the fixture it reads.
    struct LiveServer {
        /// Held so the fixture outlives the requests made against it.
        _directory: FixtureDirectory,
        volume: std::path::PathBuf,
        address: SocketAddr,
        source: Arc<LiveSource>,
    }

    impl LiveServer {
        /// Rewrites a page in place, moving the volume's file stamp exactly as a
        /// running engine would, without making the volume unreadable.
        fn write_to_the_volume(&self) {
            rewrite_fixture_volume(&self.volume);
        }
    }

    fn rewrite_fixture_volume(volume: &std::path::Path) {
        use std::os::unix::fs::FileExt as _;

        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(volume)
            .unwrap();
        file.write_all_at(&fixture_header_page(0), 0).unwrap();
        file.sync_all().unwrap();
    }

    /// Boots the adapter on `127.0.0.1:0` over a one-volume fixture, following
    /// the input when `follow` is `Some`. The server runs on its own runtime
    /// thread so the test thread can talk to it with a blocking socket.
    fn boot(follow: Option<FollowConfig>) -> LiveServer {
        boot_after_open(follow, false)
    }

    /// Optionally moves the input after its first reading but before the
    /// follower starts. That is the deterministic scheduling state left by a
    /// mid-scan change: the reading's recorded manifest is already stale.
    fn boot_after_open(follow: Option<FollowConfig>, change_after_open: bool) -> LiveServer {
        use std::io::Write as _;

        let directory = FixtureDirectory::new();
        let volume = directory.path().join("live");
        let vinf = directory.path().join("live_vinf");
        fixture_write_volume(&volume, 0, &[]);
        let mut manifest = std::fs::File::create(&vinf).unwrap();
        writeln!(manifest, "0 {}", volume.display()).unwrap();
        drop(manifest);

        let request = OpenRequest {
            input: crate::source::InputSpec::Vinf {
                path: vinf,
                volume_root: None,
            },
            tde_keys_file: None,
            spill_directory: None,
        };
        let policy =
            ResourcePolicy::new(8 * 1024 * 1024, 1024 * 1024, 1, 64, 8 * 1024 * 1024).unwrap();
        let cancel = CancelToken::new();
        let inspection = if follow.is_some() {
            crate::inspection::Inspection::open_live(&request, policy, &cancel, None)
        } else {
            crate::inspection::Inspection::open(&request, policy, &cancel, None)
        }
        .unwrap();
        let view = inspection
            .view(crate::inspection::RevisionSelector::Latest)
            .unwrap();
        if change_after_open {
            rewrite_fixture_volume(&volume);
        }
        let following = follow.is_some();
        let source = LiveSource::new(view, follow.unwrap_or_default(), following);
        let server_source = Arc::clone(&source);

        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                let state = WebState {
                    source: Arc::clone(&source),
                    enrichment: Arc::new(Mutex::new(())),
                    policy,
                    cursor_key: Arc::new([7_u8; 32]),
                    authority: Some(Arc::from(address.to_string())),
                    semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
                    watchers: Arc::new(Semaphore::new(MAX_CONCURRENT_WATCHERS)),
                };
                if following {
                    tokio::spawn(crate::follow::follow(source, request, policy));
                }
                sender.send(address).unwrap();
                let _ = axum::serve(listener, build_router(state)).await;
            });
        });

        LiveServer {
            _directory: directory,
            volume,
            address: receiver.recv().unwrap(),
            source: server_source,
        }
    }

    /// A blocking HTTP/1.1 exchange over one connection. `Connection: close`
    /// makes the response self-delimiting, which is all a test needs and keeps
    /// the release dependency graph pinned.
    fn exchange_raw(address: SocketAddr, request: &str) -> (u16, String) {
        use std::io::{Read as _, Write as _};

        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(40)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let text = String::from_utf8_lossy(&raw).into_owned();
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        let body = text
            .split_once("\r\n\r\n")
            .map_or("", |(_, body)| body)
            .to_owned();
        (status, body)
    }

    fn exchange(address: SocketAddr, request: &str) -> (u16, serde_json::Value) {
        let (status, body) = exchange_raw(address, request);
        (
            status,
            serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
        )
    }

    fn shell(address: SocketAddr, path: &str) -> (u16, String) {
        exchange_raw(
            address,
            &format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"),
        )
    }

    fn get(address: SocketAddr, path: &str) -> (u16, serde_json::Value) {
        exchange(
            address,
            &format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"),
        )
    }

    fn enrichment(address: SocketAddr, selector: &str) -> (u16, serde_json::Value) {
        let payload = format!("{{\"selector\":\"{selector}\"}}");
        exchange(
            address,
            &format!(
                "POST /api/v1/enrichments HTTP/1.1\r\nHost: {address}\r\nOrigin: http://{address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            ),
        )
    }

    fn generation_of(document: &serde_json::Value) -> u64 {
        document["snapshot"]["generation"]
            .as_str()
            .unwrap_or_else(|| panic!("no generation in {document}"))
            .parse()
            .unwrap()
    }

    /// A short-fused follow config, so the watcher's debounce runs its real
    /// course inside a test rather than being bypassed.
    fn brisk_follow() -> FollowConfig {
        FollowConfig {
            poll_interval: std::time::Duration::from_millis(25),
            quiet_period: std::time::Duration::from_millis(25),
            max_defer: std::time::Duration::from_millis(400),
            min_idle: std::time::Duration::from_millis(25),
            retain: 4,
        }
    }

    /// Acceptance 1 and 2: writing to a followed volume advances the generation,
    /// and the entity URL that answered before the write still answers after it.
    #[test]
    fn a_write_to_a_followed_input_advances_the_generation_and_keeps_urls_live() {
        let server = boot(Some(brisk_follow()));

        let (status, before) = get(server.address, "/api/v1/volumes");
        assert_eq!(status, 200, "{before}");
        assert_eq!(generation_of(&before), 0);
        assert_eq!(before["snapshot"]["validity"], "valid");
        let volumes = before["data"]["items"].as_array().unwrap().len();

        // A followed input nobody writes to must not churn. Without this the
        // advance asserted below would prove nothing: a watcher that published a
        // generation every poll would pass just as happily.
        std::thread::sleep(brisk_follow().max_defer * 2);
        let (status, quiet) = get(server.address, "/api/v1/volumes");
        assert_eq!(status, 200, "{quiet}");
        assert_eq!(generation_of(&quiet), 0, "an unchanged input churned");
        assert_eq!(quiet["snapshot"]["validity"], "valid");

        server.write_to_the_volume();

        // The long poll returns the moment the re-read is published, so the
        // test waits on the event rather than on a sleep.
        let (status, watched) = get(server.address, "/api/v1/live/watch?generation=0");
        assert_eq!(status, 200, "{watched}");
        assert_eq!(watched["data"]["advanced"], true);
        let advanced = generation_of(&watched);
        assert!(
            advanced >= 1,
            "the generation should have advanced: {watched}"
        );

        // The URL was never generation-scoped, so it needs no updating.
        let (status, after) = get(server.address, "/api/v1/volumes");
        assert_eq!(status, 200, "{after}");
        assert!(generation_of(&after) >= advanced);
        assert_eq!(after["data"]["items"].as_array().unwrap().len(), volumes);
        assert_eq!(after["data"]["next_cursor"]["state"], "end");
    }

    /// Acceptance 2 names generations 3 and 40 deliberately: the copied URL
    /// must not depend on the generation that first answered it, even after
    /// that generation is far outside the retention window.
    #[test]
    fn a_generation_three_entity_url_still_resolves_at_generation_forty() {
        let server = boot(Some(brisk_follow()));
        let view = server.source.current().unwrap().view;
        for expected in 1..=3 {
            assert_eq!(
                server
                    .source
                    .publish(view.clone(), std::time::Duration::ZERO)
                    .unwrap(),
                expected
            );
        }

        let path = "/api/v1/page/0/2";
        let (status, copied) = get(server.address, path);
        assert_eq!(status, 200, "{copied}");
        assert_eq!(generation_of(&copied), 3);
        assert_eq!(copied["data"]["page"]["page_id"], 2);

        for expected in 4..=40 {
            assert_eq!(
                server
                    .source
                    .publish(view.clone(), std::time::Duration::ZERO)
                    .unwrap(),
                expected
            );
        }
        assert!(server.source.retained(3).unwrap().is_none());

        let (status, resolved) = get(server.address, path);
        assert_eq!(status, 200, "{resolved}");
        assert_eq!(generation_of(&resolved), 40);
        assert_eq!(resolved["data"]["page"]["page_id"], 2);
    }

    /// The session report tells the browser what it is looking at, which is what
    /// the follow loop bootstraps from.
    #[test]
    fn a_followed_session_reports_its_follow_state_and_observation_time() {
        let server = boot(Some(brisk_follow()));

        let (status, session) = get(server.address, "/api/v1/session");
        assert_eq!(status, 200, "{session}");
        assert_eq!(session["data"]["follow"]["state"], "following");
        assert_eq!(session["data"]["follow"]["poll_interval_ms"], "25");
        assert_eq!(session["data"]["follow"]["retained_generations"], "4");
        assert!(
            session["snapshot"]["observed_at_unix_seconds"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
                > 0
        );
    }

    /// Acceptance 4's scheduling half, without a timing race. A mid-scan
    /// change leaves the published reading's manifest behind the input; the
    /// follower must notice that exact state and publish another scan.
    #[test]
    fn a_live_reading_with_a_stale_manifest_schedules_another_scan() {
        let server = boot_after_open(Some(brisk_follow()), true);

        let (status, watched) = get(server.address, "/api/v1/live/watch?generation=0");
        assert_eq!(status, 200, "{watched}");
        assert_eq!(watched["data"]["advanced"], true);
        assert!(generation_of(&watched) >= 1, "{watched}");
    }

    /// Acceptance 3: with follow off the offline contract is untouched. Nothing
    /// re-reads the input, so a change ends the session at an invalidated
    /// snapshot instead of advancing past it.
    #[test]
    fn without_follow_a_changed_input_still_reaches_the_invalidated_snapshot() {
        let server = boot(None);

        let (status, session) = get(server.address, "/api/v1/session");
        assert_eq!(status, 200, "{session}");
        assert_eq!(session["data"]["follow"]["state"], "disabled");
        assert_eq!(session["snapshot"]["validity"], "valid");

        server.write_to_the_volume();

        // No watcher, so the generation cannot move and the long poll says so
        // at once rather than holding the request open.
        let (status, watched) = get(server.address, "/api/v1/live/watch?generation=0");
        assert_eq!(status, 200, "{watched}");
        assert_eq!(watched["data"]["advanced"], false);
        assert_eq!(generation_of(&watched), 0);

        // The first deep read discovers the change and ends the session on it.
        let (status, enriched) = enrichment(server.address, "page:0:2");
        assert_eq!(status, 200, "{enriched}");
        assert_eq!(enriched["snapshot"]["validity"], "invalidated");
        assert_eq!(enriched["outcome"], "fatal");
        assert!(
            enriched["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["code"] == "snapshot.modified"),
            "{enriched}"
        );

        // And it stays ended: a further enrichment is refused outright.
        let (status, refused) = enrichment(server.address, "page:0:3");
        assert_eq!(status, 409, "{refused}");
        assert_eq!(refused["error"]["code"], "base-revision-unusable");
    }

    /// The asset contract test proves the browser asks for entity paths. This
    /// proves the server answers them, which is the half a text assertion
    /// cannot reach: the two halves drifted apart once already, and the asset
    /// test stayed green throughout because it never spoke to a server.
    #[test]
    fn every_address_the_browser_builds_resolves() {
        let server = boot(Some(brisk_follow()));

        for path in [
            "/api/v1/session",
            "/api/v1/overview",
            "/api/v1/volumes",
            "/api/v1/sectors/0",
            "/api/v1/relationships",
            "/api/v1/diagnostics",
            "/api/v1/coverage",
            "/api/v1/sector/0/0",
            "/api/v1/page/0/2",
            // No generation given, so the long poll reports at once instead of
            // holding the request open for its full window.
            "/api/v1/live/watch",
        ] {
            let (status, document) = get(server.address, path);
            assert_eq!(status, 200, "{path}: {document}");
            assert_eq!(document["schema"], SCHEMA_NAME, "{path}: {document}");
            assert_eq!(document["snapshot"]["generation"], "0", "{path}");
        }

        // The notices document is its own schema and carries no reading, so it
        // is checked for reachability rather than for an envelope.
        let (status, licenses) = get(server.address, "/api/v1/licenses");
        assert_eq!(status, 200);
        assert_eq!(licenses["schema"], "volmap.licenses");

        // Every drill level is a real address the browser can be loaded at
        // directly, and each one hands back the application shell.
        for path in [
            "/",
            "/volume/0",
            "/sector/0/0",
            "/page/0/2",
            "/slot/0/2/0",
            "/oos/0/2/0",
            "/file/0/0",
        ] {
            let (status, body) = shell(server.address, path);
            assert_eq!(status, 200, "{path}: {body}");
            assert!(
                body.contains("Volmap Inspector"),
                "{path} did not serve the application shell"
            );
        }

        // The grammar the browser abandoned is gone from the server too, so a
        // stale bookmark fails loudly rather than resolving to a stale reading.
        for path in [
            "/s/00000000000000000000000000000000/r/0/volume/0",
            "/api/v1/jobs/1",
        ] {
            let (status, _body) = shell(server.address, path);
            assert_eq!(status, 404, "{path} should no longer resolve");
        }
    }

    /// The envelope reports when the input last changed on disk separately from
    /// when Volmap read it, because a reader who has committed a change and
    /// cannot see it is looking at the gap between those two times. Conflating
    /// them would hide exactly the thing that reader needs.
    #[test]
    fn the_envelope_separates_the_disk_time_from_the_read_time() {
        let server = boot(Some(brisk_follow()));

        let (status, before) = get(server.address, "/api/v1/overview");
        assert_eq!(status, 200, "{before}");
        let read_at = before["snapshot"]["observed_at_unix_seconds"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let disk_at = before["snapshot"]["input_modified_unix_seconds"]
            .as_str()
            .unwrap_or_else(|| panic!("no disk time in {before}"))
            .parse::<u64>()
            .unwrap();
        // The fixture was written before it was read, so the disk time cannot
        // be the later of the two.
        assert!(
            disk_at <= read_at,
            "disk {disk_at} later than read {read_at}"
        );
        assert!(disk_at > 0);

        server.write_to_the_volume();
        let watched = get(server.address, "/api/v1/live/watch?generation=0").1;
        assert_eq!(watched["data"]["advanced"], true);

        // A write moves the disk time, and the generation that observed it
        // reports the newer value rather than the one it superseded.
        let (status, after) = get(server.address, "/api/v1/overview");
        assert_eq!(status, 200, "{after}");
        let disk_after = after["snapshot"]["input_modified_unix_seconds"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(
            disk_after >= disk_at,
            "the disk time went backwards: {disk_at} then {disk_after}"
        );
    }

    /// An immutable session reports a disk time too. It reads the input once, so
    /// the value never moves, but a reader still wants to know how old the
    /// bytes in front of them are.
    #[test]
    fn an_immutable_session_still_reports_when_its_input_was_written() {
        let server = boot(None);

        let (status, document) = get(server.address, "/api/v1/session");
        assert_eq!(status, 200, "{document}");
        assert_eq!(document["data"]["follow"]["state"], "disabled");
        assert!(
            document["snapshot"]["input_modified_unix_seconds"]
                .as_str()
                .unwrap_or_else(|| panic!("no disk time in {document}"))
                .parse::<u64>()
                .unwrap()
                > 0
        );
    }
}
