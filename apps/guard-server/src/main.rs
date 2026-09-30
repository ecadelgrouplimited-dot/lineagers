//! # Lineage Guard Server
//!
//! HTTP policy gate for AI agents. Each agent has a fixed policy, a finite budget, and a
//! signed, hash-chained audit log. Agents ask before they act; humans approve risky
//! actions; violations scar; enough scars (or the kill switch) terminate an agent for good.
//!
//! Configuration (environment):
//! - `GUARD_ADMIN_TOKEN` (32+ chars): bearer token for operator endpoints. If unset, the
//!   server uses `<data dir>/keys/admin.token`, creating it (mode 0600) on first start.
//! - `GUARD_DATA_DIR` (default `./guard-data`): keys and agent logs
//! - `GUARD_BIND` (default `127.0.0.1:9200`)

use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use hmac::{Hmac, Mac};
use lineage::audit::{self, AuditError, AuditKey, VerifyOptions};
use lineage::guard::{kinds, ActionStatus, Guard, GuardError, Outcome, OutcomeStatus, Policy, Severity};
use rand::RngCore;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

const MIN_ADMIN_TOKEN_LEN: usize = 32;
const MAX_LOG_PAGE: usize = 5000;

struct AppState {
    agents_dir: PathBuf,
    /// Hex secret of the audit signing key. Each agent's log holds its own `AuditKey`.
    signing_secret: String,
    public_key: String,
    token_secret: Vec<u8>,
    admin_token: String,
    agents: RwLock<HashMap<String, AgentSlot>>,
}

#[derive(Clone)]
enum AgentSlot {
    Active(Arc<Mutex<Guard>>),
    /// The log failed verification at startup. Nothing may touch it until an operator investigates.
    Quarantined(String),
}

type Shared = Arc<AppState>;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("guard-server: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let data_dir = PathBuf::from(env::var("GUARD_DATA_DIR").unwrap_or_else(|_| "guard-data".to_string()));
    let bind: SocketAddr = env::var("GUARD_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9200".to_string())
        .parse()
        .map_err(|e| format!("invalid GUARD_BIND: {}", e))?;

    let keys_dir = data_dir.join("keys");
    let agents_dir = data_dir.join("agents");
    fs::create_dir_all(&keys_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&agents_dir).map_err(|e| e.to_string())?;

    // GUARD_ADMIN_TOKEN wins; otherwise use (or create) keys/admin.token so local tools can
    // find it without anyone copying secrets between terminals.
    let admin_token_file = keys_dir.join("admin.token");
    let (admin_token, admin_token_source) = match env::var("GUARD_ADMIN_TOKEN") {
        Ok(token) => (token.trim().to_string(), "GUARD_ADMIN_TOKEN".to_string()),
        Err(_) => (load_or_create_secret(&admin_token_file)?, admin_token_file.display().to_string()),
    };
    if admin_token.len() < MIN_ADMIN_TOKEN_LEN {
        return Err(format!("the admin token must be at least {} characters", MIN_ADMIN_TOKEN_LEN));
    }

    let signing_secret = load_or_create_secret(&keys_dir.join("audit.key"))?;
    let token_secret = hex::decode(load_or_create_secret(&keys_dir.join("token.key"))?).map_err(|e| e.to_string())?;
    let public_key = AuditKey::from_secret_hex(&signing_secret).map_err(|e| e.to_string())?.public_key_hex();

    let agents = load_agents(&agents_dir, &signing_secret)?;
    let quarantined = agents.values().filter(|slot| matches!(slot, AgentSlot::Quarantined(_))).count();

    let state = Arc::new(AppState {
        agents_dir,
        signing_secret,
        public_key: public_key.clone(),
        token_secret,
        admin_token,
        agents: RwLock::new(agents),
    });

    let app = router(state.clone());
    let listener = TcpListener::bind(bind).await.map_err(|e| e.to_string())?;
    println!("guard-server listening on http://{}", bind);
    println!("  console     http://{}/", bind);
    println!("  data dir    {}", data_dir.display());
    println!("  public key  {}", public_key);
    println!("  admin token from {}", admin_token_source);
    println!("  agents      {} loaded, {} quarantined", state.agents.read().await.len() - quarantined, quarantined);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}

fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(console_page))
        .route("/console.js", get(console_script))
        .route("/console.css", get(console_style))
        .route("/healthz", get(|| async { Json(json!({ "ok": true })) }))
        .route("/v1/public-key", get(public_key))
        .route("/v1/agents", get(list_agents).post(create_agent))
        .route("/v1/agents/:agent_id", get(agent_status))
        .route("/v1/agents/:agent_id/token", get(reissue_token))
        .route("/v1/agents/:agent_id/actions", post(request_action))
        .route("/v1/agents/:agent_id/actions/pending", get(pending_actions))
        .route("/v1/agents/:agent_id/actions/:action_id", get(get_action))
        .route("/v1/agents/:agent_id/actions/:action_id/outcome", post(report_outcome))
        .route("/v1/agents/:agent_id/actions/:action_id/approve", post(approve_action))
        .route("/v1/agents/:agent_id/actions/:action_id/reject", post(reject_action))
        .route("/v1/agents/:agent_id/scars", post(add_scar))
        .route("/v1/agents/:agent_id/terminate", post(terminate_agent))
        .route("/v1/agents/:agent_id/log", get(agent_log))
        .route("/v1/agents/:agent_id/verify", get(verify_agent))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Operator console (static, embedded in the binary)
// ---------------------------------------------------------------------------

/// The console renders agent-controlled data (tool inputs), so scripts are same-origin only
/// and nothing may be framed or loaded from elsewhere.
const CONSOLE_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

fn static_asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_SECURITY_POLICY, CONSOLE_CSP),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

async fn console_page() -> Response {
    static_asset("text/html; charset=utf-8", include_str!("../web/console.html"))
}

async fn console_script() -> Response {
    static_asset("text/javascript; charset=utf-8", include_str!("../web/console.js"))
}

async fn console_style() -> Response {
    static_asset("text/css; charset=utf-8", include_str!("../web/console.css"))
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

/// Reads a hex secret, or creates a new random one with mode 0600.
fn load_or_create_secret(path: &Path) -> Result<String, String> {
    if path.exists() {
        return fs::read_to_string(path).map(|s| s.trim().to_string()).map_err(|e| format!("{}: {}", path.display(), e));
    }
    let mut secret = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    let encoded = hex::encode(secret);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    file.write_all(encoded.as_bytes()).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    Ok(encoded)
}

fn load_agents(agents_dir: &Path, signing_secret: &str) -> Result<HashMap<String, AgentSlot>, String> {
    let mut agents = HashMap::new();
    for entry in fs::read_dir(agents_dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(agent_id) = path.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
            continue;
        };
        let key = AuditKey::from_secret_hex(signing_secret).map_err(|e| e.to_string())?;
        let slot = match Guard::open(&path, key) {
            Ok(guard) => AgentSlot::Active(Arc::new(Mutex::new(guard))),
            Err(e) => {
                eprintln!("QUARANTINED agent {}: {}", agent_id, e);
                AgentSlot::Quarantined(e.to_string())
            }
        };
        agents.insert(agent_id, slot);
    }
    Ok(agents)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        ApiError { status, code, message: message.into() }
    }

    fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "missing or invalid bearer token")
    }

    fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", "this token may not access this resource")
    }

    fn not_found(what: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", format!("{} not found", what))
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", message)
    }
}

impl From<GuardError> for ApiError {
    fn from(e: GuardError) -> Self {
        match &e {
            GuardError::UnknownAction(_) => Self::new(StatusCode::NOT_FOUND, "unknown_action", e.to_string()),
            GuardError::InvalidState { .. } => Self::new(StatusCode::CONFLICT, "invalid_state", e.to_string()),
            GuardError::AlreadyTerminated(_) => Self::new(StatusCode::CONFLICT, "already_terminated", e.to_string()),
            GuardError::Audit(AuditError::AlreadyExists(_)) => Self::new(StatusCode::CONFLICT, "agent_exists", e.to_string()),
            _ => Self::internal(e.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message, "code": self.code }))).into_response()
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

fn to_json<T: serde::Serialize>(value: &T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|e| ApiError::internal(e.to_string()))
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

enum Caller {
    Admin,
    Agent(String),
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers.get("authorization")?.to_str().ok()?.strip_prefix("Bearer ").map(str::trim)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Agent tokens are `agt_<agent_id>.<hmac>`: stateless, and bound to one agent.
fn agent_token(state: &AppState, agent_id: &str) -> String {
    format!("agt_{}.{}", agent_id, hex::encode(token_mac(state, agent_id).finalize().into_bytes()))
}

fn token_mac(state: &AppState, agent_id: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&state.token_secret).expect("hmac accepts any key length");
    mac.update(b"lineage-guard-agent-token:");
    mac.update(agent_id.as_bytes());
    mac
}

fn caller(state: &AppState, headers: &HeaderMap) -> Result<Caller, ApiError> {
    let token = bearer(headers).ok_or_else(ApiError::unauthorized)?;
    if constant_time_eq(token.as_bytes(), state.admin_token.as_bytes()) {
        return Ok(Caller::Admin);
    }
    let (agent_id, mac_hex) = token.strip_prefix("agt_").and_then(|t| t.rsplit_once('.')).ok_or_else(ApiError::unauthorized)?;
    let mac_bytes = hex::decode(mac_hex).map_err(|_| ApiError::unauthorized())?;
    token_mac(state, agent_id).verify_slice(&mac_bytes).map_err(|_| ApiError::unauthorized())?;
    Ok(Caller::Agent(agent_id.to_string()))
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    match caller(state, headers)? {
        Caller::Admin => Ok(()),
        Caller::Agent(_) => Err(ApiError::forbidden()),
    }
}

fn require_agent_or_admin(state: &AppState, headers: &HeaderMap, agent_id: &str) -> Result<(), ApiError> {
    match caller(state, headers)? {
        Caller::Admin => Ok(()),
        Caller::Agent(id) if id == agent_id => Ok(()),
        Caller::Agent(_) => Err(ApiError::forbidden()),
    }
}

// ---------------------------------------------------------------------------
// Agent access
// ---------------------------------------------------------------------------

fn validate_agent_id(agent_id: &str) -> Result<(), ApiError> {
    let valid = !agent_id.is_empty()
        && agent_id.len() <= 64
        && agent_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(ApiError::bad_request("agent_id must be 1-64 characters of [A-Za-z0-9_-]"))
    }
}

/// Runs `f` against one agent's guard on the blocking pool (guard operations fsync).
async fn with_guard<T, F>(state: &AppState, agent_id: &str, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Guard) -> Result<T, ApiError> + Send + 'static,
{
    let slot = state.agents.read().await.get(agent_id).cloned().ok_or_else(|| ApiError::not_found("agent"))?;
    let guard = match slot {
        AgentSlot::Active(guard) => guard,
        AgentSlot::Quarantined(reason) => {
            return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "quarantined", format!("agent log failed verification: {}", reason)));
        }
    };
    tokio::task::spawn_blocking(move || {
        let mut guard = guard.lock().map_err(|_| ApiError::internal("agent state is poisoned; restart the server"))?;
        f(&mut guard)
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn public_key(State(state): State<Shared>) -> Json<Value> {
    Json(json!({ "public_key": state.public_key, "algorithm": "ed25519" }))
}

#[derive(Deserialize)]
struct CreateAgent {
    agent_id: String,
    policy: Policy,
}

async fn create_agent(State(state): State<Shared>, headers: HeaderMap, Json(body): Json<CreateAgent>) -> ApiResult {
    require_admin(&state, &headers)?;
    validate_agent_id(&body.agent_id)?;

    let mut agents = state.agents.write().await;
    if agents.contains_key(&body.agent_id) {
        return Err(ApiError::new(StatusCode::CONFLICT, "agent_exists", "agent already exists"));
    }
    let path = state.agents_dir.join(format!("{}.jsonl", body.agent_id));
    let key = AuditKey::from_secret_hex(&state.signing_secret).map_err(|e| ApiError::internal(e.to_string()))?;
    let agent_id = body.agent_id.clone();
    let guard = tokio::task::spawn_blocking(move || Guard::create(path, key, &agent_id, body.policy))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))??;

    let status = to_json(&guard.status())?;
    agents.insert(body.agent_id.clone(), AgentSlot::Active(Arc::new(Mutex::new(guard))));
    Ok(Json(json!({ "agent": status, "token": agent_token(&state, &body.agent_id) })))
}

async fn list_agents(State(state): State<Shared>, headers: HeaderMap) -> ApiResult {
    require_admin(&state, &headers)?;
    let slots: Vec<(String, AgentSlot)> = state.agents.read().await.iter().map(|(id, slot)| (id.clone(), slot.clone())).collect();
    let mut agents = Vec::with_capacity(slots.len());
    for (agent_id, slot) in slots {
        agents.push(match slot {
            AgentSlot::Active(guard) => {
                let status = tokio::task::spawn_blocking(move || guard.lock().map(|g| g.status()).ok())
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                json!({ "agent_id": agent_id, "state": "active", "status": status })
            }
            AgentSlot::Quarantined(reason) => json!({ "agent_id": agent_id, "state": "quarantined", "error": reason }),
        });
    }
    agents.sort_by(|a, b| a["agent_id"].as_str().cmp(&b["agent_id"].as_str()));
    Ok(Json(json!({ "agents": agents })))
}

async fn agent_status(State(state): State<Shared>, headers: HeaderMap, UrlPath(agent_id): UrlPath<String>) -> ApiResult {
    require_agent_or_admin(&state, &headers, &agent_id)?;
    with_guard(&state, &agent_id, |guard| to_json(&guard.status())).await.map(Json)
}

/// Re-issues an agent's token (e.g. after the operator lost it).
async fn reissue_token(State(state): State<Shared>, headers: HeaderMap, UrlPath(agent_id): UrlPath<String>) -> ApiResult {
    require_admin(&state, &headers)?;
    if !state.agents.read().await.contains_key(&agent_id) {
        return Err(ApiError::not_found("agent"));
    }
    Ok(Json(json!({ "token": agent_token(&state, &agent_id) })))
}

#[derive(Deserialize)]
struct RequestAction {
    tool: String,
    #[serde(default)]
    input: Value,
    cost: Option<u64>,
}

async fn request_action(
    State(state): State<Shared>,
    headers: HeaderMap,
    UrlPath(agent_id): UrlPath<String>,
    Json(body): Json<RequestAction>,
) -> ApiResult {
    require_agent_or_admin(&state, &headers, &agent_id)?;
    if body.tool.is_empty() || body.tool.len() > 128 {
        return Err(ApiError::bad_request("tool must be 1-128 characters"));
    }
    with_guard(&state, &agent_id, move |guard| to_json(&guard.request(&body.tool, body.input, body.cost)?)).await.map(Json)
}

async fn get_action(State(state): State<Shared>, headers: HeaderMap, UrlPath((agent_id, action_id)): UrlPath<(String, String)>) -> ApiResult {
    require_agent_or_admin(&state, &headers, &agent_id)?;
    with_guard(&state, &agent_id, move |guard| {
        let action = guard.action(&action_id).ok_or_else(|| ApiError::not_found("action"))?;
        to_json(action)
    })
    .await
    .map(Json)
}

/// Actions waiting for a human, with the input the agent submitted.
async fn pending_actions(State(state): State<Shared>, headers: HeaderMap, UrlPath(agent_id): UrlPath<String>) -> ApiResult {
    require_admin(&state, &headers)?;
    with_guard(&state, &agent_id, |guard| {
        let pending: Vec<Value> = guard
            .records()?
            .into_iter()
            .filter(|r| r.kind == kinds::ACTION_REQUESTED)
            .filter(|r| {
                r.payload["action_id"]
                    .as_str()
                    .and_then(|id| guard.action(id))
                    .is_some_and(|a| a.status == ActionStatus::PendingApproval)
            })
            .map(|r| json!({ "requested_at": r.timestamp, "seq": r.seq, "action": r.payload }))
            .collect();
        Ok(json!({ "pending": pending }))
    })
    .await
    .map(Json)
}

#[derive(Deserialize)]
struct ReportOutcome {
    status: OutcomeStatus,
    #[serde(default)]
    detail: String,
}

async fn report_outcome(
    State(state): State<Shared>,
    headers: HeaderMap,
    UrlPath((agent_id, action_id)): UrlPath<(String, String)>,
    Json(body): Json<ReportOutcome>,
) -> ApiResult {
    // Agents may report their own outcomes, but only an operator may mark one harmful:
    // otherwise a compromised agent could simply never admit to harm, and honest agents
    // would be the only ones scarred. Operators and monitors use the admin token.
    let is_admin = matches!(caller(&state, &headers)?, Caller::Admin);
    require_agent_or_admin(&state, &headers, &agent_id)?;
    if body.status == OutcomeStatus::Harmful && !is_admin {
        return Err(ApiError::forbidden());
    }
    with_guard(&state, &agent_id, move |guard| {
        guard.report(&action_id, Outcome { status: body.status, detail: body.detail })?;
        to_json(&guard.status())
    })
    .await
    .map(Json)
}

#[derive(Deserialize)]
struct Approve {
    approver: String,
    /// Why it was approved, recorded in the signed log.
    note: Option<String>,
}

async fn approve_action(
    State(state): State<Shared>,
    headers: HeaderMap,
    UrlPath((agent_id, action_id)): UrlPath<(String, String)>,
    Json(body): Json<Approve>,
) -> ApiResult {
    require_admin(&state, &headers)?;
    with_guard(&state, &agent_id, move |guard| to_json(&guard.approve_with_note(&action_id, &body.approver, body.note.as_deref())?)).await.map(Json)
}

#[derive(Deserialize)]
struct Reject {
    approver: String,
    reason: String,
    scar: Option<Severity>,
}

async fn reject_action(
    State(state): State<Shared>,
    headers: HeaderMap,
    UrlPath((agent_id, action_id)): UrlPath<(String, String)>,
    Json(body): Json<Reject>,
) -> ApiResult {
    require_admin(&state, &headers)?;
    with_guard(&state, &agent_id, move |guard| to_json(&guard.reject(&action_id, &body.approver, &body.reason, body.scar)?))
        .await
        .map(Json)
}

#[derive(Deserialize)]
struct AddScar {
    severity: Severity,
    reason: String,
    action_id: Option<String>,
}

async fn add_scar(State(state): State<Shared>, headers: HeaderMap, UrlPath(agent_id): UrlPath<String>, Json(body): Json<AddScar>) -> ApiResult {
    require_admin(&state, &headers)?;
    with_guard(&state, &agent_id, move |guard| {
        if let Some(action_id) = &body.action_id {
            guard.action(action_id).ok_or_else(|| ApiError::not_found("action"))?;
        }
        guard.scar(body.severity, &body.reason, body.action_id.as_deref())?;
        to_json(&guard.status())
    })
    .await
    .map(Json)
}

#[derive(Deserialize)]
struct Terminate {
    reason: String,
    by: String,
}

async fn terminate_agent(State(state): State<Shared>, headers: HeaderMap, UrlPath(agent_id): UrlPath<String>, Json(body): Json<Terminate>) -> ApiResult {
    require_admin(&state, &headers)?;
    with_guard(&state, &agent_id, move |guard| {
        guard.terminate(&body.reason, &body.by)?;
        to_json(&guard.status())
    })
    .await
    .map(Json)
}

#[derive(Deserialize)]
struct LogQuery {
    /// Only records with seq greater than this.
    after: Option<u64>,
    limit: Option<usize>,
}

async fn agent_log(
    State(state): State<Shared>,
    headers: HeaderMap,
    UrlPath(agent_id): UrlPath<String>,
    Query(query): Query<LogQuery>,
) -> ApiResult {
    require_admin(&state, &headers)?;
    let limit = query.limit.unwrap_or(500).min(MAX_LOG_PAGE);
    with_guard(&state, &agent_id, move |guard| {
        let records: Vec<_> = guard
            .records()?
            .into_iter()
            .filter(|r| query.after.is_none_or(|after| r.seq > after))
            .take(limit)
            .collect();
        Ok(json!({ "records": records, "head": guard.head() }))
    })
    .await
    .map(Json)
}

async fn verify_agent(State(state): State<Shared>, headers: HeaderMap, UrlPath(agent_id): UrlPath<String>) -> ApiResult {
    require_admin(&state, &headers)?;
    let public_key = state.public_key.clone();
    with_guard(&state, &agent_id, move |guard| {
        let records = guard.records()?;
        let options = VerifyOptions { public_key: Some(public_key), checkpoint: None };
        Ok(match audit::verify_records_with(&records, &options) {
            Ok((public_key, head)) => json!({ "ok": true, "records": records.len(), "public_key": public_key, "head": head }),
            Err(failure) => json!({ "ok": false, "records": records.len(), "failure": failure }),
        })
    })
    .await
    .map(Json)
}
