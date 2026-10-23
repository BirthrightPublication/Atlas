//! Read-only mailbox connectors. Secrets live exclusively in the OS secret store.
//! Call `acknowledge` only after every returned message is durably recorded.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, NaiveDate, Utc};
use rand::RngCore;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use url::Url;

const MAX_MESSAGE: usize = 12 * 1024 * 1024;
const MAX_RESPONSE: usize = 20 * 1024 * 1024;
const BATCH: usize = 30;
const DEFAULT_QUERY: &str =
    "reservation OR booking OR confirmation OR itinerary OR cancellation OR refund";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    provider: String,
    #[serde(default = "default_interval")]
    sync_minutes: i64,
    #[serde(default)]
    client_id: String,
    #[serde(default = "common")]
    tenant: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    username: String,
    #[serde(default)]
    tls_mode: String,
    #[serde(default = "inbox")]
    folder: String,
    #[serde(default)]
    certificate_pem: Option<String>,
    #[serde(default)]
    filter: Filter,
}
fn default_interval() -> i64 {
    5
}
fn common() -> String {
    "common".into()
}
fn inbox() -> String {
    "INBOX".into()
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Filter {
    #[serde(default)]
    senders: Vec<String>,
    #[serde(default)]
    domains: Vec<String>,
    #[serde(default)]
    excluded_senders: Vec<String>,
    #[serde(default)]
    subject_keywords: Vec<String>,
    #[serde(default)]
    reservation_keywords: Vec<String>,
    #[serde(default)]
    before_date: Option<String>,
    #[serde(default)]
    after_date: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    label_or_folder: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    config: Option<Config>,
    #[serde(default)]
    connected: bool,
    #[serde(default)]
    paused: bool,
    #[serde(default)]
    cursor: Value,
    #[serde(default)]
    last_sync: Option<String>,
    #[serde(default)]
    last_attempt: Option<String>,
    #[serde(default)]
    statistics: Value,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    generation: u64,
    #[serde(skip)]
    authorization_pending: bool,
    #[serde(skip)]
    syncing: bool,
}
#[derive(Default, Serialize, Deserialize)]
struct Credentials {
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_at: i64,
}
trait SecretStore: Send + Sync {
    fn load(&self) -> Result<Option<Credentials>, String>;
    fn save(&self, credentials: &Credentials) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}
struct OsSecrets {
    account: String,
}
impl OsSecrets {
    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new("Birthright.Atlas.email", &self.account).map_err(|_| "OS secret storage is unavailable. Unlock your login keychain/credential manager and retry.".into())
    }
}
impl SecretStore for OsSecrets {
    fn load(&self) -> Result<Option<Credentials>, String> {
        match self.entry()?.get_password() {
            Ok(v) => serde_json::from_str(&v).map(Some).map_err(|_| "Stored mailbox credentials could not be decoded; reconnect the account.".into()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("OS secret storage is unavailable or locked; no plaintext credential fallback is used.".into()),
        }
    }
    fn save(&self, v: &Credentials) -> Result<(), String> {
        self.entry()?
            .set_password(
                &serde_json::to_string(v).map_err(|_| "Could not encode mailbox credentials")?,
            )
            .map_err(|_| {
                "Could not save credentials to the OS secret store. Unlock it and retry.".into()
            })
    }
    fn delete(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Could not remove mailbox credentials from the OS secret store.".into()),
        }
    }
}

#[derive(Clone)]
pub struct EmailService {
    directory: PathBuf,
    state: Arc<Mutex<Metadata>>,
    secrets: Arc<dyn SecretStore>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailMessage {
    pub id: String,
    pub provider: String,
    pub source_url: Option<String>,
    pub text: String,
    pub format: String,
    pub received_at: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncBatch {
    pub messages: Vec<EmailMessage>,
    pub has_more: bool,
    pub examined: usize,
    cursor: Value,
    generation: u64,
}
impl EmailService {
    pub fn new(data_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_dir).map_err(|_| "Could not create mailbox metadata directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(data_dir, fs::Permissions::from_mode(0o700))
                .map_err(|_| "Could not protect mailbox directory")?;
        }
        let account = format!(
            "profile-{:x}",
            Sha256::digest(data_dir.to_string_lossy().as_bytes())
        );
        Self::with_store(data_dir, Arc::new(OsSecrets { account }))
    }
    fn with_store(data_dir: &Path, secrets: Arc<dyn SecretStore>) -> Result<Self, String> {
        let path = data_dir.join("email.json");
        let state = if path.exists() {
            serde_json::from_slice(&fs::read(&path).map_err(|_| "Could not read mailbox metadata")?)
                .map_err(|_| "Invalid mailbox metadata; restore from backup before reconnecting")?
        } else {
            Metadata::default()
        };
        Ok(Self {
            directory: data_dir.into(),
            state: Arc::new(Mutex::new(state)),
            secrets,
        })
    }
    fn state(&self) -> Result<std::sync::MutexGuard<'_, Metadata>, String> {
        self.state
            .lock()
            .map_err(|_| "Mailbox state is unavailable".into())
    }
    fn persist(&self, state: &Metadata) -> Result<(), String> {
        let path = self.directory.join("email.json");
        let tmp = self.directory.join("email.json.tmp");
        let data = serde_json::to_vec_pretty(state)
            .map_err(|_| "Could not encode mailbox configuration")?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|_| "Could not save mailbox configuration")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| "Could not protect mailbox metadata")?;
        }
        file.write_all(&data)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Could not save mailbox configuration")?;
        drop(file);
        // Windows rename cannot replace an existing file. Keep the existing file until the new
        // content is fully synced, then replace it. Metadata never contains credential values.
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(&path).map_err(|_| "Could not replace mailbox configuration")?;
        }
        fs::rename(tmp, path).map_err(|_| "Could not commit mailbox configuration")?;
        Ok(())
    }
    pub fn status(&self) -> Result<Value, String> {
        let state = self.state()?;
        let secure = self.secrets.load();
        let present = secure.as_ref().ok().and_then(|v| v.as_ref()).is_some();
        let c = state.config.as_ref();
        Ok(json!({
            "configured": c.is_some(), "connected": state.connected && present,
            "paused": state.paused, "syncing": state.syncing,
            "authorizationPending": state.authorization_pending,
            "lastSync": state.last_sync, "lastAttempt": state.last_attempt, "lastError": state.last_error,
            "statistics": state.statistics, "syncMinutes": c.map(|v| v.sync_minutes).unwrap_or(5),
            "connectionState": if state.authorization_pending { "connecting" } else if state.syncing { "synchronizing" } else if state.last_error.as_ref().is_some_and(|e| e.contains("401") || e.to_lowercase().contains("reconnect")) { "reauthorization required" } else if state.last_error.is_some() { "connection error" } else if !state.connected || !present { "not connected" } else if state.paused { "paused" } else { "connected" },
            "secureStorageAvailable": secure.is_ok(),
            "provider": c.map(|v| &v.provider), "clientId": c.map(|v| &v.client_id),
            "tenant": c.map(|v| &v.tenant), "host": c.map(|v| &v.host), "port": c.map(|v| v.port),
            "username": c.map(|v| &v.username), "tlsMode": c.map(|v| &v.tls_mode),
            "folder": c.map(|v| &v.folder), "filter": c.map(|v| &v.filter),
            "hasCustomTrustCertificate": c.and_then(|v| v.certificate_pem.as_ref()).is_some(),
            "scope": if c.is_some_and(|v| v.provider == "outlook") { "openid offline_access https://graph.microsoft.com/Mail.Read" } else if c.is_some_and(|v| v.provider == "gmail") { "https://www.googleapis.com/auth/gmail.readonly" } else { "Read-only IMAP commands (account credentials may grant broader access)" },
            "automaticSyncAvailable": state.connected && present && !state.paused,
            "requiresRunningHost": true,
            "notice": "Retrieval filters limit fetched messages, not the account permission grant."
        }))
    }
    pub fn configure(&self, payload: &Value) -> Result<Value, String> {
        let provider = string(payload, "provider")?.to_lowercase();
        if !["gmail", "outlook", "proton-bridge", "imap"].contains(&provider.as_str()) {
            return Err("Choose Gmail, Outlook, Proton Bridge, or supported IMAP.".into());
        }
        let mut config: Config = serde_json::from_value(payload.clone())
            .map_err(|_| "Invalid mailbox configuration fields")?;
        config.provider = provider;
        if config.folder.is_empty() {
            config.folder = inbox();
        }
        if !(5..=1440).contains(&config.sync_minutes) {
            return Err("Sync interval must be between 5 and 1440 minutes.".into());
        }
        for list in [
            &config.filter.domains,
            &config.filter.excluded_senders,
            &config.filter.subject_keywords,
            &config.filter.reservation_keywords,
        ] {
            if list.len() > 20
                || list
                    .iter()
                    .any(|v| v.is_empty() || v.len() > 254 || v.contains(['\r', '\n', '\0']))
            {
                return Err("Use up to 20 bounded filter values per category.".into());
            }
        }
        if let Some(date) = &config.filter.before_date {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| "beforeDate must be YYYY-MM-DD")?;
        }
        if config
            .filter
            .before_date
            .as_ref()
            .zip(config.filter.after_date.as_ref())
            .is_some_and(|(end, start)| end <= start)
        {
            return Err("End date must follow the start date.".into());
        }
        if config.filter.senders.len() > 20
            || config
                .filter
                .senders
                .iter()
                .any(|s| s.len() > 254 || s.contains(['\r', '\n', '\0']))
        {
            return Err("Provide up to 20 valid sender addresses.".into());
        }
        if let Some(date) = &config.filter.after_date {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| "afterDate must be YYYY-MM-DD")?;
        }
        if config.filter.query.as_ref().is_some_and(|q| q.len() > 1000) {
            return Err("Mailbox query is too long.".into());
        }
        if config.provider == "imap" || config.provider == "proton-bridge" {
            if config.host.is_empty() {
                config.host = if config.provider == "proton-bridge" {
                    "127.0.0.1".into()
                } else {
                    return Err("Enter your provider's IMAP hostname.".into());
                };
            }
            if !config
                .host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            {
                return Err("Invalid IMAP hostname.".into());
            }
            if config.provider == "proton-bridge"
                && !["localhost", "127.0.0.1"].contains(&config.host.as_str())
            {
                return Err(
                    "Proton Bridge must run on this computer; use localhost or 127.0.0.1.".into(),
                );
            }
            if config.port == 0 {
                config.port = if config.provider == "proton-bridge" {
                    1143
                } else {
                    993
                };
            }
            if config.tls_mode.is_empty() {
                config.tls_mode = if config.provider == "proton-bridge" {
                    "starttls".into()
                } else {
                    "tls".into()
                };
            }
            if !["tls", "starttls"].contains(&config.tls_mode.as_str()) {
                return Err("IMAP requires TLS or STARTTLS; plaintext is unsupported.".into());
            }
            if config.username.is_empty() || config.username.len() > 320 {
                return Err("Enter the IMAP username shown by your provider or Bridge.".into());
            }
            quote_imap(&config.username)?;
            quote_imap(&config.folder)?;
            // A settings form does not receive the PEM certificate in status.
            // Preserve its existing trust anchor for the same endpoint when the
            // field is omitted; explicit null is an intentional removal.
            if payload.get("certificatePem").is_none() {
                if let Some(previous) = self.state()?.config.as_ref() {
                    if previous.provider == config.provider && previous.host == config.host {
                        config.certificate_pem = previous.certificate_pem.clone();
                    }
                }
            }
            if let Some(pem) = &config.certificate_pem {
                if pem.len() > 32768 {
                    return Err("Trust certificate is too large.".into());
                }
                if pem.contains("PRIVATE KEY") {
                    return Err(
                        "Paste only the public trust certificate, never a private key.".into(),
                    );
                }
                native_tls::Certificate::from_pem(pem.as_bytes())
                    .map_err(|_| "Invalid PEM trust certificate")?;
            }
        } else {
            if config.client_id.trim().is_empty() || config.client_id.len() > 512 {
                return Err("Enter your registered native application's OAuth client ID.".into());
            }
            if !config
                .tenant
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            {
                return Err("Invalid Microsoft tenant identifier.".into());
            }
        }
        let mut state = self.state()?;
        if state.syncing {
            return Err("Wait for the current sync before changing filters.".into());
        }
        let changed = state
            .config
            .as_ref()
            .is_none_or(|c| serde_json::to_value(c).ok() != serde_json::to_value(&config).ok());
        let identity_changed = state.config.as_ref().is_none_or(|c| {
            c.provider != config.provider
                || c.client_id != config.client_id
                || c.tenant != config.tenant
                || c.host != config.host
                || c.username != config.username
        });
        let new_password = payload
            .get("password")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty());
        let new_secret = payload
            .get("clientSecret")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty());
        if new_password.is_some() || new_secret.is_some() {
            let mut credentials = if identity_changed {
                Credentials::default()
            } else {
                self.secrets.load()?.unwrap_or_default()
            };
            if let Some(v) = payload
                .get("password")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
            {
                if v.len() > 16384 || v.contains(['\r', '\n', '\0']) {
                    return Err("Invalid IMAP password format.".into());
                }
                credentials.password = Some(v.into());
            }
            if let Some(v) = payload
                .get("clientSecret")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
            {
                if v.len() > 4096 {
                    return Err("OAuth client secret is too long.".into());
                }
                credentials.client_secret = Some(v.into());
            }
            self.secrets.save(&credentials)?;
        } else if identity_changed && state.config.is_some() {
            self.secrets.delete()?;
        }
        if changed {
            state.cursor = Value::Null;
            if identity_changed {
                state.connected = false;
            }
            state.last_sync = None;
            state.generation += 1;
        }
        state.config = Some(config);
        state.last_error = None;
        state.authorization_pending = false;
        self.persist(&state)?;
        drop(state);
        self.status()
    }
    pub fn pause(&self, paused: bool) -> Result<Value, String> {
        let mut state = self.state()?;
        state.paused = paused;
        self.persist(&state)?;
        drop(state);
        self.status()
    }
    pub fn disconnect(&self) -> Result<Value, String> {
        let mut state = self.state()?;
        self.secrets.delete()?;
        state.connected = false;
        state.authorization_pending = false;
        state.cursor = Value::Null;
        state.generation += 1;
        state.last_error = None;
        self.persist(&state)?;
        drop(state);
        self.status()
    }
    pub fn connect(&self) -> Result<Value, String> {
        let result = self.connect_inner();
        if let Err(ref e) = result {
            let mut state = self.state()?;
            state.connected = false;
            state.last_error = Some(e.clone());
            self.persist(&state)?;
        }
        result
    }
    fn connect_inner(&self) -> Result<Value, String> {
        let (config, generation) = {
            let s = self.state()?;
            (
                s.config.clone().ok_or("Configure a mailbox first")?,
                s.generation,
            )
        };
        if config.provider == "imap" || config.provider == "proton-bridge" {
            let credentials = self
                .secrets
                .load()?
                .ok_or("Save your mailbox/Bridge password in settings first")?;
            let mut session = ImapSession::connect(
                &config,
                credentials
                    .password
                    .as_deref()
                    .ok_or("Missing IMAP password")?,
            )?;
            session.examine(&config.folder)?;
            let _ = session.command("LOGOUT");
            let mut s = self.state()?;
            if s.generation != generation {
                return Err("Mailbox configuration changed; reconnect.".into());
            }
            s.connected = true;
            s.last_error = None;
            self.persist(&s)?;
            return Ok(json!({"connected":true}));
        }
        // Probe protected storage before sending the owner through an OAuth flow.
        let existing = self.secrets.load()?.unwrap_or_default();
        if config.provider == "gmail" && existing.client_secret.is_none() {
            return Err("Google Desktop OAuth requires the client secret from your own OAuth client registration; save it securely in mailbox settings first.".into());
        }
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|_| "Could not open the local OAuth callback listener")?;
        listener
            .set_nonblocking(true)
            .map_err(|_| "Could not initialize the OAuth callback listener")?;
        let redirect = format!(
            "http://{}:{}/oauth/callback",
            if config.provider == "outlook" {
                "localhost"
            } else {
                "127.0.0.1"
            },
            listener
                .local_addr()
                .map_err(|_| "Missing callback address")?
                .port()
        );
        let verifier = random_secret();
        let state_token = random_secret();
        let url = authorization_url(&config, &redirect, &verifier, &state_token)?;
        {
            let mut s = self.state()?;
            if s.authorization_pending {
                return Err("A mailbox authorization is already waiting in the browser.".into());
            }
            s.authorization_pending = true;
            s.last_error = None;
        }
        let this = self.clone();
        thread::spawn(move || {
            let result = this.finish_oauth(
                listener,
                config,
                redirect,
                verifier,
                state_token,
                generation,
            );
            if let Ok(mut s) = this.state() {
                if s.generation == generation {
                    s.authorization_pending = false;
                    if let Err(err) = result {
                        s.connected = false;
                        s.last_error = Some(err);
                    }
                    let _ = this.persist(&s);
                }
            }
        });
        Ok(json!({"authorizationUrl":url, "authorizationPending":true, "expiresInSeconds":300}))
    }
    fn finish_oauth(
        &self,
        listener: TcpListener,
        config: Config,
        redirect: String,
        verifier: String,
        expected_state: String,
        generation: u64,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(300);
        let code = loop {
            if Instant::now() >= deadline {
                return Err("Mailbox authorization timed out. Reconnect to try again.".into());
            }
            if self.state()?.generation != generation {
                return Err("Mailbox authorization was cancelled.".into());
            }
            match listener.accept() {
                Ok((mut stream, address)) => {
                    if !address.ip().is_loopback() {
                        continue;
                    }
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .map_err(|_| "Could not read OAuth callback")?;
                    let mut reader = BufReader::new(&stream);
                    let mut line = String::new();
                    reader
                        .by_ref()
                        .take(8192)
                        .read_line(&mut line)
                        .map_err(|_| "Could not read OAuth callback")?;
                    let target = line.split_whitespace().nth(1).unwrap_or("");
                    let parsed = parse_callback(target, &expected_state);
                    let body = if parsed.is_ok() {
                        "Authorization received. Return to Birthright: Atlas to check connection status."
                    } else {
                        "This authorization response was not accepted. Return to Birthright: Atlas and reconnect."
                    };
                    let response = format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", if parsed.is_ok() { "200 OK" } else { "400 Bad Request" }, body.len(), body);
                    let _ = stream.write_all(response.as_bytes());
                    match parsed {
                        Ok(code) => break code,
                        Err(error) if error == "Mailbox authorization was declined" => {
                            return Err(error)
                        }
                        Err(_) => {}
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100))
                }
                Err(_) => return Err("The local OAuth callback listener failed.".into()),
            }
        };
        let mut credentials = self.secrets.load()?.unwrap_or_default();
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("client_id", &config.client_id),
            ("redirect_uri", &redirect),
            ("code_verifier", &verifier),
        ];
        if config.provider == "gmail" {
            if let Some(secret) = &credentials.client_secret {
                form.push(("client_secret", secret));
            }
        }
        let response = http_client()?
            .post(token_endpoint(&config))
            .form(&form)
            .send()
            .map_err(|_| {
                "OAuth token exchange could not reach the provider. Check network access and retry."
            })?;
        let token = read_json_response(response)?;
        apply_token(&mut credentials, &token)?;
        if self.state()?.generation != generation {
            return Err("Mailbox authorization was cancelled.".into());
        }
        // Verify mailbox access before presenting Connected. Consent alone is insufficient.
        let token = credentials
            .access_token
            .as_deref()
            .ok_or("Missing access token")?;
        let probe = if config.provider == "gmail" {
            "https://gmail.googleapis.com/gmail/v1/users/me/profile"
        } else {
            "https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages?$top=1&$select=id"
        };
        self.get_json(probe, token)?;
        let mut s = self.state()?;
        if s.generation != generation {
            return Err("Mailbox authorization was cancelled.".into());
        }
        self.secrets.save(&credentials)?;
        s.connected = true;
        s.last_error = None;
        self.persist(&s)
    }
    fn access_token(&self, config: &Config, generation: u64) -> Result<String, String> {
        let mut credentials = self
            .secrets
            .load()?
            .ok_or("Reconnect your mailbox; saved credentials are unavailable")?;
        if credentials.expires_at > Utc::now().timestamp() + 60 {
            return credentials
                .access_token
                .clone()
                .ok_or_else(|| "Reconnect your mailbox".into());
        }
        let refresh = credentials
            .refresh_token
            .as_ref()
            .ok_or("Reconnect your mailbox; no refresh token is available")?;
        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("client_id", config.client_id.as_str()),
            ("refresh_token", refresh.as_str()),
        ];
        if config.provider == "gmail" {
            if let Some(secret) = &credentials.client_secret {
                form.push(("client_secret", secret));
            }
        }
        let response = http_client()?
            .post(token_endpoint(config))
            .form(&form)
            .send()
            .map_err(|_| "Mailbox token refresh failed to reach the provider")?;
        let token = read_json_response(response)?;
        apply_token(&mut credentials, &token)?;
        let state = self.state()?;
        if state.generation != generation || !state.connected {
            return Err("Mailbox disconnected during token refresh; reconnect to continue.".into());
        }
        self.secrets.save(&credentials)?;
        drop(state);
        credentials
            .access_token
            .ok_or_else(|| "The provider did not return an access token".into())
    }
    pub fn sync(&self) -> Result<SyncBatch, String> {
        let (config, cursor, generation) = {
            let mut s = self.state()?;
            if s.paused {
                return Err("Mailbox import is paused.".into());
            }
            if !s.connected {
                return Err("Connect a mailbox before syncing.".into());
            }
            if s.syncing {
                return Err("A mailbox sync is already running.".into());
            }
            s.last_attempt = Some(Utc::now().to_rfc3339());
            self.persist(&s)?;
            s.syncing = true;
            (
                s.config.clone().ok_or("Mailbox is not configured")?,
                s.cursor.clone(),
                s.generation,
            )
        };
        let result = if config.provider == "imap" || config.provider == "proton-bridge" {
            self.sync_imap(&config, &cursor, generation)
        } else {
            self.access_token(&config, generation).and_then(|token| {
                if config.provider == "gmail" {
                    self.sync_gmail(&config, &token, &cursor, generation)
                } else {
                    self.sync_graph(&config, &token, &cursor, generation)
                }
            })
        };
        let mut s = self.state()?;
        if let Err(err) = &result {
            s.syncing = false;
            s.last_error = Some(err.clone());
            self.persist(&s)?;
        }
        result
    }
    pub fn acknowledge(&self, batch: &SyncBatch) -> Result<Value, String> {
        let mut s = self.state()?;
        s.syncing = false;
        if s.generation != batch.generation {
            return Err(
                "Mailbox configuration changed during import. The cursor was not advanced.".into(),
            );
        }
        s.cursor = batch.cursor.clone();
        s.last_sync = Some(Utc::now().to_rfc3339());
        s.last_error = None;
        self.persist(&s)?;
        drop(s);
        Ok(
            json!({"importedMessages": batch.messages.len(), "hasMore": batch.has_more, "lastSync":Utc::now().to_rfc3339()}),
        )
    }
    pub fn finish_statistics(
        &self,
        examined: usize,
        candidates: usize,
        drafts: usize,
    ) -> Result<(), String> {
        let mut s = self.state()?;
        s.statistics = json!({"examined":examined,"candidates":candidates,"drafts":drafts});
        self.persist(&s)
    }
    pub fn abort(&self) {
        if let Ok(mut s) = self.state() {
            s.syncing = false;
            s.last_error =
                Some("Import batch was not completed. Retry sync; its cursor was retained.".into());
            let _ = self.persist(&s);
        }
    }
    pub fn identity(&self) -> Result<String, String> {
        let s = self.state()?;
        let c = s.config.as_ref().ok_or("Configure a mailbox first")?;
        Ok(format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}|{}|{}|{}|{}",
                    c.provider, c.host, c.username, c.client_id, c.tenant
                )
                .as_bytes()
            )
        ))
    }
    /// Hosts call at most every few minutes. Failed syncs should back off; paused or
    /// disconnected accounts return false. This method never starts a thread itself.
    pub fn should_auto_sync(&self, minimum_seconds: i64) -> bool {
        self.state()
            .map(|s| {
                s.connected
                    && !s.paused
                    && !s.syncing
                    && s.last_attempt
                        .as_ref()
                        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
                        .is_none_or(|v| {
                            Utc::now().timestamp() - v.timestamp()
                                >= minimum_seconds.max(
                                    s.config
                                        .as_ref()
                                        .map(|c| c.sync_minutes * 60)
                                        .unwrap_or(300),
                                )
                        })
            })
            .unwrap_or(false)
    }
    fn sync_imap(
        &self,
        config: &Config,
        cursor: &Value,
        generation: u64,
    ) -> Result<SyncBatch, String> {
        let credentials = self
            .secrets
            .load()?
            .ok_or("Mailbox credentials are unavailable; reconnect")?;
        let mut session = ImapSession::connect(
            config,
            credentials
                .password
                .as_deref()
                .ok_or("Missing IMAP password")?,
        )?;
        let validity = session.examine(&config.folder)?;
        let last_uid = if cursor["uidValidity"].as_u64() == Some(validity) {
            cursor["lastUid"].as_u64().unwrap_or(0)
        } else {
            0
        };
        let search = format!(
            "UID SEARCH UID {}:*{}",
            last_uid.saturating_add(1),
            config
                .filter
                .after_date
                .as_ref()
                .map(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d")
                    .map(|d| format!(" SINCE {}", d.format("%d-%b-%Y")))
                    .unwrap_or_default())
                .unwrap_or_default()
        );
        let response = session.command(&search)?;
        let mut ids: Vec<u64> = response
            .lines
            .iter()
            .filter_map(|l| l.strip_prefix("* SEARCH "))
            .flat_map(|s| s.split_whitespace())
            .filter_map(|s| s.parse().ok())
            .filter(|id| *id > last_uid)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let has_more = ids.len() > BATCH;
        ids.truncate(BATCH);
        let mut messages = Vec::new();
        let mut next_uid = last_uid;
        let examined = ids.len();
        for uid in ids {
            let fetched = session.command(&format!(
                "UID FETCH {uid} (UID BODY.PEEK[HEADER.FIELDS (FROM DATE SUBJECT MESSAGE-ID)])"
            ))?;
            let raw = fetched
                .literals
                .first()
                .ok_or("The IMAP server did not return the requested message")?;
            let parsed =
                mailparse::parse_mail(raw).map_err(|_| "A mailbox message was not valid MIME")?;
            use mailparse::MailHeaderMap;
            if matches_filter(
                &config.filter,
                &parsed.headers.get_first_value("From").unwrap_or_default(),
                &parsed.headers.get_first_value("Date").unwrap_or_default(),
            ) && matches_subject(
                &config.filter,
                &parsed
                    .headers
                    .get_first_value("Subject")
                    .unwrap_or_default(),
            ) {
                let full = session.command(&format!("UID FETCH {uid} (UID BODY.PEEK[])"))?;
                let raw = full
                    .literals
                    .first()
                    .ok_or("The IMAP server did not return the requested message")?;
                let text = String::from_utf8_lossy(raw).into_owned();
                messages.push(EmailMessage {
                    id: format!(
                        "imap:{:x}:{}:{}:{}",
                        Sha256::digest(format!("{}\n{}", config.host, config.username).as_bytes()),
                        config.folder,
                        validity,
                        uid
                    ),
                    provider: config.provider.clone(),
                    source_url: None,
                    text,
                    format: "eml".into(),
                    received_at: parsed.headers.get_first_value("Date"),
                });
            }
            next_uid = uid;
        }
        let _ = session.command("LOGOUT");
        Ok(SyncBatch {
            messages,
            has_more,
            examined,
            cursor: json!({"uidValidity":validity,"lastUid":next_uid}),
            generation,
        })
    }
    fn get_json(&self, url: &str, token: &str) -> Result<Value, String> {
        validate_api_url(url)?;
        for attempt in 0..3 {
            let response = http_client()?.get(url).bearer_auth(token).send().map_err(|_| "Mailbox service could not be reached; sync will retry without advancing its cursor")?;
            if (response.status().as_u16() == 429 || response.status().is_server_error())
                && attempt < 2
            {
                let seconds = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1 << attempt)
                    .min(30);
                thread::sleep(Duration::from_secs(seconds));
                continue;
            }
            return read_json_response(response);
        }
        Err("Mailbox request retries exhausted".into())
    }
    fn sync_gmail(
        &self,
        config: &Config,
        token: &str,
        cursor: &Value,
        generation: u64,
    ) -> Result<SyncBatch, String> {
        let query = gmail_query(&config.filter);
        // A rolling list cursor is used for the first pass. A Gmail history cursor
        // follows it, so arrivals during the initial scan cannot be missed.
        let base = "https://gmail.googleapis.com/gmail/v1/users/me";
        let bootstrap = cursor["historyId"].as_str().is_none();
        let mut history = cursor["bootstrapHistoryId"].as_str().map(str::to_owned);
        if bootstrap && history.is_none() {
            history = self.get_json(&format!("{base}/profile"), token)?["historyId"]
                .as_str()
                .map(str::to_owned);
        }
        let mut url = Url::parse(&format!(
            "{base}/{}",
            if bootstrap { "messages" } else { "history" }
        ))
        .map_err(|_| "Invalid provider endpoint")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("maxResults", &BATCH.to_string());
            if bootstrap {
                q.append_pair("q", &query);
                if let Some(page) = cursor["pageToken"].as_str() {
                    q.append_pair("pageToken", page);
                }
            } else {
                q.append_pair(
                    "startHistoryId",
                    cursor["historyId"].as_str().ok_or("Invalid Gmail cursor")?,
                );
                q.append_pair("historyTypes", "messageAdded");
                if let Some(page) = cursor["pageToken"].as_str() {
                    q.append_pair("pageToken", page);
                }
            }
        }
        let result = match self.get_json(url.as_str(), token) {
            Ok(v) => v,
            Err(e) if !bootstrap && e.contains("HTTP 404") => {
                return self.sync_gmail(config, token, &Value::Null, generation)
            }
            Err(e) => return Err(e),
        };
        let ids: BTreeSet<String> = if bootstrap {
            result["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["id"].as_str().map(str::to_owned))
                .collect()
        } else {
            result["history"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|h| h["messagesAdded"].as_array().into_iter().flatten())
                .filter_map(|m| m["message"]["id"].as_str().map(str::to_owned))
                .collect()
        };
        if ids.len() > 500 {
            return Err("Gmail returned an oversized history batch; narrow the mailbox filter or reconnect for a fresh scan.".into());
        }
        let mut messages = Vec::new();
        let examined = ids.len();
        for id in ids {
            if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err("Invalid Gmail message identity".into());
            }
            // History cannot use q: re-evaluate the configured Gmail query against
            // this message's RFC Message-ID before fetching its complete body.
            if !bootstrap {
                let meta = self.get_json(
                    &format!("{base}/messages/{id}?format=metadata&metadataHeaders=Message-ID"),
                    token,
                )?;
                let mid = meta["payload"]["headers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|h| {
                        h["name"]
                            .as_str()
                            .is_some_and(|v| v.eq_ignore_ascii_case("Message-ID"))
                    })
                    .and_then(|h| h["value"].as_str());
                if let Some(mid) = mid {
                    let mut check = Url::parse(&format!("{base}/messages")).unwrap();
                    check.query_pairs_mut().append_pair(
                        "q",
                        &format!("({query}) rfc822msgid:{}", mid.trim_matches(['<', '>'])),
                    );
                    let found = self.get_json(check.as_str(), token)?;
                    if !found["messages"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|m| m["id"] == id)
                    {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            let headers = self.get_json(&format!("{base}/messages/{id}?format=metadata&metadataHeaders=From&metadataHeaders=Date&metadataHeaders=Subject"),token)?;
            let header = |name: &str| {
                headers["payload"]["headers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|h| {
                        h["name"]
                            .as_str()
                            .is_some_and(|v| v.eq_ignore_ascii_case(name))
                    })
                    .and_then(|h| h["value"].as_str())
                    .unwrap_or("")
            };
            if !matches_filter(&config.filter, header("From"), header("Date"))
                || !matches_subject(&config.filter, header("Subject"))
            {
                continue;
            }
            let data = self.get_json(&format!("{base}/messages/{id}?format=raw"), token)?;
            let raw = URL_SAFE_NO_PAD
                .decode(
                    data["raw"]
                        .as_str()
                        .ok_or("Gmail did not return MIME content")?,
                )
                .map_err(|_| "Invalid Gmail message encoding")?;
            if raw.len() > MAX_MESSAGE {
                return Err("A mailbox message exceeds the 12 MiB import limit; narrow the retrieval filter.".into());
            }
            messages.push(EmailMessage {
                id: id.clone(),
                provider: "gmail".into(),
                source_url: Some(format!("https://mail.google.com/mail/u/0/#all/{id}")),
                text: String::from_utf8_lossy(&raw).into_owned(),
                format: "eml".into(),
                received_at: data["internalDate"]
                    .as_str()
                    .and_then(|v| v.parse::<i64>().ok())
                    .and_then(DateTime::from_timestamp_millis)
                    .map(|v| v.to_rfc3339()),
            });
        }
        let next = result["nextPageToken"].as_str();
        let next_cursor = if bootstrap && next.is_some() {
            json!({"bootstrapHistoryId":history,"pageToken":next})
        } else if bootstrap {
            json!({"historyId":history.ok_or("Gmail did not return a history cursor")?})
        } else if next.is_some() {
            json!({"historyId":cursor["historyId"],"pageToken":next})
        } else {
            json!({"historyId":result["historyId"].as_str().ok_or("Gmail did not return a history cursor")?})
        };
        Ok(SyncBatch {
            messages,
            examined,
            has_more: next.is_some(),
            cursor: next_cursor,
            generation,
        })
    }
    fn sync_graph(
        &self,
        config: &Config,
        token: &str,
        cursor: &Value,
        generation: u64,
    ) -> Result<SyncBatch, String> {
        let url = if let Some(next) = cursor["url"].as_str() {
            next.to_owned()
        } else {
            let folder = config.filter.label_or_folder.as_deref().unwrap_or("inbox");
            if !folder
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '='))
            {
                return Err("Use a Microsoft Graph folder ID or well-known folder name.".into());
            }
            let mut url = Url::parse(&format!(
                "https://graph.microsoft.com/v1.0/me/mailFolders/{folder}/messages/delta"
            ))
            .map_err(|_| "Invalid Graph endpoint")?;
            url.query_pairs_mut()
                .append_pair(
                    "$select",
                    "id,subject,from,receivedDateTime,webLink,internetMessageId",
                )
                .append_pair("$top", &BATCH.to_string());
            if let Some(date) = &config.filter.after_date {
                url.query_pairs_mut()
                    .append_pair("$filter", &format!("receivedDateTime ge {date}T00:00:00Z"));
            }
            url.to_string()
        };
        validate_graph_url(&url)?;
        let value = self.get_json(&url, token)?;
        let mut messages = Vec::new();
        let mut seen = HashSet::new();
        for message in value["value"]
            .as_array()
            .ok_or("Microsoft Graph did not return messages")?
        {
            if message.get("@removed").is_some() {
                continue;
            }
            let id = message["id"]
                .as_str()
                .ok_or("Microsoft Graph returned a message without an ID")?;
            if !seen.insert(id.to_owned()) {
                continue;
            }
            let sender = message["from"]["emailAddress"]["address"]
                .as_str()
                .unwrap_or("");
            let date = message["receivedDateTime"].as_str().unwrap_or("");
            if !matches_filter(&config.filter, sender, date)
                || !matches_subject(&config.filter, message["subject"].as_str().unwrap_or(""))
            {
                continue;
            }
            // Fetch only selected messages, as bounded MIME (including supported attachments).
            let mut endpoint = Url::parse("https://graph.microsoft.com/v1.0/me/messages/").unwrap();
            endpoint
                .path_segments_mut()
                .map_err(|_| "Invalid message endpoint")?
                .push(id)
                .push("$value");
            validate_graph_url(endpoint.as_str())?;
            let response = http_client()?
                .get(endpoint)
                .bearer_auth(token)
                .send()
                .map_err(|_| "Could not retrieve selected Outlook message")?;
            if !response.status().is_success() {
                return Err(format!(
                    "Mailbox service HTTP {}",
                    response.status().as_u16()
                ));
            }
            let mut bytes = Vec::new();
            response
                .take((MAX_MESSAGE + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| "Could not read selected message")?;
            if bytes.len() > MAX_MESSAGE {
                return Err("A mailbox message exceeds the 12 MiB import limit.".into());
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let link = message["webLink"]
                .as_str()
                .filter(|s| Url::parse(s).is_ok_and(|u| u.scheme() == "https"))
                .map(str::to_owned);
            messages.push(EmailMessage {
                id: id.into(),
                provider: "outlook".into(),
                source_url: link,
                text,
                format: "eml".into(),
                received_at: Some(date.into()),
            });
        }
        let next = value["@odata.nextLink"].as_str();
        let link = next
            .or(value["@odata.deltaLink"].as_str())
            .ok_or("Microsoft Graph did not return a continuation cursor")?;
        validate_graph_url(link)?;
        Ok(SyncBatch {
            messages,
            examined: value["value"].as_array().map(Vec::len).unwrap_or(0),
            has_more: next.is_some(),
            cursor: json!({"url":link}),
            generation,
        })
    }
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Birthright-Atlas/0.1")
        .build()
        .map_err(|_| "Could not initialize HTTPS client".into())
}

fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key].as_str().ok_or_else(|| format!("Missing {key}"))
}
fn random_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn authorization_url(
    c: &Config,
    redirect: &str,
    verifier: &str,
    state: &str,
) -> Result<String, String> {
    let mut url = Url::parse(&if c.provider == "gmail" {
        "https://accounts.google.com/o/oauth2/v2/auth".into()
    } else {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
            c.tenant
        )
    })
    .map_err(|_| "Invalid authorization endpoint")?;
    url.query_pairs_mut()
        .append_pair("client_id", &c.client_id)
        .append_pair("redirect_uri", redirect)
        .append_pair("response_type", "code")
        .append_pair("state", state)
        .append_pair(
            "code_challenge",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        )
        .append_pair("code_challenge_method", "S256")
        .append_pair(
            "scope",
            if c.provider == "gmail" {
                "https://www.googleapis.com/auth/gmail.readonly"
            } else {
                "openid offline_access https://graph.microsoft.com/Mail.Read"
            },
        );
    if c.provider == "gmail" {
        url.query_pairs_mut()
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
    }
    Ok(url.into())
}
fn token_endpoint(c: &Config) -> String {
    if c.provider == "gmail" {
        "https://oauth2.googleapis.com/token".into()
    } else {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            c.tenant
        )
    }
}
fn parse_callback(target: &str, expected_state: &str) -> Result<String, String> {
    let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| "Invalid callback")?;
    if url.path() != "/oauth/callback" {
        return Err("Unexpected callback path".into());
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.iter().filter(|(k, _)| k == "state").count() != 1
        || pairs
            .iter()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.as_ref())
            != Some(expected_state)
    {
        return Err("OAuth state mismatch".into());
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        return Err("Mailbox authorization was declined".into());
    }
    let code = pairs
        .iter()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .filter(|v| !v.is_empty())
        .ok_or("Missing authorization code")?;
    if code.len() > 4096 {
        return Err("Authorization code too large".into());
    }
    Ok(code)
}
fn apply_token(c: &mut Credentials, value: &Value) -> Result<(), String> {
    c.access_token = Some(
        value["access_token"]
            .as_str()
            .ok_or("Provider did not return an access token")?
            .into(),
    );
    if let Some(refresh) = value["refresh_token"].as_str() {
        c.refresh_token = Some(refresh.into());
    }
    c.expires_at = Utc::now().timestamp()
        + value["expires_in"]
            .as_i64()
            .unwrap_or(3600)
            .clamp(30, 86400);
    Ok(())
}
fn read_json_response(mut response: reqwest::blocking::Response) -> Result<Value, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Mailbox provider returned HTTP {}. Reconnect after 401/403; retry after rate limits. No cursor was advanced.",status.as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE as u64)
    {
        return Err("Mailbox provider response exceeded the size limit".into());
    }
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take((MAX_RESPONSE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read mailbox response")?;
    if bytes.len() > MAX_RESPONSE {
        return Err("Mailbox provider response exceeded the size limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Mailbox provider returned invalid JSON".into())
}
fn validate_api_url(value: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|_| "Invalid mailbox API URL")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || !["gmail.googleapis.com", "graph.microsoft.com"].contains(&url.host_str().unwrap_or(""))
    {
        return Err("Refusing a mailbox cursor outside the provider's HTTPS API".into());
    }
    Ok(())
}
fn validate_graph_url(value: &str) -> Result<(), String> {
    validate_api_url(value)?;
    let url = Url::parse(value).map_err(|_| "Invalid Microsoft Graph cursor URL")?;
    if url.host_str() != Some("graph.microsoft.com") {
        return Err("Refusing to send a Microsoft token to a different provider".into());
    }
    Ok(())
}
fn gmail_query(filter: &Filter) -> String {
    let mut query = filter.query.clone().unwrap_or_else(|| DEFAULT_QUERY.into());
    if !filter.senders.is_empty() || !filter.domains.is_empty() {
        query = format!(
            "({query}) ({})",
            filter
                .senders
                .iter()
                .chain(filter.domains.iter())
                .map(|s| format!("from:{}", s.replace(['"', '(', ')', ' '], "")))
                .collect::<Vec<_>>()
                .join(" OR ")
        );
    }
    for (prefix, values) in [
        ("subject", &filter.subject_keywords),
        ("subject", &filter.reservation_keywords),
    ] {
        if !values.is_empty() {
            query.push_str(&format!(
                " ({})",
                values
                    .iter()
                    .map(|v| format!("{prefix}:\"{}\"", v.replace(['"', '\\', '\r', '\n'], "")))
                    .collect::<Vec<_>>()
                    .join(" OR ")
            ));
        }
    }
    for sender in &filter.excluded_senders {
        query.push_str(&format!(" -from:\"{}\"", sender.replace(['"', '\\'], "")));
    }
    if let Some(date) = &filter.before_date {
        query.push_str(&format!(" before:{}", date.replace('-', "/")));
    }
    if let Some(date) = &filter.after_date {
        query.push_str(&format!(" after:{}", date.replace('-', "/")));
    }
    if let Some(label) = &filter.label_or_folder {
        query.push_str(&format!(
            " label:\"{}\"",
            label.replace(['"', '\r', '\n'], "")
        ));
    }
    query
}
fn matches_filter(filter: &Filter, from: &str, date: &str) -> bool {
    let addresses: Vec<String> = mailparse::addrparse(from)
        .ok()
        .map(|list| {
            list.iter()
                .flat_map(|a| match a {
                    mailparse::MailAddr::Single(s) => vec![s.addr.to_lowercase()],
                    mailparse::MailAddr::Group(g) => {
                        g.addrs.iter().map(|s| s.addr.to_lowercase()).collect()
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    if addresses.iter().any(|a| {
        filter
            .excluded_senders
            .iter()
            .any(|s| a.eq_ignore_ascii_case(s.trim()))
    }) {
        return false;
    }
    if !filter.senders.is_empty() || !filter.domains.is_empty() {
        let allowed = addresses.iter().any(|a| {
            filter
                .senders
                .iter()
                .any(|s| a.eq_ignore_ascii_case(s.trim()))
                || filter.domains.iter().any(|d| {
                    a.rsplit_once('@').is_some_and(|(_, domain)| {
                        domain.eq_ignore_ascii_case(d.trim().trim_start_matches('@'))
                    })
                })
        });
        if !allowed {
            return false;
        }
    }
    if filter.after_date.is_some() || filter.before_date.is_some() {
        let Some(parsed) = DateTime::parse_from_rfc3339(date)
            .or_else(|_| DateTime::parse_from_rfc2822(date))
            .ok()
        else {
            return false;
        };
        let day = parsed.date_naive().to_string();
        if filter.after_date.as_ref().is_some_and(|v| &day < v)
            || filter.before_date.as_ref().is_some_and(|v| &day >= v)
        {
            return false;
        }
    }
    true
}
fn matches_subject(filter: &Filter, subject: &str) -> bool {
    let subject = subject.to_lowercase();
    [&filter.subject_keywords, &filter.reservation_keywords]
        .iter()
        .all(|words| {
            words.is_empty()
                || words
                    .iter()
                    .any(|word| subject.contains(&word.to_lowercase()))
        })
}
fn quote_imap(value: &str) -> Result<String, String> {
    if value.contains(['\r', '\n', '\0']) || value.len() > 16384 {
        return Err("Invalid IMAP text field".into());
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}
struct ImapResponse {
    lines: Vec<String>,
    literals: Vec<Vec<u8>>,
}
struct ImapSession {
    reader: BufReader<native_tls::TlsStream<TcpStream>>,
    tag: u32,
}
impl ImapSession {
    fn connect(config: &Config, password: &str) -> Result<Self, String> {
        let address = (config.host.as_str(), config.port)
            .to_socket_addrs()
            .map_err(|_| "Could not resolve IMAP hostname")?
            .next()
            .ok_or("IMAP host has no address")?;
        if config.provider == "proton-bridge" && !address.ip().is_loopback() {
            return Err("Proton Bridge must resolve to this computer's loopback interface.".into());
        }
        let tcp=TcpStream::connect_timeout(&address,Duration::from_secs(10)).map_err(|_|"Could not connect to IMAP. For Proton, start the paid-plan Bridge and check its settings.")?;
        tcp.set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|_| "Could not configure IMAP timeout")?;
        tcp.set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|_| "Could not configure IMAP timeout")?;
        let mut tls = native_tls::TlsConnector::builder();
        tls.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
        if let Some(pem) = &config.certificate_pem {
            tls.add_root_certificate(
                native_tls::Certificate::from_pem(pem.as_bytes())
                    .map_err(|_| "Invalid custom trust certificate")?,
            );
        }
        let connector = tls
            .build()
            .map_err(|_| "Could not initialize trusted TLS")?;
        let tls_stream = if config.tls_mode == "starttls" {
            let mut raw = BufReader::new(tcp);
            let greeting = read_imap_line(&mut raw)?;
            if !greeting.starts_with("* OK") {
                return Err("IMAP server did not provide an OK greeting".into());
            }
            raw.get_mut()
                .write_all(b"A0 STARTTLS\r\n")
                .map_err(|_| "Could not request IMAP STARTTLS")?;
            let response = read_imap_response(&mut raw, "A0")?;
            if response.lines.iter().any(|l| l.starts_with("A0 OK")) && raw.buffer().is_empty() {
                connector.connect(&config.host,raw.into_inner()).map_err(|_|"IMAP TLS verification failed. Import the provider/Bridge trust certificate and use its matching hostname; verification cannot be disabled.")?
            } else {
                return Err("The IMAP server did not accept STARTTLS safely.".into());
            }
        } else {
            connector.connect(&config.host,tcp).map_err(|_|"IMAP TLS verification failed. Check the server hostname and trusted certificate; verification cannot be disabled.")?
        };
        let mut session = Self {
            reader: BufReader::new(tls_stream),
            tag: 1,
        };
        if config.tls_mode != "starttls" {
            let greeting = read_imap_line(&mut session.reader)?;
            if !greeting.starts_with("* OK") {
                return Err("IMAP server did not provide an OK greeting".into());
            }
        }
        session.command(&format!(
            "LOGIN {} {}",
            quote_imap(&config.username)?,
            quote_imap(password)?
        ))?;
        Ok(session)
    }
    fn command(&mut self, command: &str) -> Result<ImapResponse, String> {
        let tag = format!("A{}", self.tag);
        self.tag = self.tag.saturating_add(1);
        self.reader
            .get_mut()
            .write_all(format!("{tag} {command}\r\n").as_bytes())
            .map_err(|_| "Could not send read-only IMAP command")?;
        read_imap_response(&mut self.reader, &tag)
    }
    fn examine(&mut self, folder: &str) -> Result<u64, String> {
        let response = self.command(&format!("EXAMINE {}", quote_imap(folder)?))?;
        response
            .lines
            .iter()
            .find_map(|line| {
                line.split("[UIDVALIDITY ")
                    .nth(1)
                    .and_then(|v| v.split(']').next())
                    .and_then(|v| v.parse().ok())
            })
            .ok_or_else(|| {
                "IMAP server did not supply UIDVALIDITY; refusing an unsafe incremental cursor"
                    .into()
            })
    }
}
fn read_imap_line<R: BufRead>(reader: &mut R) -> Result<String, String> {
    let mut data = Vec::new();
    reader
        .take(65537)
        .read_until(b'\n', &mut data)
        .map_err(|_| "IMAP response read failed")?;
    if data.is_empty() {
        return Err("IMAP connection closed unexpectedly".into());
    }
    if data.len() > 65536 || !data.ends_with(b"\n") {
        return Err("IMAP response line exceeded the limit".into());
    }
    String::from_utf8(data).map_err(|_| "Invalid IMAP response line".into())
}
fn read_imap_response<R: BufRead>(reader: &mut R, tag: &str) -> Result<ImapResponse, String> {
    let mut response = ImapResponse {
        lines: Vec::new(),
        literals: Vec::new(),
    };
    let mut size = 0usize;
    loop {
        let line = read_imap_line(reader)?;
        size += line.len();
        if size > MAX_RESPONSE {
            return Err("IMAP response exceeds the size limit".into());
        }
        if line.starts_with(&format!("{tag} ")) {
            if !line.starts_with(&format!("{tag} OK")) {
                return Err("IMAP rejected a read-only command. Check credentials, folder, and provider policy.".into());
            }
            response.lines.push(line);
            return Ok(response);
        }
        if let Some(part) = line
            .trim_end()
            .strip_suffix('}')
            .and_then(|v| v.rsplit('{').next())
        {
            let length: usize = part.parse().map_err(|_| "Invalid IMAP literal length")?;
            if length > MAX_MESSAGE || size.saturating_add(length) > MAX_RESPONSE {
                return Err("A mailbox message exceeds the 12 MiB import limit; narrow the retrieval filter.".into());
            }
            let mut literal = vec![0; length];
            reader
                .read_exact(&mut literal)
                .map_err(|_| "Incomplete IMAP message body")?;
            size += length;
            response.literals.push(literal);
        }
        response.lines.push(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    pub(super) struct MemorySecrets(Mutex<Option<String>>);
    impl SecretStore for MemorySecrets {
        fn load(&self) -> Result<Option<Credentials>, String> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .as_ref()
                .map(|v| serde_json::from_str(v).unwrap()))
        }
        fn save(&self, c: &Credentials) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(serde_json::to_string(c).unwrap());
            Ok(())
        }
        fn delete(&self) -> Result<(), String> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }
    fn config() -> Config {
        serde_json::from_value(json!({"provider":"gmail","clientId":"synthetic-client"})).unwrap()
    }
    #[test]
    fn selective_headers_fail_closed_and_exclusions_win() {
        let f = Filter {
            domains: vec!["example.invalid".into()],
            excluded_senders: vec!["ads@example.invalid".into()],
            subject_keywords: vec!["reservation".into()],
            after_date: Some("2028-01-01".into()),
            before_date: Some("2029-01-01".into()),
            ..Default::default()
        };
        assert!(matches_filter(
            &f,
            "Bookings <travel@example.invalid>",
            "2028-04-01T12:00:00Z"
        ));
        assert!(!matches_filter(
            &f,
            "travel@example.invalid <adversary@evil.invalid>",
            "2028-04-01T12:00:00Z"
        ));
        assert!(!matches_filter(
            &f,
            "ads@example.invalid",
            "2028-04-01T12:00:00Z"
        ));
        assert!(!matches_filter(&f, "travel@example.invalid", "malformed"));
        assert!(!matches_filter(
            &f,
            "travel@example.invalid",
            "2029-01-01T00:00:00Z"
        ));
        assert!(matches_subject(&f, "Reservation updated"));
        assert!(!matches_subject(&f, "Weekly newsletter"));
        assert!(gmail_query(&f).contains("before:2029/01/01"));
    }
    #[test]
    fn failed_batches_keep_cursor_and_back_off_from_attempt_time() {
        let dir = tempfile::tempdir().unwrap();
        let service = fixture_service(dir.path());
        {
            let mut s = service.state().unwrap();
            s.connected = true;
            s.last_attempt = Some(Utc::now().to_rfc3339());
            s.syncing = true;
        }
        assert!(!service.should_auto_sync(300));
        assert!(service.sync().is_err());
        assert!(service.configure(&json!({"provider":"imap","host":"synthetic.invalid","username":"bookings","filter":{"senders":["one@example.invalid"]}})).is_err());
        service.abort();
        assert!(!service.should_auto_sync(300));
        assert!(service.state().unwrap().cursor.is_null());
        let batch = fixture_batch(&service, "synthetic");
        assert!(service.state().unwrap().syncing);
        service.acknowledge(&batch).unwrap();
        assert!(!service.state().unwrap().syncing);
        let reopened =
            EmailService::with_store(dir.path(), Arc::new(MemorySecrets::default())).unwrap();
        assert_eq!(reopened.state().unwrap().cursor["lastUid"], 1);
    }
    #[test]
    fn oauth_has_pkce_state_and_readonly_scope() {
        let url = Url::parse(
            &authorization_url(
                &config(),
                "http://127.0.0.1:12345/oauth/callback",
                "verifier",
                "nonce",
            )
            .unwrap(),
        )
        .unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "nonce");
        assert!(q["scope"].contains("gmail.readonly"));
        assert!(!q["scope"].contains("modify"));
        assert_ne!(q["code_challenge"], "verifier");
    }
    #[test]
    fn callback_rejects_wrong_state_and_duplicate_state() {
        assert!(parse_callback("/oauth/callback?code=a&state=other", "expected").is_err());
        assert!(parse_callback("/oauth/callback?code=a&state=x&state=x", "x").is_err());
        assert_eq!(
            parse_callback("/oauth/callback?code=abc&state=expected", "expected").unwrap(),
            "abc"
        );
        assert_eq!(
            parse_callback(
                "/oauth/callback?error=access_denied&state=expected",
                "expected"
            )
            .unwrap_err(),
            "Mailbox authorization was declined"
        );
        assert_eq!(
            parse_callback(
                "/oauth/callback?error=access_denied&state=wrong",
                "expected"
            )
            .unwrap_err(),
            "OAuth state mismatch"
        );
    }
    #[test]
    fn configuration_never_persists_password() {
        let dir = tempfile::tempdir().unwrap();
        let service =
            EmailService::with_store(dir.path(), Arc::new(MemorySecrets::default())).unwrap();
        let status=service.configure(&json!({"provider":"proton-bridge","host":"127.0.0.1","username":"synthetic@example.invalid","password":"synthetic-test-only","tlsMode":"starttls","port":1143})).unwrap();
        let text = fs::read_to_string(dir.path().join("email.json")).unwrap();
        assert!(!text.contains("synthetic-test-only"));
        assert!(!status.to_string().contains("synthetic-test-only"));
        assert_eq!(status["connected"], false);
    }
    #[test]
    fn proton_rejects_remote_host_and_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let service =
            EmailService::with_store(dir.path(), Arc::new(MemorySecrets::default())).unwrap();
        assert!(service
            .configure(
                &json!({"provider":"proton-bridge","host":"mail.example.invalid","username":"x"})
            )
            .is_err());
        assert!(service.configure(&json!({"provider":"imap","host":"mail.example.invalid","username":"x","tlsMode":"none"})).is_err());
    }
    #[test]
    fn cursor_commits_only_after_acknowledgement() {
        let dir = tempfile::tempdir().unwrap();
        let service =
            EmailService::with_store(dir.path(), Arc::new(MemorySecrets::default())).unwrap();
        let batch = SyncBatch {
            messages: vec![],
            has_more: false,
            examined: 0,
            cursor: json!({"uidValidity":3,"lastUid":10}),
            generation: 0,
        };
        assert!(service.state().unwrap().cursor.is_null());
        service.acknowledge(&batch).unwrap();
        assert_eq!(service.state().unwrap().cursor["lastUid"], 10);
        let mut s = service.state().unwrap();
        s.generation = 1;
        drop(s);
        assert!(service.acknowledge(&batch).is_err());
    }
    #[test]
    fn imap_literal_preserves_body_and_ignores_body_tags() {
        let input = b"* 1 FETCH (UID 4 BODY[] {12}\r\nA1 OK body!!\r\n)\r\nA1 OK Done\r\n";
        let response = read_imap_response(&mut BufReader::new(&input[..]), "A1").unwrap();
        assert_eq!(response.literals[0], b"A1 OK body!!");
    }
    #[test]
    fn imap_excessive_literals_rejected() {
        let input = b"* 1 FETCH (BODY[] {999999999}\r\n";
        assert!(read_imap_response(&mut BufReader::new(&input[..]), "A1").is_err());
    }
    #[test]
    fn sender_filter_checks_exact_mailbox_not_display_name() {
        let filter = Filter {
            senders: vec!["booking@example.invalid".into()],
            ..Default::default()
        };
        assert!(matches_filter(
            &filter,
            "Bookings <booking@example.invalid>",
            ""
        ));
        assert!(!matches_filter(
            &filter,
            "booking@example.invalid <attacker@example.invalid>",
            ""
        ));
    }
    #[test]
    fn malicious_delta_url_is_rejected() {
        for url in [
            "http://graph.microsoft.com/x",
            "https://graph.microsoft.com.attacker.invalid/x",
            "https://user@graph.microsoft.com/x",
            "https://127.0.0.1/x",
            "https://graph.microsoft.com:444/x",
        ] {
            assert!(validate_api_url(url).is_err());
        }
        assert!(validate_api_url(
            "https://graph.microsoft.com/v1.0/me/messages/delta?$deltatoken=abc"
        )
        .is_ok());
        assert!(
            validate_graph_url("https://gmail.googleapis.com/gmail/v1/users/me/profile").is_err()
        );
        assert!(validate_graph_url(
            "https://graph.microsoft.com/v1.0/me/messages/delta?$deltatoken=abc"
        )
        .is_ok());
    }
    #[test]
    fn imap_escaping_disallows_command_injection() {
        assert!(quote_imap("user\r\nDELETE INBOX").is_err());
        assert_eq!(quote_imap("a\"b\\c").unwrap(), "\"a\\\"b\\\\c\"");
    }
    #[test]
    fn refresh_preserves_original_refresh_token() {
        let mut c = Credentials {
            refresh_token: Some("synthetic-refresh".into()),
            ..Default::default()
        };
        apply_token(
            &mut c,
            &json!({"access_token":"synthetic-access","expires_in":3600}),
        )
        .unwrap();
        assert_eq!(c.refresh_token.as_deref(), Some("synthetic-refresh"));
    }

    #[test]
    fn changing_retrieval_filter_preserves_account_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemorySecrets::default());
        let service = EmailService::with_store(dir.path(), store.clone()).unwrap();
        service.configure(&json!({"provider":"imap","host":"imap.example.invalid","username":"synthetic@example.invalid","password":"synthetic-test-only"})).unwrap();
        service.state().unwrap().connected = true;
        service.configure(&json!({"provider":"imap","host":"imap.example.invalid","username":"synthetic@example.invalid","password":"","filter":{"senders":["booking@example.invalid"]}})).unwrap();
        assert_eq!(
            store.load().unwrap().unwrap().password.as_deref(),
            Some("synthetic-test-only")
        );
        assert_eq!(service.status().unwrap()["connected"], true);
        assert!(service.state().unwrap().cursor.is_null());
    }

    #[test]
    fn changing_account_clears_the_previous_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemorySecrets::default());
        let service = EmailService::with_store(dir.path(), store.clone()).unwrap();
        service.configure(&json!({"provider":"imap","host":"imap.example.invalid","username":"one@example.invalid","password":"synthetic-test-only"})).unwrap();
        service.configure(&json!({"provider":"imap","host":"imap.example.invalid","username":"two@example.invalid"})).unwrap();
        assert!(store.load().unwrap().is_none());
        assert_eq!(service.status().unwrap()["connected"], false);
    }

    #[test]
    fn configuration_fails_closed_when_secret_storage_is_unavailable() {
        struct Locked;
        impl SecretStore for Locked {
            fn load(&self) -> Result<Option<Credentials>, String> {
                Err("locked".into())
            }
            fn save(&self, _: &Credentials) -> Result<(), String> {
                Err("locked".into())
            }
            fn delete(&self) -> Result<(), String> {
                Err("locked".into())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let service = EmailService::with_store(dir.path(), Arc::new(Locked)).unwrap();
        assert!(service.configure(&json!({"provider":"imap","host":"imap.example.invalid","username":"synthetic@example.invalid","password":"synthetic-test-only"})).is_err());
        assert!(!dir.path().join("email.json").exists());
        assert_eq!(service.status().unwrap()["secureStorageAvailable"], false);
    }
    // Public, synthetic certificate generated for parser/configuration tests.
    // Its throwaway private key is not retained or distributed.
    const TEST_TRUST_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDGjCCAgKgAwIBAgIUQA6G7Zb0gWkHSTGNTgTwGl5kh/wwDQYJKoZIhvcNAQEL
BQAwFDESMBAGA1UEAwwJMTI3LjAuMC4xMB4XDTI2MTAyMjIwNTU0MVoXDTM2MTAx
OTIwNTU0MVowFDESMBAGA1UEAwwJMTI3LjAuMC4xMIIBIjANBgkqhkiG9w0BAQEF
AAOCAQ8AMIIBCgKCAQEA37SSD722tlogydfFgM1gshFAPk7hB0cZhVDWHRRMeqki
q05XD+FUIFQFMpKK+f++zTL0FDni4sdy6NVEtgWJm9IOT0IblaXZjK/tfFFXrYKO
xJFmqg7kD+BGt8xFhbZBK63a+YXUSOBt0nlZHuPOwYmaYOl4TXVr7ByDp9AOP6hR
4FLS49cNLEGum2IE4kBoA2iwkAePWObj4mMSgd6HAsqoaiYeeVwMU6+YQLtg5YUn
EURV05GNx2jI24l9f01fft1Zpe05wJOEj4T2KCCOdBotzxqJDUXczq4obeuQXbQx
WGBujfJpNRKU584+bUPO4B3i5dR0zB7uK9KtvzHD8wIDAQABo2QwYjAdBgNVHQ4E
FgQUOEpcixWAa5/6/7vyxPw5vr3kvxQwHwYDVR0jBBgwFoAUOEpcixWAa5/6/7vy
xPw5vr3kvxQwDwYDVR0TAQH/BAUwAwEB/zAPBgNVHREECDAGhwR/AAABMA0GCSqG
SIb3DQEBCwUAA4IBAQA/jEw8uTACdg918qnZnDZAL4MQ6wm5jg6/joKfiV67pYCw
Ydqf1Yvibpg+coRBld/gSnIOkEbJktL3Ywr5H3TwCjeJuNvnhiZSbXN8BpvPnj1P
lrqvz8iPc7bsclDB9E8y6EcLeBPg1P2T4aU7my7gkCeIHlDijm5YT9xkJjcMeanK
QONHeZiP9P1IKIU7fb/VkO7XibQq9l+m6uUg/m/ofOOLZIVvFWKuCVz7JMG/OmSP
n0zCYFIFQ+tZzlYioJtRbS+R3PFXohqwgFwecuOfKrdjl2tPA29nnoguDW4FFn2M
6Cl1UTRonP/jMT0b73ZkzgJUTFRid5IWcYpJzwBa
-----END CERTIFICATE-----"#;

    #[test]
    fn omitted_trust_certificate_survives_filter_edits_but_null_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let service =
            EmailService::with_store(dir.path(), Arc::new(MemorySecrets::default())).unwrap();
        service.configure(&json!({"provider":"proton-bridge","host":"127.0.0.1","username":"synthetic@example.invalid","certificatePem":TEST_TRUST_CERT})).unwrap();
        let updated = service.configure(&json!({"provider":"proton-bridge","host":"127.0.0.1","username":"synthetic@example.invalid","filter":{"senders":["booking@example.invalid"]}})).unwrap();
        assert_eq!(updated["hasCustomTrustCertificate"], true);
        assert_eq!(
            service
                .state()
                .unwrap()
                .config
                .as_ref()
                .unwrap()
                .certificate_pem
                .as_deref(),
            Some(TEST_TRUST_CERT)
        );
        let reloaded =
            EmailService::with_store(dir.path(), Arc::new(MemorySecrets::default())).unwrap();
        assert_eq!(
            reloaded
                .state()
                .unwrap()
                .config
                .as_ref()
                .unwrap()
                .certificate_pem
                .as_deref(),
            Some(TEST_TRUST_CERT)
        );
        let cleared = service.configure(&json!({"provider":"proton-bridge","host":"127.0.0.1","username":"synthetic@example.invalid","certificatePem":null})).unwrap();
        assert_eq!(cleared["hasCustomTrustCertificate"], false);
        assert!(service
            .state()
            .unwrap()
            .config
            .as_ref()
            .unwrap()
            .certificate_pem
            .is_none());
    }

    #[test]
    fn changing_host_or_provider_never_inherits_the_previous_trust_certificate() {
        for next in [
            json!({"provider":"proton-bridge","host":"localhost","username":"synthetic@example.invalid"}),
            json!({"provider":"imap","host":"127.0.0.1","username":"synthetic@example.invalid"}),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let service =
                EmailService::with_store(dir.path(), Arc::new(MemorySecrets::default())).unwrap();
            service.configure(&json!({"provider":"proton-bridge","host":"127.0.0.1","username":"synthetic@example.invalid","certificatePem":TEST_TRUST_CERT})).unwrap();
            assert_eq!(
                service.configure(&next).unwrap()["hasCustomTrustCertificate"],
                false
            );
            assert!(service
                .state()
                .unwrap()
                .config
                .as_ref()
                .unwrap()
                .certificate_pem
                .is_none());
        }
    }
}

#[cfg(test)]
pub(crate) fn fixture_service(path: &Path) -> EmailService {
    let service =
        EmailService::with_store(path, Arc::new(tests::MemorySecrets::default())).unwrap();
    service.configure(&json!({"provider":"imap","host":"synthetic.invalid","username":"bookings","password":"synthetic-only"})).unwrap();
    service
}
#[cfg(test)]
pub(crate) fn fixture_batch(service: &EmailService, text: &str) -> SyncBatch {
    let generation = service.state().unwrap().generation;
    service.state().unwrap().syncing = true;
    SyncBatch {
        messages: vec![EmailMessage {
            id: "synthetic-message-1".into(),
            provider: "imap".into(),
            source_url: None,
            text: text.into(),
            format: "text".into(),
            received_at: Some("2028-01-01T00:00:00Z".into()),
        }],
        has_more: false,
        examined: 1,
        cursor: json!({"lastUid":1}),
        generation,
    }
}
