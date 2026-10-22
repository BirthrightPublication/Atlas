//! Optional loopback-only HTTP boundary for the Atlas companion.
//!
//! Production traffic must arrive through a separately configured trusted HTTPS
//! reverse proxy. Pairing credentials authorize a small service allowlist; they
//! never authorize arbitrary SQLite, filesystem, email, or model access.

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub static_dir: Option<PathBuf>,
    pub port: u16,
    pub dev: bool,
    pub origin: Option<String>,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.dev && self.origin.is_some() {
            return Err("--dev and --origin cannot be combined".into());
        }
        if !self.dev {
            let origin = self.origin.as_ref().ok_or("Companion requires --origin https://your-private-host and a trusted HTTPS reverse proxy")?;
            let parsed = url::Url::parse(origin).map_err(|_| "Invalid companion origin")?;
            if parsed.scheme() != "https"
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.path() != "/"
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err("Companion origin must be an HTTPS origin without credentials, path, query, or fragment".into());
            }
            if self.static_dir.is_none() {
                return Err("Companion requires --static-dir pointing to the built UI".into());
            }
        }
        Ok(())
    }

    fn expected_origin(&self) -> Option<String> {
        self.origin
            .as_ref()
            .map(|s| s.trim_end_matches('/').to_string())
    }
}

#[derive(Clone)]
pub struct HostState {
    pub atlas: Arc<Mutex<atlas_core::Atlas>>,
    config: Config,
    devices: Arc<Mutex<DeviceStore>>,
    mutation_lock: Arc<tokio::sync::Mutex<()>>,
    admin_hash: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Device {
    id: String,
    name: String,
    token_hash: String,
    paired_at: String,
    last_sync: Option<String>,
    revoked: bool,
    #[serde(default)]
    receipts: BTreeMap<String, Receipt>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Receipt {
    digest: String,
    response: Value,
}

#[derive(Serialize, Deserialize)]
struct PairingCode {
    hash: String,
    expires_at: String,
}

#[derive(Serialize, Deserialize, Default)]
struct DeviceStore {
    #[serde(default)]
    devices: Vec<Device>,
    #[serde(default)]
    pairing: Option<PairingCode>,
    #[serde(default)]
    failed_pair_attempts: u32,
    #[serde(default)]
    attempts_reset_at: Option<String>,
}

type ApiResult = Result<Json<Value>, ApiError>;
struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
fn invalid(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message.into())
}
fn forbidden(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::FORBIDDEN, message.into())
}
fn internal(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, message.into())
}

fn token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

fn private_write(path: &Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|e| format!("Create private companion state: {e}"))?;
    file.write_all(data)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Write companion state: {e}"))?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Replace companion state: {e}"))?;
    }
    fs::rename(&temporary, path).map_err(|e| format!("Save companion state: {e}"))
}

impl HostState {
    pub fn open(config: Config) -> Result<Self, String> {
        config.validate()?;
        fs::create_dir_all(&config.data_dir)
            .map_err(|e| format!("Create Atlas data directory: {e}"))?;
        let state_path = config.data_dir.join("companion-devices.json");
        let devices = if state_path.exists() {
            serde_json::from_slice(&fs::read(&state_path).map_err(|e| e.to_string())?).map_err(
                |_| "Companion device state is unreadable; restore it from a private backup",
            )?
        } else {
            DeviceStore::default()
        };
        let admin_path = config.data_dir.join("companion-admin-token");
        let admin_token = token();
        private_write(&admin_path, admin_token.as_bytes())?;
        let atlas = atlas_core::Atlas::open(&config.data_dir)?;
        Ok(Self {
            atlas: Arc::new(Mutex::new(atlas)),
            config,
            devices: Arc::new(Mutex::new(devices)),
            mutation_lock: Arc::new(tokio::sync::Mutex::new(())),
            admin_hash: hash(&admin_token),
        })
    }
    fn save_devices(&self, devices: &DeviceStore) -> Result<(), ApiError> {
        private_write(
            &self.config.data_dir.join("companion-devices.json"),
            &serde_json::to_vec_pretty(devices)
                .map_err(|_| internal("Serialize companion state"))?,
        )
        .map_err(internal)
    }
    fn authenticate(&self, headers: &HeaderMap) -> Result<String, ApiError> {
        let bearer = headers
            .get(header::AUTHORIZATION)
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| {
                ApiError(
                    StatusCode::UNAUTHORIZED,
                    "Pair this device before synchronizing".into(),
                )
            })?;
        if bearer.len() > 256 {
            return Err(forbidden("Invalid device credential"));
        }
        let digest = hash(bearer);
        let devices = self
            .devices
            .lock()
            .map_err(|_| internal("Companion state unavailable"))?;
        devices
            .devices
            .iter()
            .find(|d| !d.revoked && equal(&d.token_hash, &digest))
            .map(|d| d.id.clone())
            .ok_or_else(|| {
                ApiError(
                    StatusCode::UNAUTHORIZED,
                    "Device credential expired or revoked; pair again".into(),
                )
            })
    }
}

fn local_host(headers: &HeaderMap, port: u16) -> bool {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    [format!("127.0.0.1:{port}"), format!("localhost:{port}")]
        .iter()
        .any(|h| h == host)
}

fn check_origin(config: &Config, headers: &HeaderMap) -> Result<(), ApiError> {
    if headers.get("sec-fetch-site").and_then(|h| h.to_str().ok()) == Some("cross-site") {
        return Err(forbidden("Cross-site companion requests are not permitted"));
    }
    let origin = headers.get(header::ORIGIN).and_then(|h| h.to_str().ok());
    if config.dev {
        if !local_host(headers, config.port) {
            return Err(forbidden("Development host accepts localhost only"));
        }
        if let Some(origin) = origin {
            let allowed = [
                format!("http://127.0.0.1:{}", config.port),
                format!("http://localhost:{}", config.port),
                "http://localhost:5173".into(),
                "http://127.0.0.1:5173".into(),
                "http://localhost:1420".into(),
                "http://127.0.0.1:1420".into(),
            ];
            if !allowed.iter().any(|v| v == origin) {
                return Err(forbidden("Untrusted development origin"));
            }
        }
    } else {
        if let Some(origin) = origin {
            if Some(origin) != config.expected_origin().as_deref() {
                return Err(forbidden("Untrusted companion origin"));
            }
        }
        let expected = url::Url::parse(config.origin.as_deref().unwrap_or_default())
            .map_err(|_| internal("Invalid host configuration"))?;
        let host = headers
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default();
        let public_authority = match expected.port() {
            Some(port) => format!("{}:{port}", expected.host_str().unwrap_or_default()),
            None => expected.host_str().unwrap_or_default().to_string(),
        };
        if !local_host(headers, config.port) && host != public_authority {
            return Err(forbidden("Unexpected companion host"));
        }
    }
    Ok(())
}

pub fn router(state: HostState) -> Router {
    let body_limit = if state.config.dev {
        32 * 1024 * 1024
    } else {
        1024 * 1024
    };
    let mut app = Router::new()
        .route("/api/health", get(health))
        .route("/api/rpc", post(rpc))
        .route("/api/pair", post(pair))
        .route("/api/device", get(device_status))
        .route("/api/local/control", post(control))
        .layer(DefaultBodyLimit::max(body_limit));
    if let Some(dir) = &state.config.static_dir {
        app = app
            .fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))));
    }
    app.with_state(state)
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL,HeaderValue::from_static("no-store")))
        .layer(SetResponseHeaderLayer::if_not_present(header::X_CONTENT_TYPE_OPTIONS,HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::if_not_present(header::REFERRER_POLICY,HeaderValue::from_static("no-referrer")))
        .layer(SetResponseHeaderLayer::if_not_present(header::CONTENT_SECURITY_POLICY,HeaderValue::from_static("default-src 'self'; connect-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'")))
}

async fn health(State(state): State<HostState>, headers: HeaderMap) -> ApiResult {
    check_origin(&state.config, &headers)?;
    Ok(Json(
        json!({"service":"Birthright: Atlas","mode":if state.config.dev {"development"} else {"paired-companion"},"requiresPairing":!state.config.dev,"origin":state.config.expected_origin()}),
    ))
}

#[derive(Deserialize)]
struct PairRequest {
    code: String,
    name: String,
}
async fn pair(
    State(state): State<HostState>,
    headers: HeaderMap,
    Json(request): Json<PairRequest>,
) -> ApiResult {
    check_origin(&state.config, &headers)?;
    if request.name.trim().is_empty() || request.name.len() > 80 || request.code.len() > 128 {
        return Err(invalid("Enter a device name and a valid pairing code"));
    }
    let mut devices = state
        .devices
        .lock()
        .map_err(|_| internal("Companion state unavailable"))?;
    let now = Utc::now();
    let reset = devices
        .attempts_reset_at
        .as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
    if reset.map(|d| d.with_timezone(&Utc) <= now).unwrap_or(true) {
        devices.failed_pair_attempts = 0;
        devices.attempts_reset_at = Some((now + Duration::minutes(1)).to_rfc3339());
    }
    if devices.failed_pair_attempts >= 10 {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pairing attempts; wait one minute".into(),
        ));
    }
    let valid = devices
        .pairing
        .as_ref()
        .map(|p| {
            equal(&p.hash, &hash(request.code.trim()))
                && chrono::DateTime::parse_from_rfc3339(&p.expires_at)
                    .map(|t| t.with_timezone(&Utc) > now)
                    .unwrap_or(false)
        })
        .unwrap_or(false);
    if !valid {
        devices.failed_pair_attempts += 1;
        state.save_devices(&devices)?;
        return Err(forbidden(
            "Pairing code is invalid, expired, or already used",
        ));
    }
    let credential = token();
    let id = uuid::Uuid::new_v4().to_string();
    devices.devices.push(Device {
        id: id.clone(),
        name: request.name.trim().into(),
        token_hash: hash(&credential),
        paired_at: now.to_rfc3339(),
        last_sync: None,
        revoked: false,
        receipts: BTreeMap::new(),
    });
    devices.pairing = None;
    devices.failed_pair_attempts = 0;
    state.save_devices(&devices)?;
    Ok(Json(
        json!({"deviceId":id,"token":credential,"name":request.name.trim(),"pairedAt":now.to_rfc3339()}),
    ))
}

async fn device_status(State(state): State<HostState>, headers: HeaderMap) -> ApiResult {
    check_origin(&state.config, &headers)?;
    let id = state.authenticate(&headers)?;
    let devices = state
        .devices
        .lock()
        .map_err(|_| internal("Companion state unavailable"))?;
    let device = devices
        .devices
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| forbidden("Unknown device"))?;
    Ok(Json(
        json!({"deviceId":id,"name":device.name,"lastSync":device.last_sync,"pairedAt":device.paired_at}),
    ))
}

#[derive(Deserialize)]
struct ControlRequest {
    action: String,
    #[serde(default)]
    device_id: Option<String>,
}
async fn control(
    State(state): State<HostState>,
    headers: HeaderMap,
    Json(request): Json<ControlRequest>,
) -> ApiResult {
    if !local_host(&headers, state.config.port) || headers.contains_key(header::ORIGIN) {
        return Err(forbidden(
            "Control is available only to the local Atlas command line",
        ));
    }
    let credential = headers
        .get("x-atlas-admin")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| forbidden("Local control credential required"))?;
    if !equal(&hash(credential), &state.admin_hash) {
        return Err(forbidden("Invalid local control credential"));
    }
    let mut devices = state
        .devices
        .lock()
        .map_err(|_| internal("Companion state unavailable"))?;
    match request.action.as_str() {
        "pair" => {
            let mut random = [0u8; 12];
            rand::rngs::OsRng.fill_bytes(&mut random);
            let code = URL_SAFE_NO_PAD.encode(random);
            let expires = (Utc::now() + Duration::minutes(10)).to_rfc3339();
            devices.pairing = Some(PairingCode {
                hash: hash(&code),
                expires_at: expires.clone(),
            });
            state.save_devices(&devices)?;
            Ok(Json(
                json!({"code":code,"expiresAt":expires,"url":state.config.expected_origin().map(|o|format!("{o}/companion"))}),
            ))
        }
        "devices" => Ok(Json(
            json!({"devices":devices.devices.iter().map(|d|json!({"id":d.id,"name":d.name,"pairedAt":d.paired_at,"lastSync":d.last_sync,"revoked":d.revoked})).collect::<Vec<_>>()}),
        )),
        "revoke" => {
            let id = request
                .device_id
                .ok_or_else(|| invalid("Device ID required"))?;
            let device = devices
                .devices
                .iter_mut()
                .find(|d| d.id == id)
                .ok_or_else(|| invalid("Device not found"))?;
            device.revoked = true;
            device.receipts.clear();
            state.save_devices(&devices)?;
            Ok(Json(json!({"revoked":true,"deviceId":id})))
        }
        _ => Err(invalid("Unknown local control action")),
    }
}

#[derive(Deserialize)]
struct RpcRequest {
    method: String,
    #[serde(default)]
    payload: Value,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
}
fn projected_record(record: &Value) -> Option<Value> {
    let kind = record.get("kind")?.as_str()?;
    let allowed = matches!(
        kind,
        "trip"
            | "stay"
            | "transport"
            | "objective"
            | "workBlock"
            | "expense"
            | "reminder"
            | "place"
    ) || (kind == "document"
        && record.get("companionAvailable").and_then(Value::as_bool) == Some(true));
    if !allowed {
        return None;
    }
    let mut record = record.clone();
    if let Some(map) = record.as_object_mut() {
        for key in [
            "sourceId",
            "sourceIds",
            "sourceMessageId",
            "sourceMessageIds",
            "sourceText",
            "evidence",
            "path",
            "storagePath",
            "storageName",
            "contentBase64",
            "token",
            "accessToken",
            "refreshToken",
        ] {
            map.remove(key);
        }
    }
    Some(record)
}
fn projected_settings(settings: &Value) -> Value {
    let mut result = serde_json::Map::new();
    for key in [
        "currency",
        "context",
        "officeTimezone",
        "currentTimezone",
        "currentLocation",
        "workStart",
        "workEnd",
        "workHours",
        "overlapHours",
        "workdays",
    ] {
        if let Some(value) = settings.get(key) {
            result.insert(key.into(), value.clone());
        }
    }
    Value::Object(result)
}

fn companion_call(atlas: &mut atlas_core::Atlas, request: &RpcRequest) -> Result<Value, String> {
    match request.method.as_str() {
        "snapshot" => {
            let snapshot = atlas.call("snapshot", json!({}))?;
            let records = snapshot
                .get("records")
                .and_then(Value::as_array)
                .ok_or("Invalid snapshot")?
                .iter()
                .filter_map(projected_record)
                .collect::<Vec<_>>();
            Ok(json!({"records":records,"settings":projected_settings(&snapshot["settings"])}))
        }
        "budget.summary" | "work.window" | "calendar.export" => {
            atlas.call(&request.method, request.payload.clone())
        }
        "documents.read" => {
            let id = request
                .payload
                .get("id")
                .and_then(Value::as_str)
                .ok_or("Document ID required")?;
            let records = atlas.call("records.list", json!({"kind":"document"}))?;
            if !records
                .as_array()
                .ok_or("Invalid document list")?
                .iter()
                .any(|r| r["id"] == id && r["companionAvailable"] == true)
            {
                return Err(
                    "This document has not been selected for companion access on the desktop"
                        .into(),
                );
            }
            let document = atlas.call("documents.read", json!({"id":id}))?;
            if document
                .get("contentBase64")
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or(0)
                > MAX_DOCUMENT_BYTES * 4 / 3 + 4
            {
                return Err("Document exceeds the companion limit of 8 MiB".into());
            }
            Ok(document)
        }
        "records.save" => {
            let record = request.payload.get("record").ok_or("Record required")?;
            let expected = request
                .payload
                .get("expectedVersion")
                .and_then(Value::as_u64)
                .ok_or("Offline writes require expectedVersion")?;
            let id = record
                .get("id")
                .and_then(Value::as_str)
                .ok_or("Offline writes require a stable record ID")?;
            if uuid::Uuid::parse_str(id).is_err() {
                return Err("Record ID must be a UUID".into());
            }
            match record.get("kind").and_then(Value::as_str) {
                Some("expense") => {}
                Some("objective") => {
                    if expected == 0 {
                        return Err("Create objectives on the desktop; the companion can mark existing objectives complete".into());
                    }
                    let records = atlas.call("records.list", json!({"kind":"objective"}))?;
                    let old = records
                        .as_array()
                        .ok_or("Invalid objectives")?
                        .iter()
                        .find(|r| r["id"] == id)
                        .ok_or("Objective no longer exists")?;
                    let mut updated = old.clone();
                    let status = record
                        .get("status")
                        .and_then(Value::as_str)
                        .ok_or("Objective status required")?;
                    if !matches!(status, "planned" | "done") {
                        return Err("Invalid objective status".into());
                    }
                    updated["status"] = json!(status);
                    return atlas.call(
                        "records.save",
                        json!({"record":updated,"expectedVersion":expected}),
                    );
                }
                _ => {
                    return Err(
                        "Companion writes are limited to expenses and objective completion".into(),
                    )
                }
            }
            if expected > 0 {
                let records = atlas.call("records.list", json!({"kind":"expense"}))?;
                if !records
                    .as_array()
                    .ok_or("Invalid expenses")?
                    .iter()
                    .any(|r| r["id"] == id)
                {
                    return Err("Only existing expenses can be edited here".into());
                }
            }
            atlas.call("records.save", request.payload.clone())
        }
        _ => Err("This service is available only on the desktop".into()),
    }
}

async fn rpc(
    State(state): State<HostState>,
    headers: HeaderMap,
    Json(request): Json<RpcRequest>,
) -> ApiResult {
    check_origin(&state.config, &headers)?;
    if state.config.dev && request.method == "ai.cancel" {
        return Ok(Json(atlas_core::ai::cancel()));
    }
    let device_id = if state.config.dev {
        None
    } else {
        Some(state.authenticate(&headers)?)
    };
    // Keep receipt lookup, the domain mutation, and receipt persistence together.
    // Concurrent retries with the same request ID must not race into conflicts.
    let _mutation_guard = if device_id.is_some() && request.method == "records.save" {
        Some(state.mutation_lock.lock().await)
    } else {
        None
    };
    if _mutation_guard.is_some() {
        state.authenticate(&headers)?;
    }
    let request_digest = hash(
        &serde_json::to_string(&json!({"method":request.method,"payload":request.payload}))
            .map_err(|_| invalid("Invalid request"))?,
    );
    if device_id.is_some() && request.method == "records.save" {
        let key = request
            .request_id
            .as_deref()
            .ok_or_else(|| invalid("Offline mutations require a unique requestId"))?;
        if key.len() > 80 || uuid::Uuid::parse_str(key).is_err() {
            return Err(invalid("requestId must be a UUID"));
        }
        let devices = state
            .devices
            .lock()
            .map_err(|_| internal("Companion state unavailable"))?;
        if let Some(receipt) = devices
            .devices
            .iter()
            .find(|d| Some(&d.id) == device_id.as_ref())
            .and_then(|d| d.receipts.get(key))
        {
            if receipt.digest != request_digest {
                return Err(forbidden("Request ID already used with different content"));
            }
            return Ok(Json(receipt.response.clone()));
        }
    }
    let core = state.atlas.clone();
    let dev = state.config.dev;
    let receipt_id = request.request_id.clone();
    let method = request.method.clone();
    let result = tokio::task::spawn_blocking(move || {
        if dev && request.method == "releases.history" {
            return atlas_core::releases::query(&core, &request.payload);
        }
        let mut atlas = core
            .lock()
            .map_err(|_| "Atlas service unavailable".to_string())?;
        if dev {
            atlas.call(&request.method, request.payload)
        } else {
            companion_call(&mut atlas, &request)
        }
    })
    .await
    .map_err(|_| internal("Atlas service task failed"))?;
    let result = result.map_err(|error| {
        let status = if error.to_ascii_lowercase().contains("conflict")
            || error.to_ascii_lowercase().contains("version")
        {
            StatusCode::CONFLICT
        } else {
            StatusCode::BAD_REQUEST
        };
        ApiError(status, error)
    })?;
    if let Some(device_id) = device_id {
        let mut devices = state
            .devices
            .lock()
            .map_err(|_| internal("Companion state unavailable"))?;
        let device = devices
            .devices
            .iter_mut()
            .find(|d| d.id == device_id && !d.revoked)
            .ok_or_else(|| forbidden("Device revoked during request"))?;
        device.last_sync = Some(Utc::now().to_rfc3339());
        if method == "records.save" {
            if let Some(id) = receipt_id {
                if device.receipts.len() >= 1000 {
                    if let Some(first) = device.receipts.keys().next().cloned() {
                        device.receipts.remove(&first);
                    }
                }
                device.receipts.insert(
                    id,
                    Receipt {
                        digest: request_digest,
                        response: result.clone(),
                    },
                );
            }
        }
        state.save_devices(&devices)?;
    }
    Ok(Json(result))
}

/// Commands use the private local control credential; it is never exposed to a phone.
pub fn local_control(
    data_dir: &Path,
    port: u16,
    action: &str,
    device_id: Option<&str>,
) -> Result<Value, String> {
    let token = fs::read_to_string(data_dir.join("companion-admin-token")).map_err(|_| {
        "Start atlas-host with this data directory before issuing companion control commands"
    })?;
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(format!("http://127.0.0.1:{port}/api/local/control"))
        .header("x-atlas-admin", token.trim())
        .json(&json!({"action":action,"device_id":device_id}))
        .send()
        .map_err(|e| format!("Reach local Atlas host: {e}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|_| "Local host returned an invalid response")?;
    if !status.is_success() {
        return Err(body["error"]
            .as_str()
            .unwrap_or("Local command failed")
            .into());
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn state(dev: bool) -> (tempfile::TempDir, HostState) {
        let temp = tempfile::tempdir().unwrap();
        let config = Config {
            data_dir: temp.path().into(),
            static_dir: if dev { None } else { Some(temp.path().into()) },
            port: 47832,
            dev,
            origin: if dev {
                None
            } else {
                Some("https://atlas.example.test".into())
            },
        };
        let state = HostState::open(config).unwrap();
        (temp, state)
    }
    async fn request(
        state: &HostState,
        path: &str,
        body: Value,
        token: Option<&str>,
        origin: Option<&str>,
        admin: bool,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header("host", "127.0.0.1:47832")
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        if let Some(origin) = origin {
            builder = builder.header("origin", origin);
        }
        if admin {
            builder = builder.header(
                "x-atlas-admin",
                fs::read_to_string(state.config.data_dir.join("companion-admin-token")).unwrap(),
            );
        }
        let response = router(state.clone())
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&body).unwrap())
    }
    async fn paired(state: &HostState) -> Value {
        let (status, code) = request(
            state,
            "/api/local/control",
            json!({"action":"pair"}),
            None,
            None,
            true,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, device) = request(
            state,
            "/api/pair",
            json!({"name":"Synthetic phone","code":code["code"]}),
            None,
            Some("https://atlas.example.test"),
            false,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        device
    }
    #[test]
    fn requires_trusted_https_and_explicit_mode() {
        let mut config = Config {
            data_dir: PathBuf::from("/tmp/unused"),
            static_dir: Some(PathBuf::from("dist")),
            port: 47832,
            dev: false,
            origin: None,
        };
        assert!(config.validate().is_err());
        config.origin = Some("http://192.168.1.1".into());
        assert!(config.validate().is_err());
        config.origin = Some("https://atlas.example.test/path".into());
        assert!(config.validate().is_err());
        config.origin = Some("https://atlas.example.test".into());
        assert!(config.validate().is_ok());
        config.dev = true;
        assert!(config.validate().is_err());
    }
    #[tokio::test]
    async fn refuses_unauthenticated_and_cross_origin_requests() {
        let (_temp, state) = state(false);
        let (status, _) = request(
            &state,
            "/api/rpc",
            json!({"method":"snapshot"}),
            None,
            None,
            false,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let device = paired(&state).await;
        let token = device["token"].as_str().unwrap();
        let (status, _) = request(
            &state,
            "/api/rpc",
            json!({"method":"snapshot"}),
            Some(token),
            Some("https://evil.example"),
            false,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, body) = request(
            &state,
            "/api/rpc",
            json!({"method":"email.status"}),
            Some(token),
            Some("https://atlas.example.test"),
            false,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("desktop"));
    }
    #[tokio::test]
    async fn pairing_is_single_use_and_revocation_is_effective() {
        let (_temp, state) = state(false);
        let (_, code) = request(
            &state,
            "/api/local/control",
            json!({"action":"pair"}),
            None,
            None,
            true,
        )
        .await;
        let payload = json!({"name":"Test phone","code":code["code"]});
        let (status, device) =
            request(&state, "/api/pair", payload.clone(), None, None, false).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request(&state, "/api/pair", payload, None, None, false).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = request(
            &state,
            "/api/rpc",
            json!({"method":"snapshot"}),
            device["token"].as_str(),
            None,
            false,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request(
            &state,
            "/api/local/control",
            json!({"action":"revoke","device_id":device["deviceId"]}),
            None,
            None,
            true,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request(
            &state,
            "/api/rpc",
            json!({"method":"snapshot"}),
            device["token"].as_str(),
            None,
            false,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    #[test]
    fn snapshot_drops_private_sources_connections_and_paths() {
        assert!(
            projected_record(&json!({"kind":"source","sourceText":"synthetic mail"})).is_none()
        );
        assert!(projected_record(&json!({"kind":"connection","accessToken":"test"})).is_none());
        assert!(projected_record(&json!({"kind":"document","path":"/private/doc"})).is_none());
        let record=projected_record(&json!({"kind":"document","companionAvailable":true,"path":"/private/doc","filename":"ticket.pdf"})).unwrap();
        assert!(record.get("path").is_none());
        assert_eq!(record["filename"], "ticket.pdf");
        assert_eq!(
            projected_settings(&json!({"currency":"USD","emailClientSecret":"synthetic"})),
            json!({"currency":"USD"})
        );
    }
    #[tokio::test]
    async fn expense_queue_retries_are_idempotent_and_conflicts_stay_visible() {
        let (_temp, state) = state(false);
        let device = paired(&state).await;
        let token = device["token"].as_str();
        let id = uuid::Uuid::new_v4().to_string();
        let request_id = uuid::Uuid::new_v4().to_string();
        let record = json!({"id":id,"kind":"expense","name":"Synthetic groceries","date":"2026-10-22","currency":"USD","amountMinor":2100,"category":"groceries"});
        let create = json!({"method":"records.save","requestId":request_id,"payload":{"record":record,"expectedVersion":0}});
        let ((status, saved), (retry_status, concurrent_replay)) = tokio::join!(
            request(&state, "/api/rpc", create.clone(), token, None, false),
            request(&state, "/api/rpc", create.clone(), token, None, false)
        );
        assert_eq!(status, StatusCode::OK, "{saved}");
        assert_eq!(retry_status, StatusCode::OK, "{concurrent_replay}");
        assert_eq!(saved, concurrent_replay);
        let (status, replayed) = request(&state, "/api/rpc", create, token, None, false).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(saved, replayed);
        let (status,error)=request(&state,"/api/rpc",json!({"method":"records.save","requestId":uuid::Uuid::new_v4().to_string(),"payload":{"record":record,"expectedVersion":0}}),token,None,false).await;
        assert_eq!(status, StatusCode::CONFLICT, "{error}");
        let (status,_)=request(&state,"/api/rpc",json!({"method":"records.save","requestId":request_id,"payload":{"record":{"kind":"expense"},"expectedVersion":0}}),token,None,false).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    #[tokio::test]
    async fn dev_mode_rejects_dns_rebinding_and_untrusted_web_origins() {
        let (_temp, state) = state(true);
        let (status, _) = request(
            &state,
            "/api/rpc",
            json!({"method":"snapshot"}),
            None,
            Some("https://evil.example"),
            false,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/rpc")
                    .header("host", "attacker.example:47832")
                    .header("content-type", "application/json")
                    .body(Body::from("{\"method\":\"snapshot\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn companion_shell_is_successful_so_service_worker_can_install() {
        let (temp, state) = state(false);
        fs::write(
            temp.path().join("index.html"),
            "<!doctype html><title>Atlas</title>",
        )
        .unwrap();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/companion")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html"));
    }
}
