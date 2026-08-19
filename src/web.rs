//! Authenticated, same-origin HTTP adapter with embedded Atlas assets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Read, Write};
use std::net::SocketAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::Json;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Request, State};
use axum::http::header::{
    AUTHORIZATION, CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN,
};
use axum::http::{HeaderName, HeaderValue, StatusCode, Uri};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Semaphore};

use crate::inspection::{CancelToken, DiagnosticRecord, GraphView, QueryError, ResourcePolicy};
use crate::model::{FileId, Oid, PageId, SectorId, SlotId, Vfid, VolId, Vpid};
use crate::projection::{
    CoverageProjection, DeepPageProjection, DiagnosticProjection, OosChainProjection,
    PageProjection, SCHEMA_NAME, SCHEMA_VERSION, SlotProjection, SnapshotProjection,
    VolumeProjection, coverage_projection, deep_page_projection, diagnostic_projection,
    file_header_projection, oos_chain_projection, outcome_name, page_projection, sector_projection,
    slot_projection, snapshot_id_hex, summary_projection, volume_projection,
};

const MAX_URI_BYTES: usize = 8192;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_HEADER_FIELDS: usize = 64;
const MAX_JSON_BYTES: usize = 64 * 1024;
const MAX_CONCURRENT_REQUESTS: usize = 32;

#[derive(Clone, Debug)]
pub struct ServeOptions {
    pub listen: SocketAddr,
    pub allow_remote_http: bool,
    pub external_origin: Option<String>,
    pub token_file: Option<PathBuf>,
    pub policy: ResourcePolicy,
}

#[derive(Clone)]
struct WebState {
    session: Arc<RwLock<LiveSession>>,
    enrichment: Arc<Mutex<()>>,
    policy: ResourcePolicy,
    token_header: Arc<[u8]>,
    authority: Arc<str>,
    origin: Arc<str>,
    semaphore: Arc<Semaphore>,
}

struct LiveSession {
    views: BTreeMap<u64, GraphView>,
    jobs: BTreeSet<u64>,
    latest: u64,
}

#[derive(Debug)]
pub enum ServeError {
    InvalidListener,
    RemoteAcknowledgementRequired,
    ExternalOriginRequired,
    InvalidExternalOrigin,
    TokenDisclosureUnavailable,
    Io(io::Error),
    Runtime(String),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidListener => formatter.write_str("listener must use a numeric IP address"),
            Self::RemoteAcknowledgementRequired => {
                formatter.write_str("non-loopback HTTP requires --allow-remote-http")
            }
            Self::ExternalOriginRequired => {
                formatter.write_str("wildcard HTTP requires --external-origin")
            }
            Self::InvalidExternalOrigin => formatter.write_str("invalid external origin"),
            Self::TokenDisclosureUnavailable => formatter.write_str(
                "token disclosure requires a controlling terminal or a new --token-file",
            ),
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
    let origin = options
        .external_origin
        .as_deref()
        .map(parse_origin)
        .transpose()?
        .unwrap_or_else(|| ParsedOrigin {
            origin: format!("http://{local}"),
            authority: local.to_string(),
        });
    let token = generate_token()?;
    disclose_token(&token, options.token_file.as_ref())?;
    let token_header = Arc::<[u8]>::from(format!("Bearer {token}").into_bytes());
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
        token_header,
        authority: Arc::from(origin.authority.as_str()),
        origin: Arc::from(origin.origin.as_str()),
        semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
    };
    let router = Router::new()
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
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(MAX_JSON_BYTES))
        .layer(from_fn_with_state(state, request_guard));
    eprintln!("Volmap web origin: {}", origin.origin);
    if !options.listen.ip().is_loopback() {
        eprintln!(
            "WARNING: plain HTTP on a non-loopback listener; bearer authentication does not provide transport confidentiality or integrity. Use only on a trusted internal network, SSH, VPN, or a trusted TLS proxy."
        );
    }
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| ServeError::Runtime(error.to_string()))
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
    if host != Some(state.authority.as_ref()) || hosts.next().is_some() {
        return Err(GuardError {
            status: StatusCode::MISDIRECTED_REQUEST,
            code: "invalid-host",
        });
    }
    if request.uri().path().starts_with("/api/v1/") {
        let authorization = request.headers().get_all(AUTHORIZATION);
        let mut values = authorization.iter();
        let supplied = values.next().map(HeaderValue::as_bytes);
        let exactly_one = values.next().is_none();
        let valid = supplied.is_some_and(|value| {
            value.len() == state.token_header.len()
                && bool::from(value.ct_eq(state.token_header.as_ref()))
        });
        if !exactly_one || !valid {
            return Err(GuardError {
                status: StatusCode::UNAUTHORIZED,
                code: "authentication-required",
            });
        }
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
        if supplied != Some(state.origin.as_ref()) || values.next().is_some() {
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
        APP_CSS,
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
            origin: state.origin.to_string(),
        },
    ))
    .into_response()
}

#[derive(Serialize)]
struct SessionProjection {
    origin: String,
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
) -> Response {
    let view = match revision_view(&state, &snapshot, revision) {
        Ok(value) => value,
        Err(error) => return error_response(error.status, error.code),
    };
    let overview = projected_overview(&state, &view);
    let data: Vec<VolumeProjection> = view.volumes().into_iter().map(volume_projection).collect();
    Json(api_envelope(&overview, data)).into_response()
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
            Json(api_envelope(
                &overview,
                PageResourceProjection {
                    page: page_projection(value),
                    deep: deep_page_projection(view.deep_page(vpid)),
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
    if current_revision != revision
        || base.overview().validity == crate::model::SnapshotValidity::Invalidated
    {
        return error_response(StatusCode::CONFLICT, "base-revision-unusable");
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
            return error_response(StatusCode::CONFLICT, "base-revision-unusable");
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
}

fn error_response(status: StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            schema: SCHEMA_NAME,
            schema_version: SCHEMA_VERSION,
            document_type: "error",
            error: ErrorDetail { code },
        }),
    )
        .into_response()
}

fn validate_listener(options: &ServeOptions) -> Result<(), ServeError> {
    let ip = options.listen.ip();
    if !ip.is_loopback() && !options.allow_remote_http {
        return Err(ServeError::RemoteAcknowledgementRequired);
    }
    if ip.is_unspecified() && options.external_origin.is_none() {
        return Err(ServeError::ExternalOriginRequired);
    }
    if let Some(origin) = &options.external_origin {
        parse_origin(origin)?;
    }
    Ok(())
}

struct ParsedOrigin {
    origin: String,
    authority: String,
}

fn parse_origin(value: &str) -> Result<ParsedOrigin, ServeError> {
    let uri: Uri = value
        .parse()
        .map_err(|_| ServeError::InvalidExternalOrigin)?;
    let scheme = uri.scheme_str().ok_or(ServeError::InvalidExternalOrigin)?;
    if !matches!(scheme, "http" | "https")
        || uri.path() != "/"
        || uri.query().is_some()
        || uri.authority().is_none()
    {
        return Err(ServeError::InvalidExternalOrigin);
    }
    let authority = uri.authority().ok_or(ServeError::InvalidExternalOrigin)?;
    if authority.as_str().contains('@') || authority.port_u16().is_none() {
        return Err(ServeError::InvalidExternalOrigin);
    }
    Ok(ParsedOrigin {
        origin: format!("{scheme}://{authority}"),
        authority: authority.to_string(),
    })
}

fn generate_token() -> Result<String, ServeError> {
    let mut random = OpenOptions::new().read(true).open("/dev/urandom")?;
    let mut bytes = [0_u8; 32];
    random.read_exact(&mut bytes)?;
    let mut token = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut token, "{byte:02x}")
            .map_err(|_| ServeError::Runtime("could not encode session credential".to_owned()))?;
    }
    Ok(token)
}

fn disclose_token(token: &str, token_file: Option<&PathBuf>) -> Result<(), ServeError> {
    if let Some(path) = token_file {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(token.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        return Ok(());
    }
    if !io::stderr().is_terminal() {
        return Err(ServeError::TokenDisclosureUnavailable);
    }
    eprintln!("Volmap bearer token (shown once): {token}");
    Ok(())
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer"><title>Volmap Inspector</title><link rel="stylesheet" href="/app.css"></head>
<body><header><strong>VOLMAP</strong><span id="crumb">locked session</span><span class="spacer"></span><button id="licenses" hidden>About &amp; licenses</button><span id="outcome">locked</span></header>
<section id="unlock"><h1>Unlock inspection</h1><p>Enter the one-time bearer token printed by the server. It remains only in this page's memory and is lost on refresh.</p><form id="unlockForm"><label>Bearer token <input id="token" type="password" autocomplete="off" spellcheck="false"></label><button>Unlock</button><p id="unlockError" role="alert"></p></form></section>
<main id="app" hidden><aside><h2>Snapshot hierarchy</h2><div id="volumes"></div><h2>Sector window</h2><div id="sectors"></div></aside><section class="workspace"><div class="workspace-title"><div><h1 id="sectorTitle">Sector</h1><p id="sectorNote"></p></div><div id="legend">S system · r reserved · . unreserved · ! finding</div></div><div id="grid" role="grid"></div></section><aside id="details"><h2>Selected page</h2><div id="pageDetail">Select a page.</div></aside></main>
<script src="/app.js"></script></body></html>"#;

#[allow(clippy::needless_raw_string_hashes)]
const APP_CSS: &str = r#":root{color-scheme:dark;--bg:#071014;--panel:#0d1820;--line:#29404b;--text:#dce8ec;--muted:#8fa5ae;--cyan:#68d8d0;--blue:#244c66;--green:#205444;--purple:#3b3761;--red:#6b2939}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.4 system-ui,sans-serif}button,input{font:inherit}header{height:54px;display:flex;align-items:center;gap:18px;padding:0 18px;border-bottom:1px solid var(--line);background:#0a1319}header strong{letter-spacing:.08em}.spacer{flex:1}#unlock{max-width:620px;margin:12vh auto;padding:28px;border:1px solid var(--line);background:var(--panel)}#unlock label{display:grid;gap:7px}#unlock input{padding:10px;background:#071014;color:var(--text);border:1px solid var(--line)}button{padding:8px 11px;margin-top:10px;background:var(--cyan);color:#071014;border:0;font-weight:700;cursor:pointer}button:focus-visible,input:focus-visible,[role=gridcell]:focus-visible{outline:2px solid #ffd376;outline-offset:2px}main{min-height:calc(100vh - 54px);display:grid;grid-template-columns:250px minmax(560px,1fr) 340px}aside,.workspace{border-right:1px solid var(--line);min-width:0}aside:last-child{border:0}h1,h2,p{margin:0}h2{font-size:12px;text-transform:uppercase;letter-spacing:.12em;color:var(--muted);padding:14px;border-bottom:1px solid var(--line)}#volumes,#sectors,#pageDetail{padding:10px}.nav{display:block;width:100%;text-align:left;background:transparent;color:var(--text);border:0;padding:7px;margin:0}.nav.active{background:#16303b;color:var(--cyan)}.workspace-title{display:flex;gap:18px;align-items:end;padding:18px}.workspace-title p,#legend{color:var(--muted);font-size:12px}#legend{margin-left:auto}#grid{display:grid;grid-template-columns:repeat(8,minmax(52px,1fr));gap:5px;padding:0 18px 18px}.page{height:62px;margin:0;padding:6px;text-align:left;color:var(--text);border:1px solid transparent;background:#263740}.page.system-metadata{background:var(--purple)}.page.reserved-unallocated{background:var(--blue)}.page.allocated{background:var(--green)}.page.finding{background:var(--red);border-color:#ff7690}.page.selected{border-color:var(--cyan);box-shadow:inset 0 0 0 1px var(--cyan)}.page small{display:block;color:#b3c3c9;margin-top:5px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}dl{display:grid;grid-template-columns:115px 1fr;gap:7px}dt{color:var(--muted)}dd{margin:0;overflow-wrap:anywhere}.withheld{padding:8px;border:1px solid var(--line);color:var(--muted);font-family:ui-monospace,monospace}@media(max-width:1050px){main{grid-template-columns:210px 1fr}#details{grid-column:1/-1;border-top:1px solid var(--line)}}@media(max-width:720px){main{display:block}aside,.workspace{border:0;border-bottom:1px solid var(--line)}#grid{overflow-x:auto;grid-template-columns:repeat(8,62px)}}"#;

const APP_JS: &str = r"(()=>{'use strict';
let token='',session=null,currentVolume=null,currentSector=0,selectedPage=null;
const $=id=>document.getElementById(id);
function button(label,action,className=''){const node=document.createElement('button');node.textContent=label;node.className=className;node.onclick=action;return node}
function fieldList(fields){const list=document.createElement('dl');for(const [name,value] of fields){const term=document.createElement('dt'),detail=document.createElement('dd');term.textContent=name;detail.textContent=String(value);list.append(term,detail)}return list}
async function api(path,options={}){const headers={Authorization:`Bearer ${token}`,...options.headers};const response=await fetch(path,{...options,headers,cache:'no-store',credentials:'omit'});if(!response.ok)throw new Error(`request failed (${response.status})`);return response.json()}
function base(){return `/api/v1/s/${session.snapshot.id}/r/${session.snapshot.revision}`}
function updateSession(payload){session.snapshot=payload.snapshot;session.outcome=payload.outcome;$('outcome').textContent=payload.outcome;$('crumb').textContent=`snapshot ${payload.snapshot.id.slice(0,12)} · revision ${payload.snapshot.revision}`}
async function unlock(event){event.preventDefault();token=$('token').value;$('token').value='';try{session=await api('/api/v1/session');$('unlock').hidden=true;$('app').hidden=false;$('licenses').hidden=false;updateSession(session);await loadVolumes()}catch(error){token='';$('unlockError').textContent=error.message}}
async function loadVolumes(){const payload=await api(`${base()}/volumes`),root=$('volumes');root.replaceChildren();payload.data.forEach((volume,index)=>{const node=button(`volume ${volume.vol_id} · ${volume.total_sectors} sectors`,()=>selectVolume(volume),'nav');node.dataset.volume=String(volume.vol_id);root.append(node);if(index===0)selectVolume(volume)})}
async function selectVolume(volume){currentVolume=volume;currentSector=0;document.querySelectorAll('#volumes .nav').forEach(node=>node.classList.toggle('active',node.dataset.volume===String(volume.vol_id)));renderSectorWindow();await loadSector()}
function renderSectorWindow(){const root=$('sectors');root.replaceChildren();const start=Math.max(0,currentSector-4),end=Math.min(currentVolume.total_sectors,start+10);for(let sector=start;sector<end;sector++){root.append(button(`sector ${sector}`,async()=>{currentSector=sector;renderSectorWindow();await loadSector()},`nav ${sector===currentSector?'active':''}`))}}
async function loadSector(){const payload=await api(`${base()}/sector/${currentVolume.vol_id}/${currentSector}`);$('sectorTitle').textContent=`Sector ${currentSector}`;$('sectorNote').textContent=payload.data.reserved?'reserved by volume bitmap':'unreserved';const grid=$('grid');grid.replaceChildren();payload.data.pages.forEach((page,index)=>{const node=button(String(page.page_id),()=>loadPage(page.page_id),`page ${page.allocation}${page.diagnostic.state==='known'?' finding':''}${page.page_id===selectedPage?' selected':''}`);node.setAttribute('role','gridcell');node.dataset.index=String(index);const small=document.createElement('small');small.textContent=page.page_type.state==='known'?page.page_type.value:'not inspected';node.append(small);node.onkeydown=event=>moveGrid(event,index);grid.append(node)})}
function moveGrid(event,index){let next=index;if(event.key==='ArrowLeft')next--;else if(event.key==='ArrowRight')next++;else if(event.key==='ArrowUp')next-=8;else if(event.key==='ArrowDown')next+=8;else return;if(next>=0&&next<64){event.preventDefault();$('grid').children[next].focus()}}
function withheld(identity){const note=document.createElement('p');note.className='withheld';note.textContent=`evidence ${identity} · structural ranges only · bytes withheld`;return note}
function renderPage(payload){const p=payload.data.page,deep=payload.data.deep,root=$('pageDetail');root.replaceChildren(fieldList([['Identity',`page:${p.vol_id}:${p.page_id}`],['Physical type',p.page_type.state==='known'?p.page_type.value:'not inspected'],['Allocation',p.allocation],['Availability',p.availability],['Detail support',p.detail_support.state==='known'?p.detail_support.value:p.detail_support.state],['Deep revision',deep.state],['TDE',p.tde_state]]));if(deep.state==='not-enriched')root.append(button('Enrich structural metadata',()=>enrich(`page:${p.vol_id}:${p.page_id}`,()=>loadPage(p.page_id))));if(deep.state==='slotted'){const title=document.createElement('h3');title.textContent=`Slots (${deep.structure.slots.length})`;root.append(title);for(const slot of deep.structure.slots){root.append(button(`slot ${slot.slot_id} · ${slot.record_type} · ${slot.length} bytes`,()=>loadSlot(p,slot.slot_id),'nav'))}}root.append(withheld(`page:${p.vol_id}:${p.page_id}`))}
async function loadPage(pageId){selectedPage=pageId;const payload=await api(`${base()}/page/${currentVolume.vol_id}/${pageId}`);renderPage(payload);await loadSector()}
async function loadSlot(page,slotId){const payload=await api(`${base()}/slot/${page.vol_id}/${page.page_id}/${slotId}`),slot=payload.data.selected_slot,root=$('pageDetail');root.replaceChildren(fieldList([['Identity',`slot:${page.vol_id}:${page.page_id}:${slot.slot_id}`],['Record type',`${slot.record_type} (${slot.record_type_ordinal})`],['Offset',slot.offset],['Length',slot.length]]));if(page.page_type.state==='known'&&page.page_type.value==='oos')root.append(button('Validate OOS chain',()=>enrich(`oos:${page.vol_id}:${page.page_id}:${slot.slot_id}`,()=>loadOos(page,slot.slot_id))));root.append(withheld(`slot:${page.vol_id}:${page.page_id}:${slot.slot_id}`))}
async function loadOos(page,slotId){const payload=await api(`${base()}/oos/${page.vol_id}/${page.page_id}/${slotId}`),chain=payload.data.chain,root=$('pageDetail');root.replaceChildren(fieldList([['Identity',`oos:${page.vol_id}:${page.page_id}:${slotId}`],['Complete',chain.complete],['Validated bytes',chain.validated_payload_bytes],['Chunks',chain.chunks.length],['Diagnostic',chain.diagnostic.state==='known'?chain.diagnostic.value:'none']]));for(const chunk of chain.chunks){root.append(fieldList([['Chunk',chunk.chunk_index],['OID',`${chunk.oid.vol_id}:${chunk.oid.page_id}:${chunk.oid.slot_id}`],['Payload length',chunk.payload_length]]))}root.append(withheld(`oos:${page.vol_id}:${page.page_id}:${slotId}`))}
async function enrich(selector,done){try{const payload=await api(`${base()}/enrichments`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({selector})});updateSession(payload);await done()}catch(error){$('pageDetail').append(document.createTextNode(` ${error.message}`))}}
async function showLicenses(){const payload=await api('/api/v1/licenses'),root=$('pageDetail'),text=document.createElement('pre');text.textContent=payload.notice;text.className='withheld';root.replaceChildren(text)}
$('licenses').addEventListener('click',showLicenses);$('unlockForm').addEventListener('submit',unlock)})();";

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
            token_header: Arc::from(&b"Bearer test-token"[..]),
            authority: Arc::from("127.0.0.1:8787"),
            origin: Arc::from("http://127.0.0.1:8787"),
            semaphore: Arc::new(Semaphore::new(1)),
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

    fn authenticated(method: Method, uri: &str) -> Request<Body> {
        let mut request = request(method, uri);
        request
            .headers_mut()
            .insert(AUTHORIZATION, HeaderValue::from_static("Bearer test-token"));
        request
    }

    #[test]
    fn api_authentication_is_exact_and_constant_shape() {
        let state = state();
        let valid = authenticated(Method::GET, "/api/v1/session");
        assert!(guard(&state, &valid).is_ok());

        let missing = request(Method::GET, "/api/v1/session");
        let error = guard(&state, &missing).unwrap_err();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert_eq!(error.code, "authentication-required");

        let mut duplicate = authenticated(Method::GET, "/api/v1/session");
        duplicate
            .headers_mut()
            .append(AUTHORIZATION, HeaderValue::from_static("Bearer test-token"));
        assert_eq!(
            guard(&state, &duplicate).unwrap_err().status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn host_check_precedes_auth_and_ignores_forwarded_authority() {
        let state = state();
        let mut request = authenticated(Method::GET, "/api/v1/session");
        request
            .headers_mut()
            .insert(HOST, HeaderValue::from_static("attacker.test:8787"));
        request.headers_mut().insert(
            HeaderName::from_static("x-forwarded-host"),
            HeaderValue::from_static("127.0.0.1:8787"),
        );
        let error = guard(&state, &request).unwrap_err();
        assert_eq!(error.status, StatusCode::MISDIRECTED_REQUEST);
        assert_eq!(error.code, "invalid-host");

        let mut duplicate = authenticated(Method::GET, "/api/v1/session");
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
        let mut valid = authenticated(Method::POST, "/api/v1/s/id/r/0/enrichments");
        valid
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        valid
            .headers_mut()
            .insert(ORIGIN, HeaderValue::from_static("http://127.0.0.1:8787"));
        assert!(guard(&state, &valid).is_ok());

        let mut missing_origin = authenticated(Method::POST, "/api/v1/s/id/r/0/enrichments");
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

        let mut content_type = authenticated(Method::POST, "/api/v1/s/id/r/0/enrichments");
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
            allow_remote_http: false,
            external_origin: None,
            token_file: None,
            policy: ResourcePolicy::new(1024, 1024, 1, 1, 1024).unwrap(),
        }
    }

    #[test]
    fn remote_http_requires_explicit_acknowledgement_and_wildcard_origin() {
        assert!(validate_listener(&options("127.0.0.1:8787")).is_ok());

        let remote = options("192.0.2.10:8787");
        assert!(matches!(
            validate_listener(&remote),
            Err(ServeError::RemoteAcknowledgementRequired)
        ));

        let mut remote = remote;
        remote.allow_remote_http = true;
        assert!(validate_listener(&remote).is_ok());

        let mut wildcard = options("0.0.0.0:8787");
        wildcard.allow_remote_http = true;
        assert!(matches!(
            validate_listener(&wildcard),
            Err(ServeError::ExternalOriginRequired)
        ));
        wildcard.external_origin = Some("http://debug.internal:8787".to_owned());
        assert!(validate_listener(&wildcard).is_ok());
    }

    #[test]
    fn external_origin_is_an_exact_origin_with_an_explicit_port() {
        let origin = parse_origin("http://debug.internal:8787").unwrap();
        assert_eq!(origin.origin, "http://debug.internal:8787");
        assert_eq!(origin.authority, "debug.internal:8787");
        for invalid in [
            "http://debug.internal",
            "http://user@debug.internal:8787",
            "http://debug.internal:8787/path",
            "http://debug.internal:8787/?query",
            "ftp://debug.internal:8787",
        ] {
            assert!(matches!(
                parse_origin(invalid),
                Err(ServeError::InvalidExternalOrigin)
            ));
        }
    }

    #[test]
    fn browser_credential_has_no_url_or_storage_channel() {
        assert!(!APP_JS.contains("localStorage"));
        assert!(!APP_JS.contains("sessionStorage"));
        assert!(!APP_JS.contains("location.hash"));
        assert!(!APP_JS.contains("?token="));
        assert!(!APP_JS.contains("&token="));
        assert!(APP_JS.contains("Authorization:`Bearer ${token}`"));
        assert!(INDEX_HTML.contains("remains only in this page's memory"));
    }

    #[test]
    fn terminal_invalidation_overlays_old_revision_facts_once() {
        let mut overview = crate::inspection::OverviewView {
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
        };
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
