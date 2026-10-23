//! Shared, offline-first application services. All transports call this boundary.
pub mod ai;
pub mod email;
pub mod imports;
pub mod journey;
pub mod money;
mod panels;
pub mod planning;
pub mod recurrence;
pub mod releases;
pub mod schedule;

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 4;
const MAX_FILE: usize = 20 * 1024 * 1024;
const KINDS: &[&str] = &[
    "trip",
    "stay",
    "transport",
    "objective",
    "workBlock",
    "payment",
    "expense",
    "reminder",
    "place",
    "document",
    "quote",
    "watchlist",
    "budget",
    "source",
    "draft",
    "connection",
];

pub struct Atlas {
    db: Connection,
    data_dir: PathBuf,
    email: email::EmailService,
}

fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn string<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn migrate(db: &Connection) -> Result<(), String> {
    db.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY, kind TEXT NOT NULL, version INTEGER NOT NULL, body TEXT NOT NULL, updated_at TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS records_kind ON records(kind);
            CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS history(sequence INTEGER PRIMARY KEY AUTOINCREMENT, record_id TEXT NOT NULL, action TEXT NOT NULL, body TEXT NOT NULL, changed_at TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS history_record ON history(record_id);
            CREATE TABLE IF NOT EXISTS import_keys(source_key TEXT PRIMARY KEY, draft_id TEXT NOT NULL);
            CREATE UNIQUE INDEX IF NOT EXISTS reservation_identity ON records(kind,lower(coalesce(json_extract(body,'$.provider'),'')),lower(json_extract(body,'$.reservationId')))
                WHERE kind IN ('stay','transport') AND json_type(body,'$.reservationId')='text' AND length(json_extract(body,'$.reservationId'))>0;
            ").map_err(error)?;
    db.execute_batch(panels::MIGRATION).map_err(error)?;
    db.execute_batch("PRAGMA user_version=4; COMMIT;")
        .map_err(error)?;
    Ok(())
}

impl Atlas {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir).map_err(error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o700)).map_err(error)?;
        }
        for dir in ["documents", "backups"] {
            fs::create_dir_all(data_dir.join(dir)).map_err(error)?;
        }
        let db = Connection::open(data_dir.join("atlas.sqlite3")).map_err(error)?;
        db.busy_timeout(std::time::Duration::from_secs(10))
            .map_err(error)?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
            .map_err(error)?;
        let version: i64 = db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(error)?;
        if version > SCHEMA_VERSION {
            return Err("This database needs a newer Atlas version. Restore an appropriate backup or update the app.".into());
        }
        if version > 0 && version < SCHEMA_VERSION {
            db.backup(
                rusqlite::DatabaseName::Main,
                data_dir
                    .join("backups")
                    .join(format!("pre-migration-v{version}-{}.sqlite3", id())),
                None,
            )
            .map_err(error)?;
        }
        migrate(&db)?;
        let email = email::EmailService::new(&data_dir)?;
        Ok(Self {
            db,
            data_dir,
            email,
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn call(&mut self, method: &str, payload: Value) -> Result<Value, String> {
        if payload.to_string().len() > 30 * 1024 * 1024 {
            return Err("Request exceeds the local service size limit.".into());
        }
        match method {
            "snapshot" => Ok(json!({"records": self.records(None)?, "settings": self.settings()?})),
            "records.list" => Ok(json!(
                self.records(payload.get("kind").and_then(Value::as_str))?
            )),
            "records.save" => self.save_record(
                payload.get("record").cloned().ok_or("Missing record")?,
                payload.get("expectedVersion").and_then(Value::as_i64),
            ),
            "records.delete" => self.delete_record(
                string(&payload, "id"),
                payload.get("expectedVersion").and_then(Value::as_i64),
            ),
            "records.history" => self.history(string(&payload, "id")),
            "settings.get" => self.settings(),
            "releases.history" => self.release_history(&payload),
            "settings.save" => self.save_settings(payload.get("settings").unwrap_or(&payload)),
            "budget.summary" => money::summary(&self.records(None)?, &self.settings()?, &payload),
            "budget.period" => {
                money::period_summary(&self.records(None)?, &self.settings()?, &payload)
            }
            "work.window" => schedule::window(&self.records(None)?, &self.settings()?, &payload),
            "calendar.occurrences" => Ok(json!(recurrence::expand(
                &self.records(None)?,
                string(&payload, "start"),
                string(&payload, "end")
            )?)),
            "fx.convert" => self.convert_money(&payload),
            "discovery.rank" => planning::rank(&self.records(None)?, &self.settings()?, &payload),
            "planning.what_if" => {
                planning::what_if(&self.records(None)?, &self.settings()?, &payload)
            }
            "planning.origin" => {
                planning::preceding_location(&self.records(None)?, &self.settings()?, &payload)
            }
            "imports.parse" => self.parse_import(
                string(&payload, "text"),
                payload
                    .get("format")
                    .and_then(Value::as_str)
                    .unwrap_or("text"),
                None,
            ),
            "imports.file" => self.import_file(&payload),
            "imports.accept" => self.accept_import(&payload),
            "imports.dismiss" => {
                let mut draft = self.get_required(string(&payload, "id"))?;
                if string(&draft, "kind") != "draft" {
                    return Err("Expected an import draft".into());
                }
                draft["status"] = json!("dismissed");
                self.save_record(draft, None)
            }
            "documents.attach" => {
                let mut input = payload.clone();
                input["attachOnly"] = json!(true);
                self.import_file(&input)
            }
            "planning.instant" => Ok(
                json!({"instant": schedule::resolve_local(string(&payload,"date"),string(&payload,"time"),string(&payload,"timezone"))?.to_rfc3339()}),
            ),
            "documents.read" => self.read_document(string(&payload, "id")),
            "calendar.export" => Ok(
                json!({"filename":"Birthright-Atlas.ics", "content":imports::export_ics(&self.records(None)?)?}),
            ),
            "reminders.due" => self.due_reminders(payload.get("now").and_then(Value::as_str)),
            "reminders.snooze" | "reminders.dismiss" => self.update_reminder(method, &payload),
            "backup.export" => self.export_backup(),
            "backup.list" => self.list_backups(),
            "backup.restore" => self.restore_backup(string(&payload, "filename")),
            "assistant.ask" => {
                self.contextual_answer(string(&payload, "text"), &payload["context"])
            }
            "chat.list" | "chat.create" | "chat.get" | "chat.send" | "chat.delete"
            | "chat.rename" | "chat.undo" => self.chat(method, &payload),
            "notifications.list"
            | "notifications.read"
            | "notifications.read_all"
            | "notifications.dismiss" => self.notifications(method, &payload),
            "planning.gaps" => journey::gaps(&self.records(None)?, &payload),
            "planning.preview" => {
                journey::preview(&self.records(None)?, &self.settings()?, &payload)
            }
            "planning.workspace" => Ok(journey::workspace(
                &self.records(None)?,
                &self.get_required(string(&payload, "id"))?,
            )),
            "email.status" => self.email.status(),
            "email.configure" => self.email.configure(&payload),
            "email.connect" => self.email.connect(),
            "email.pause" => self.email.pause(
                payload
                    .get("paused")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            ),
            "email.disconnect" => self.email.disconnect(),
            "email.sync" => self.sync_email(),
            "email.auto_sync" => {
                if self.email.should_auto_sync(300) {
                    self.sync_email()
                } else {
                    Ok(json!({"skipped":true}))
                }
            }
            "ai.status" => Ok(ai::status(&self.settings()?)),
            "ai.extract" => ai::extract(string(&payload, "text"), &self.settings()?),
            "imports.ai" => {
                let text = string(&payload, "text");
                let draft = ai::extract(text, &self.settings()?)?;
                self.persist_drafts(
                    vec![draft],
                    text.as_bytes(),
                    Some(&json!({"method":"local_ai","reviewRequired":true})),
                )
            }
            "ai.cancel" => Ok(ai::cancel()),
            _ => Err(format!("Unknown application service: {method}")),
        }
    }

    fn records(&self, kind: Option<&str>) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT body FROM records WHERE (?1 IS NULL OR kind=?1) ORDER BY updated_at,id",
            )
            .map_err(error)?;
        let raw = stmt
            .query_map([kind], |r| r.get::<_, String>(0))
            .map_err(error)?;
        raw.map(|r| serde_json::from_str(&r.map_err(error)?).map_err(error))
            .collect()
    }

    fn get(&self, record_id: &str) -> Result<Option<Value>, String> {
        let value: Option<String> = self
            .db
            .query_row("SELECT body FROM records WHERE id=?1", [record_id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(error)?;
        value
            .map(|v| serde_json::from_str(&v).map_err(error))
            .transpose()
    }
    fn get_required(&self, record_id: &str) -> Result<Value, String> {
        self.get(record_id)?
            .ok_or_else(|| "Record not found".into())
    }

    fn save_record(&mut self, mut record: Value, expected: Option<i64>) -> Result<Value, String> {
        if !record.is_object() {
            return Err("Record must be an object".into());
        }
        if record.to_string().len() > 2 * 1024 * 1024 {
            return Err("Record is too large".into());
        }
        if string(&record, "id").is_empty() {
            record["id"] = json!(id());
        }
        let record_id = string(&record, "id").to_owned();
        if Uuid::parse_str(&record_id).is_err() {
            return Err("Record ID must be a UUID".into());
        }
        let existing = self.get(&record_id)?;
        let current_version = existing
            .as_ref()
            .and_then(|v| v["version"].as_i64())
            .unwrap_or(0);
        let expected = expected.or_else(|| record["version"].as_i64());
        if expected.is_some_and(|e| e != current_version) {
            return Err(format!("Conflict: record changed (current version {current_version}). Refresh and review your edit."));
        }
        if current_version > 0 && expected.is_none() {
            return Err("An existing record needs expectedVersion to prevent overwriting concurrent changes.".into());
        }
        if let Some(old) = &existing {
            if old["kind"] != record["kind"] {
                return Err("Record kind cannot change".into());
            }
        }
        self.validate_record(&mut record)?;
        record["version"] = json!(current_version + 1);
        record["createdAt"] = existing
            .as_ref()
            .map(|v| v["createdAt"].clone())
            .unwrap_or_else(|| json!(now()));
        record["updatedAt"] = json!(now());
        let tx = self.db.transaction().map_err(error)?;
        let changed=tx.execute("INSERT INTO records(id,kind,version,body,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,version=excluded.version,body=excluded.body,updated_at=excluded.updated_at WHERE records.version=?6",
            params![record_id,string(&record,"kind"),current_version+1,record.to_string(),string(&record,"updatedAt"),current_version])
            .map_err(|e|if e.to_string().contains("reservation_identity"){"A booking with this provider and reservation ID already exists. Review or update that booking.".to_owned()}else{error(e)})?;
        if changed != 1 {
            return Err(
                "Conflict: another client changed this record. Refresh before editing.".into(),
            );
        }
        tx.execute(
            "INSERT INTO history(record_id,action,body,changed_at) VALUES(?1,?2,?3,?4)",
            params![
                record_id,
                if existing.is_some() {
                    "update"
                } else {
                    "create"
                },
                record.to_string(),
                now()
            ],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        if string(&record, "kind") == "stay" && existing.is_some() {
            // Suggestions depend on dates and context; confirmed records are never moved.
            for mut draft in self.records(Some("draft"))? {
                if string(&draft, "stayId") == record_id && string(&draft, "status") == "pending" {
                    draft["needsReview"] = json!(true);
                    self.save_record(draft, None)?;
                }
            }
        }
        Ok(record)
    }

    fn validate_record(&self, r: &mut Value) -> Result<(), String> {
        let kind = string(r, "kind").to_owned();
        if !KINDS.contains(&kind.as_str()) {
            return Err("Unsupported record kind".into());
        }
        if [
            "stay",
            "transport",
            "objective",
            "workBlock",
            "place",
            "reminder",
            "expense",
            "quote",
            "watchlist",
            "trip",
        ]
        .contains(&kind.as_str())
            && string(r, "name").trim().is_empty()
        {
            return Err("A name is required".into());
        }
        for key in [
            "totalMinor",
            "amountMinor",
            "incomeMinor",
            "reserveMinor",
            "allowanceMinor",
            "openingBalanceMinor",
        ] {
            if let Some(v) = r.get(key) {
                let n = v
                    .as_i64()
                    .ok_or_else(|| format!("{key} must be integer minor units"))?;
                if n.unsigned_abs() > 1_000_000_000_000 {
                    return Err(format!("{key} exceeds the supported range"));
                }
                if n < 0 && !["amountMinor", "openingBalanceMinor"].contains(&key) {
                    return Err(format!("{key} cannot be negative"));
                }
            }
        }
        if let Some(currency) = r.get("currency") {
            let c = currency
                .as_str()
                .ok_or("Currency must be a three-letter code")?;
            if c.len() != 3 || !c.bytes().all(|b| b.is_ascii_uppercase()) {
                return Err("Use an uppercase three-letter currency code".into());
            }
        }
        for field in ["url", "hostUrl", "rulesUrl", "sourceUrl"] {
            if let Some(v) = r
                .get(field)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                let u = url::Url::parse(v).map_err(|_| format!("{field} is not a valid URL"))?;
                if !["http", "https"].contains(&u.scheme())
                    || !u.username().is_empty()
                    || u.password().is_some()
                {
                    return Err(format!(
                        "{field} must be an HTTP(S) link without embedded credentials"
                    ));
                }
            }
        }
        for key in ["timezone", "departureTimezone", "arrivalTimezone"] {
            if let Some(tz) = r.get(key).and_then(Value::as_str) {
                tz.parse::<chrono_tz::Tz>()
                    .map_err(|_| format!("Unknown IANA timezone: {key}"))?;
            }
        }
        for (key, maximum) in [
            ("workspaceCommuteMinutes", 240),
            ("minimumTransferMinutes", 1440),
        ] {
            if let Some(value) = r
                .get(key)
                .filter(|v| !v.is_null() && v.as_str() != Some(""))
            {
                if value.as_i64().is_none_or(|v| !(0..=maximum).contains(&v)) {
                    return Err(format!(
                        "{key} must be a whole number between 0 and {maximum}"
                    ));
                }
            }
        }
        if let Some(hours) = r.get("workspaceHours").filter(|v| !v.is_null()) {
            for key in ["start", "end"] {
                chrono::NaiveTime::parse_from_str(string(hours, key), "%H:%M")
                    .map_err(|_| "Workspace hours require both opening and closing times")?;
            }
            if let Some(days) = hours.get("weekdays") {
                if days.as_array().is_none_or(|d| {
                    d.iter()
                        .any(|n| n.as_i64().is_none_or(|v| !(1..=7).contains(&v)))
                }) {
                    return Err("Workspace weekdays must be 1 (Monday) through 7 (Sunday)".into());
                }
            }
        }
        for key in ["date", "checkIn", "checkOut"] {
            if let Some(date) = r.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                parse_date(date)?;
            }
        }
        for key in ["dueAt", "departure", "arrival", "cancellationAt"] {
            if let Some(t) = r.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                DateTime::parse_from_rfc3339(t)
                    .map_err(|_| format!("{key} needs a timezone-aware ISO date/time"))?;
            }
        }
        for (key, expected_kind) in [
            ("stayId", "stay"),
            ("tripId", "trip"),
            ("placeId", "place"),
            ("objectiveId", "objective"),
            ("paymentId", "payment"),
        ] {
            if let Some(value) = r.get(key).and_then(Value::as_str).filter(|v| !v.is_empty()) {
                let linked = self.get_required(value)?;
                if string(&linked, "kind") != expected_kind {
                    return Err(format!("{key} must reference a {expected_kind}"));
                }
            }
        }
        match kind.as_str() {
            "stay" => {
                let a = parse_date(string(r, "checkIn"))?;
                let b = parse_date(string(r, "checkOut"))?;
                if b <= a {
                    return Err(
                        "Checkout must follow check-in; checkout is exclusive for occupied nights."
                            .into(),
                    );
                }
                if (b - a).num_days() > 3660 {
                    return Err("Stay exceeds ten years".into());
                }
                require_currency_amount(r, "totalMinor")?;
                if string(r, "timezone").is_empty() {
                    return Err("Choose the stay's IANA timezone".into());
                }
                default(r, "status", json!("tentative"));
                default(r, "context", json!("on_foot"));
                default(r, "checkInTime", json!("15:00"));
                default(r, "checkOutTime", json!("11:00"));
                for key in ["checkInTime", "checkOutTime"] {
                    chrono::NaiveTime::parse_from_str(string(r, key), "%H:%M")
                        .map_err(|_| format!("{key} must be HH:MM"))?;
                }
                for field in [
                    "refrigerator",
                    "workspace",
                    "internet",
                    "kitchen",
                    "quiet",
                    "power",
                    "private",
                    "scenic",
                    "utilitiesIncluded",
                ] {
                    if let Some(v) = r.get("facilities").and_then(|f| f.get(field)) {
                        if !v.is_boolean() && !v.is_null() {
                            return Err("Facilities must be true, false, or unknown (null)".into());
                        }
                    }
                }
            }
            "transport" => {
                let departure =
                    DateTime::parse_from_rfc3339(string(r, "departure")).map_err(error)?;
                let arrival = DateTime::parse_from_rfc3339(string(r, "arrival")).map_err(error)?;
                if arrival <= departure {
                    return Err("Arrival must follow departure as actual instants, including date-line crossings.".into());
                }
                for key in ["departureTimezone", "arrivalTimezone"] {
                    if string(r, key).is_empty() {
                        return Err(format!("{key} is required"));
                    }
                }
                default(r, "status", json!("tentative"));
                default(r, "context", json!("on_foot"));
                require_currency_amount(r, "totalMinor")?;
            }
            "payment" => {
                require_currency_amount(r, "amountMinor")?;
                parse_date(string(r, "date"))?;
                let booking = self.get_required(string(r, "bookingId"))?;
                if !["stay", "transport", "objective"].contains(&string(&booking, "kind")) {
                    return Err("Payment must link to a booking or objective".into());
                }
                default(r, "status", json!("settled"));
                default(r, "type", json!("payment"));
                if !["planned", "settled"].contains(&string(r, "status")) {
                    return Err("Payment status must be planned or settled".into());
                }
                if !["payment", "deposit", "refund", "deposit_refund"].contains(&string(r, "type"))
                {
                    return Err("Unsupported payment type".into());
                }
                let amount = r["amountMinor"].as_i64().unwrap_or(0);
                if r.get("bookingCurrency").is_some() || r.get("bookingAmountMinor").is_some() {
                    if string(r, "bookingCurrency") != string(&booking, "currency") {
                        return Err(
                            "Historic booking currency must match the linked booking".into()
                        );
                    }
                    let basis = r["bookingAmountMinor"]
                        .as_i64()
                        .ok_or("Historic booking equivalent must be integer minor units")?;
                    if basis.unsigned_abs() > 1_000_000_000_000
                        || (amount != 0 && basis.signum() != amount.signum())
                    {
                        return Err("Historic booking equivalent must have the same sign and a supported amount".into());
                    }
                    parse_date(string(r, "rateDate"))?;
                    if string(r, "rateSource").is_empty() {
                        return Err(
                            "Record the source of the historic settlement conversion".into()
                        );
                    }
                }
                if ["refund", "deposit_refund"].contains(&string(r, "type")) && amount >= 0 {
                    return Err("Refunds use negative amounts against the original booking".into());
                }
                if ["payment", "deposit"].contains(&string(r, "type")) && amount < 0 {
                    return Err("Payments and deposits use positive amounts".into());
                }
            }
            "expense" => {
                require_currency_amount(r, "amountMinor")?;
                parse_date(string(r, "date"))?;
                if r["amountMinor"].as_i64().unwrap_or(0) < 0 {
                    return Err("Use a linked refund payment for booking refunds".into());
                }
            }
            "budget" => {
                parse_date(&format!("{}-01", string(r, "month")))?;
                if string(r, "currency").is_empty() {
                    return Err("Budget currency is required".into());
                }
                for key in [
                    "incomeMinor",
                    "openingBalanceMinor",
                    "reserveMinor",
                    "allowanceMinor",
                ] {
                    default(r, key, json!(0));
                }
                for old in self.records(Some("budget"))? {
                    if old["month"] == r["month"]
                        && old["currency"] == r["currency"]
                        && old["id"] != r["id"]
                    {
                        return Err(
                            "A budget already exists for that month and currency. Edit it instead."
                                .into(),
                        );
                    }
                }
                if let Some(expenses) = r.get("recurringExpenses") {
                    let expenses = expenses
                        .as_array()
                        .ok_or("Recurring expenses must be a list")?;
                    if expenses.iter().any(|e| {
                        e["amountMinor"]
                            .as_i64()
                            .is_none_or(|n| !(0..=1_000_000_000_000).contains(&n))
                    }) {
                        return Err(
                            "Recurring expenses require nonnegative integer minor units".into()
                        );
                    }
                }
            }
            "reminder" => {
                DateTime::parse_from_rfc3339(string(r, "dueAt"))
                    .map_err(|_| "Reminder needs a timezone-aware due instant")?;
                default(r, "state", json!("pending"));
                default(r, "catchUp", json!("deliver"));
                if !["pending", "delivered", "dismissed", "snoozed"].contains(&string(r, "state")) {
                    return Err("Unsupported reminder state".into());
                }
                if !["deliver", "skip"].contains(&string(r, "catchUp")) {
                    return Err("Catch-up must be deliver or skip".into());
                }
                if !string(r, "bookingId").is_empty() {
                    self.get_required(string(r, "bookingId"))?;
                }
            }
            "objective" | "workBlock" => {
                if kind == "objective" {
                    default(r, "status", json!("planned"));
                    if !["planned", "done", "cancelled"].contains(&string(r, "status")) {
                        return Err("Objective status must be planned, done, or cancelled".into());
                    }
                }
                for key in ["startTime", "endTime"] {
                    if !string(r, key).is_empty() {
                        chrono::NaiveTime::parse_from_str(string(r, key), "%H:%M")
                            .map_err(|_| format!("{key} must be HH:MM"))?;
                    }
                }
                if !string(r, "date").is_empty() && !string(r, "startTime").is_empty() {
                    let tz = string(r, "timezone");
                    schedule::resolve_local(string(r, "date"), string(r, "startTime"), tz)?;
                    if !string(r, "endTime").is_empty() {
                        let end_day = if string(r, "endTime") <= string(r, "startTime") {
                            parse_date(string(r, "date"))?
                                .succ_opt()
                                .ok_or("Date overflow")?
                                .to_string()
                        } else {
                            string(r, "date").to_owned()
                        };
                        schedule::resolve_local(&end_day, string(r, "endTime"), tz)?;
                    }
                }
                if !string(r, "startAt").is_empty() || !string(r, "endAt").is_empty() {
                    let a = DateTime::parse_from_rfc3339(string(r, "startAt")).map_err(error)?;
                    let b = DateTime::parse_from_rfc3339(string(r, "endAt")).map_err(error)?;
                    if b <= a {
                        return Err("Scheduled end must follow start".into());
                    }
                }
                if kind == "workBlock"
                    && (!string(r, "start").is_empty() || !string(r, "end").is_empty())
                {
                    let a = DateTime::parse_from_rfc3339(string(r, "start")).map_err(error)?;
                    let b = DateTime::parse_from_rfc3339(string(r, "end")).map_err(error)?;
                    if b <= a {
                        return Err("Work block end must follow start".into());
                    }
                }
                if !string(r, "recurrence").is_empty() && string(r, "recurrence") != "none" {
                    let start = string(r, "date");
                    let end = parse_date(start)?
                        .succ_opt()
                        .ok_or("Date overflow")?
                        .to_string();
                    recurrence::expand(&[r.clone()], start, &end)?;
                }
            }
            "quote" => {
                default(r, "status", json!("tentative"));
                require_currency_amount(r, "totalMinor")?;
                if !string(r, "checkIn").is_empty()
                    && !string(r, "checkOut").is_empty()
                    && parse_date(string(r, "checkOut"))? <= parse_date(string(r, "checkIn"))?
                {
                    return Err("Quote checkout must follow check-in".into());
                }
            }
            "document" => {
                if let Some(path) = r.get("storageName").and_then(Value::as_str) {
                    safe_name(path)?;
                }
            }
            _ => {}
        }
        if ["stay", "transport", "quote"].contains(&kind.as_str()) {
            if !["tentative", "confirmed", "cancelled"].contains(&string(r, "status")) {
                return Err("Booking status must be tentative, confirmed, or cancelled".into());
            }
        }
        if let Some(c) = r.get("context").and_then(Value::as_str) {
            if !["on_foot", "on_road"].contains(&c) {
                return Err("Travel context must be on_foot or on_road".into());
            }
        }
        Ok(())
    }

    fn delete_record(&mut self, record_id: &str, expected: Option<i64>) -> Result<Value, String> {
        let old = self.get_required(record_id)?;
        if expected != old["version"].as_i64() {
            return Err("Conflict: supply the current expectedVersion before deleting.".into());
        }
        for r in self.records(None)? {
            if r["id"] == old["id"] {
                continue;
            }
            if ["bookingId", "stayId", "tripId", "placeId"]
                .iter()
                .any(|k| string(&r, k) == record_id)
            {
                return Err(
                    "This record has linked records. Cancel it or remove those links first.".into(),
                );
            }
        }
        let tx = self.db.transaction().map_err(error)?;
        let deleted = tx
            .execute(
                "DELETE FROM records WHERE id=?1 AND version=?2",
                params![record_id, expected],
            )
            .map_err(error)?;
        if deleted != 1 {
            return Err("Conflict: another client changed this record before deletion.".into());
        }
        tx.execute(
            "INSERT INTO history(record_id,action,body,changed_at) VALUES(?1,'delete',?2,?3)",
            params![record_id, old.to_string(), now()],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(json!({"deleted":true}))
    }

    fn history(&self, record_id: &str) -> Result<Value, String> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT action,body,changed_at FROM history WHERE record_id=?1 ORDER BY sequence",
            )
            .map_err(error)?;
        let rows = stmt
            .query_map([record_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(error)?;
        let mut out = vec![];
        for row in rows {
            let (action, body, date) = row.map_err(error)?;
            out.push(json!({"action":action,"record":serde_json::from_str::<Value>(&body).map_err(error)?,"at":date}));
        }
        Ok(json!(out))
    }

    fn settings(&self) -> Result<Value, String> {
        let saved: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM settings WHERE key='application'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        let mut defaults = json!({"currency":"USD","context":"on_foot","officeTimezone":"America/Los_Angeles","currentTimezone":"UTC","currentLocation":"","workStart":"08:00","workEnd":"17:00","workHours":8,"overlapHours":4,"workdays":[1,2,3,4,5],"preferences":{},"ai":{"enabled":false}});
        if let Some(s) = saved {
            merge(
                &mut defaults,
                &serde_json::from_str::<Value>(&s).map_err(error)?,
            );
        }
        Ok(defaults)
    }
    fn save_settings(&mut self, patch: &Value) -> Result<Value, String> {
        if !patch.is_object() || patch.to_string().len() > 128 * 1024 {
            return Err("Settings must be a bounded object".into());
        }
        let mut settings = self.settings()?;
        merge(&mut settings, patch);
        for field in ["officeTimezone", "currentTimezone"] {
            string(&settings, field)
                .parse::<chrono_tz::Tz>()
                .map_err(|_| format!("Unknown timezone: {field}"))?;
        }
        for field in ["workStart", "workEnd"] {
            chrono::NaiveTime::parse_from_str(string(&settings, field), "%H:%M")
                .map_err(|_| format!("{field} must be HH:MM"))?;
        }
        for field in ["workHours", "overlapHours"] {
            let n = settings[field]
                .as_f64()
                .ok_or("Work hours must be numeric")?;
            if !(0.0..=24.0).contains(&n) {
                return Err("Work hours must be between zero and 24".into());
            }
        }
        if settings["overlapHours"].as_f64() > settings["workHours"].as_f64() {
            return Err("Overlap hours cannot exceed total work hours".into());
        }
        self.db.execute("INSERT INTO settings(key,value) VALUES('application',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[settings.to_string()]).map_err(error)?;
        Ok(settings)
    }

    fn parse_import(
        &mut self,
        text: &str,
        format: &str,
        source_meta: Option<&Value>,
    ) -> Result<Value, String> {
        if text.len() > 2 * 1024 * 1024 {
            return Err("Text import exceeds 2 MiB".into());
        }
        let parsed = imports::parse(text, format)?;
        self.persist_drafts(parsed, text.as_bytes(), source_meta)
    }
    fn persist_drafts(
        &mut self,
        parsed: Vec<Value>,
        source_bytes: &[u8],
        source_meta: Option<&Value>,
    ) -> Result<Value, String> {
        if parsed.is_empty() {
            return Ok(json!([]));
        }
        let source_hash = hash(source_bytes);
        let mut out = vec![];
        for (index, mut parsed) in parsed.into_iter().enumerate() {
            let key = format!("{source_hash}:{index}");
            let prior: Option<String> = self
                .db
                .query_row(
                    "SELECT draft_id FROM import_keys WHERE source_key=?1",
                    [&key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(error)?;
            if let Some(prior) = prior {
                if let Some(draft) = self.get(&prior)? {
                    out.push(draft);
                    continue;
                }
            }
            let mut source = json!({"kind":"source","hash":source_hash,"snippet":parsed.get("sourceText").cloned().unwrap_or(Value::Null),"metadata":source_meta});
            // Keep the extracted evidence, never an entire unrelated mailbox body.
            if source["snippet"].as_str().is_some_and(|s| s.len() > 8192) {
                source["snippet"] = json!(source["snippet"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(8192)
                    .collect::<String>());
            }
            source = self.save_record(source, None)?;
            parsed["kind"] = json!("draft");
            parsed["status"] = json!("pending");
            parsed["sourceId"] = source["id"].clone();
            parsed["sourceHash"] = json!(source_hash);
            parsed["record"]["sourceIds"] = json!([source["id"]]);
            if let Some(existing) = self.find_booking(&parsed["record"])? {
                parsed["existingRecordId"] = existing["id"].clone();
                parsed["expectedBookingVersion"] = existing["version"].clone();
            }
            let saved = self.save_record(parsed, None)?;
            self.db
                .execute(
                    "INSERT INTO import_keys(source_key,draft_id) VALUES(?1,?2)",
                    params![key, string(&saved, "id")],
                )
                .map_err(error)?;
            out.push(saved);
        }
        Ok(json!(out))
    }

    fn find_booking(&self, candidate: &Value) -> Result<Option<Value>, String> {
        let reservation = string(candidate, "reservationId");
        let provider = string(candidate, "provider");
        let mut matches = vec![];
        for record in self.records(None)? {
            if !["stay", "transport", "objective"].contains(&string(&record, "kind")) {
                continue;
            }
            if !reservation.is_empty()
                && string(candidate, "kind") == string(&record, "kind")
                && string(&record, "reservationId").eq_ignore_ascii_case(reservation)
                && string(&record, "provider").eq_ignore_ascii_case(provider)
            {
                matches.push(record);
                continue;
            }
            if reservation.is_empty()
                && string(candidate, "kind") == string(&record, "kind")
                && !string(candidate, "name").is_empty()
                && string(candidate, "name").eq_ignore_ascii_case(string(&record, "name"))
                && candidate.get("checkIn") == record.get("checkIn")
                && candidate.get("departure") == record.get("departure")
                && candidate.get("checkOut") == record.get("checkOut")
            {
                matches.push(record);
            }
        }
        if matches.len() > 1 {
            return Err("Multiple bookings match this confirmation. Review the reservation identity before importing.".into());
        }
        Ok(matches.into_iter().next())
    }

    fn accept_import(&mut self, payload: &Value) -> Result<Value, String> {
        let mut draft = self.get_required(string(payload, "id"))?;
        if string(&draft, "kind") != "draft" {
            return Err("Expected an import draft".into());
        }
        if string(&draft, "status") == "accepted" {
            return self.get_required(string(&draft, "acceptedRecordId"));
        }
        let mut record = payload
            .get("record")
            .cloned()
            .unwrap_or_else(|| draft["record"].clone());
        if !["stay", "transport", "objective", "payment", "place"]
            .contains(&string(&record, "kind"))
        {
            return Err(
                "Review must identify a supported booking, objective, payment, or place".into(),
            );
        }
        let existing = if !string(&draft, "existingRecordId").is_empty() {
            self.get(string(&draft, "existingRecordId"))?
        } else {
            self.find_booking(&record)?
        };
        if let Some(old) = existing {
            let expected = payload["expectedVersion"]
                .as_i64()
                .or_else(|| draft["expectedBookingVersion"].as_i64())
                .unwrap_or_else(|| old["version"].as_i64().unwrap_or(0));
            if old["version"].as_i64() != Some(expected) {
                return Err("Conflict: booking changed since this import draft. Reopen and review the current record.".into());
            }
            let mut combined = old.clone();
            // Missing extracted fields are unknown, not instructions to erase saved facts.
            merge_known(&mut combined, &record);
            record = combined;
            record["id"] = old["id"].clone();
            record["version"] = old["version"].clone();
            let mut sources = old["sourceIds"].as_array().cloned().unwrap_or_default();
            if !sources.contains(&draft["sourceId"]) {
                sources.push(draft["sourceId"].clone());
            }
            record["sourceIds"] = json!(sources);
        } else {
            record
                .as_object_mut()
                .ok_or("Draft record missing")?
                .remove("id");
            record.as_object_mut().unwrap().remove("version");
            record["sourceIds"] = json!([draft["sourceId"]]);
        }
        if string(&draft, "action") == "cancel" || string(&draft, "action") == "cancellation" {
            record["status"] = json!("cancelled");
        }
        let saved = self.save_record(record, None)?;
        draft["status"] = json!("accepted");
        draft["acceptedRecordId"] = saved["id"].clone();
        self.save_record(draft, None)?;
        Ok(saved)
    }

    fn import_file(&mut self, payload: &Value) -> Result<Value, String> {
        let filename = string(payload, "filename");
        safe_name(filename)?;
        let bytes = STANDARD
            .decode(string(payload, "contentBase64"))
            .map_err(|_| "Invalid base64 attachment")?;
        if bytes.len() > MAX_FILE {
            return Err("Attachment exceeds the 20 MiB limit".into());
        }
        let extension = Path::new(filename)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let storage_name = format!(
            "{}{}",
            id(),
            if extension.is_empty() {
                String::new()
            } else {
                format!(".{extension}")
            }
        );
        let parseable = payload["attachOnly"] != true
            && ["txt", "eml", "ics", "pdf"].contains(&extension.as_str());
        let parsed = if parseable {
            Some(imports::parse_file(filename, &bytes)?)
        } else {
            None
        };
        fs::write(self.data_dir.join("documents").join(&storage_name), &bytes).map_err(error)?;
        let doc=self.save_record(json!({"kind":"document","name":filename,"filename":filename,"storageName":storage_name,"mimeType":mime(&extension),"size":bytes.len(),"sha256":hash(&bytes),"companionAvailable":false}),None)?;
        if let Some(mut parsed) = parsed {
            for draft in &mut parsed {
                draft["record"]["documentIds"] = json!([doc["id"]]);
            }
            self.persist_drafts(parsed, &bytes, Some(&json!({"documentId":doc["id"]})))
        } else {
            Ok(json!({"document":doc,"drafts":[]}))
        }
    }
    fn read_document(&self, document_id: &str) -> Result<Value, String> {
        let doc = self.get_required(document_id)?;
        if string(&doc, "kind") != "document" {
            return Err("Expected a document".into());
        }
        let name = string(&doc, "storageName");
        safe_name(name)?;
        let path = self.data_dir.join("documents").join(name);
        let meta = fs::symlink_metadata(&path).map_err(error)?;
        if !meta.file_type().is_file() || meta.len() > MAX_FILE as u64 {
            return Err("Invalid or oversized attachment".into());
        }
        let bytes = fs::read(path).map_err(error)?;
        if hash(&bytes) != string(&doc, "sha256") {
            return Err("Attachment integrity check failed".into());
        }
        Ok(
            json!({"filename":doc["filename"],"mimeType":doc["mimeType"],"contentBase64":STANDARD.encode(bytes)}),
        )
    }

    fn due_reminders(&mut self, at: Option<&str>) -> Result<Value, String> {
        let at = at
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(error)?
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let mut delivered = vec![];
        for mut reminder in self.records(Some("reminder"))? {
            if !["pending", "snoozed"].contains(&string(&reminder, "state")) {
                continue;
            }
            let due = DateTime::parse_from_rfc3339(string(&reminder, "dueAt"))
                .map_err(error)?
                .with_timezone(&Utc);
            if due > at {
                continue;
            }
            if string(&reminder, "catchUp") == "skip" && at - due > Duration::minutes(5) {
                reminder["state"] = json!("dismissed");
                reminder["deliveryNote"] =
                    json!("Missed while host was unavailable; catch-up disabled");
                self.save_record(reminder, None)?;
                continue;
            }
            reminder["state"] = json!("delivered");
            reminder["deliveredAt"] = json!(at.to_rfc3339());
            delivered.push(self.save_record(reminder, None)?);
        }
        Ok(json!(delivered))
    }
    fn update_reminder(&mut self, method: &str, payload: &Value) -> Result<Value, String> {
        let mut r = self.get_required(string(payload, "id"))?;
        if string(&r, "kind") != "reminder" {
            return Err("Expected a reminder".into());
        }
        if method == "reminders.snooze" {
            let mins = payload["minutes"].as_i64().unwrap_or(15);
            if !(1..=43_200).contains(&mins) {
                return Err("Snooze must be between one minute and 30 days".into());
            }
            r["state"] = json!("snoozed");
            r["dueAt"] = json!((Utc::now() + Duration::minutes(mins)).to_rfc3339());
        } else {
            r["state"] = json!("dismissed");
        }
        self.save_record(r, None)
    }

    fn export_backup(&self) -> Result<Value, String> {
        let filename = format!(
            "atlas-{}-{}",
            Utc::now().format("%Y%m%dT%H%M%SZ"),
            &id()[..8]
        );
        let path = self.data_dir.join("backups").join(&filename);
        fs::create_dir(&path).map_err(error)?;
        self.db
            .backup(
                rusqlite::DatabaseName::Main,
                path.join("atlas.sqlite3"),
                None,
            )
            .map_err(error)?;
        let docs = path.join("documents");
        fs::create_dir(&docs).map_err(error)?;
        for doc in self.records(Some("document"))? {
            let name = string(&doc, "storageName");
            safe_name(name)?;
            self.read_document(string(&doc, "id"))?;
            fs::copy(self.data_dir.join("documents").join(name), docs.join(name)).map_err(error)?;
        }
        fs::write(path.join("manifest.json"),serde_json::to_vec_pretty(&json!({"schemaVersion":SCHEMA_VERSION,"appVersion":env!("CARGO_PKG_VERSION"),"createdAt":now(),"databaseSha256":hash(&fs::read(path.join("atlas.sqlite3")).map_err(error)?),"protection":"Plain local backup; protect with OS disk encryption. OAuth/IMAP secrets are not included."})).map_err(error)?).map_err(error)?;
        Ok(
            json!({"path":path,"filename":filename,"includes":"Database, records, settings, history and attachments. Reconnect provider credentials after moving devices."}),
        )
    }
    fn list_backups(&self) -> Result<Value, String> {
        let mut list = vec![];
        for entry in fs::read_dir(self.data_dir.join("backups")).map_err(error)? {
            let e = entry.map_err(error)?;
            if e.file_type().map_err(error)?.is_dir() && e.path().join("manifest.json").is_file() {
                let manifest: Value = serde_json::from_slice(
                    &fs::read(e.path().join("manifest.json")).map_err(error)?,
                )
                .map_err(error)?;
                list.push(json!({"filename":e.file_name().to_string_lossy(),"manifest":manifest}));
            }
        }
        Ok(json!(list))
    }
    fn restore_backup(&mut self, filename: &str) -> Result<Value, String> {
        safe_name(filename)?;
        let path = self.data_dir.join("backups").join(filename);
        if fs::symlink_metadata(&path)
            .map_err(error)?
            .file_type()
            .is_symlink()
        {
            return Err("Backup symlinks are not supported".into());
        }
        let manifest: Value =
            serde_json::from_slice(&fs::read(path.join("manifest.json")).map_err(error)?)
                .map_err(error)?;
        let backup_version = manifest["schemaVersion"]
            .as_i64()
            .ok_or("Backup schema is missing")?;
        if !(1..=SCHEMA_VERSION).contains(&backup_version) {
            return Err(
                "Backup schema differs from this app; use the matching application version.".into(),
            );
        }
        let bytes = fs::read(path.join("atlas.sqlite3")).map_err(error)?;
        if hash(&bytes) != string(&manifest, "databaseSha256") {
            return Err("Backup checksum mismatch".into());
        }
        let backup_db = Connection::open_with_flags(
            path.join("atlas.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(error)?;
        let stored_version: i64 = backup_db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(error)?;
        if stored_version != backup_version {
            return Err("Backup schema metadata does not match its database".into());
        }
        let check: String = backup_db
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(error)?;
        if check != "ok" {
            return Err("Backup database failed its integrity check".into());
        }
        let mut stmt = backup_db
            .prepare("SELECT body FROM records WHERE kind='document'")
            .map_err(error)?;
        let documents = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(error)?
            .map(|r| serde_json::from_str::<Value>(&r.map_err(error)?).map_err(error))
            .collect::<Result<Vec<_>, _>>()?;
        for doc in &documents {
            let name = string(doc, "storageName");
            safe_name(name)?;
            let doc_path = path.join("documents").join(name);
            if !fs::symlink_metadata(&doc_path).map_err(error)?.is_file() {
                return Err("Backup attachment is invalid".into());
            }
            if hash(&fs::read(&doc_path).map_err(error)?) != string(doc, "sha256") {
                return Err("Backup attachment checksum mismatch".into());
            }
        }
        let before = self.export_backup()?;
        // Restore the database via SQLite's backup API while this instance owns its lock.
        self.db
            .restore(
                rusqlite::DatabaseName::Main,
                path.join("atlas.sqlite3"),
                None::<fn(rusqlite::backup::Progress)>,
            )
            .map_err(error)?;
        if backup_version < SCHEMA_VERSION {
            self.db
                .backup(
                    rusqlite::DatabaseName::Main,
                    self.data_dir
                        .join("backups")
                        .join(format!("pre-migration-v{backup_version}-{}.sqlite3", id())),
                    None,
                )
                .map_err(error)?;
            migrate(&self.db)?;
        }
        for doc in documents {
            let name = string(&doc, "storageName");
            fs::copy(
                path.join("documents").join(name),
                self.data_dir.join("documents").join(name),
            )
            .map_err(error)?;
        }
        Ok(json!({"restored":filename,"preRestoreBackup":before,"restartRecommended":true}))
    }

    fn sync_email(&mut self) -> Result<Value, String> {
        let batch = self.email.sync()?;
        let mut drafts = vec![];
        for message in &batch.messages {
            let meta = serde_json::to_value(message).map_err(error)?;
            let text = meta.get("text").and_then(Value::as_str).unwrap_or("");
            let mut provenance = meta.clone();
            provenance
                .as_object_mut()
                .ok_or("Invalid email metadata")?
                .remove("text");
            let format = meta.get("format").and_then(Value::as_str).unwrap_or("text");
            if !imports::is_booking_message(text, format) {
                continue;
            }
            let parsed = self.parse_import(text, format, Some(&provenance))?;
            drafts.extend(parsed.as_array().cloned().unwrap_or_default());
        }
        self.email.acknowledge(&batch)?;
        Ok(
            json!({"drafts":drafts,"messagesProcessed":batch.messages.len(),"hasMore":batch.has_more,"status":self.email.status()?}),
        )
    }

    fn convert_money(&self, payload: &Value) -> Result<Value, String> {
        let from = string(payload, "fromCurrency");
        let to = string(payload, "toCurrency");
        let scale = |c: &str| -> Result<u32, String> {
            if c.len() != 3 || !c.bytes().all(|b| b.is_ascii_uppercase()) {
                return Err("Use uppercase currency codes".into());
            }
            Ok(match c {
                "JPY" | "KRW" | "VND" | "CLP" | "ISK" | "XOF" | "XAF" => 0,
                "BHD" | "KWD" | "OMR" | "JOD" | "TND" => 3,
                _ => 2,
            })
        };
        let amount = payload["amountMinor"]
            .as_i64()
            .ok_or("Amount must be integer minor units")?;
        let numerator = payload["rateNumerator"]
            .as_i64()
            .ok_or("A positive rational exchange rate is required")?;
        let denominator = payload["rateDenominator"]
            .as_i64()
            .ok_or("A positive rational exchange rate is required")?;
        parse_date(string(payload, "rateDate"))?;
        let converted =
            money::convert_minor(amount, scale(from)?, scale(to)?, numerator, denominator)?;
        Ok(
            json!({"amountMinor":amount,"convertedMinor":converted,"fromCurrency":from,"toCurrency":to,"rateNumerator":numerator,"rateDenominator":denominator,"rateDate":payload["rateDate"],"source":payload.get("source").cloned().unwrap_or(json!("manual")),"scenarioOnly":true}),
        )
    }

    fn assistant(&mut self, text: &str) -> Result<Value, String> {
        let q = text.to_lowercase();
        let records = self.records(None)?;
        let settings = self.settings()?;
        if q.trim().starts_with("remind me:") {
            let request = text
                .trim()
                .get(10..)
                .ok_or("Use Remind me: name at an ISO date/time with offset")?
                .trim();
            let (name, due) = request
                .rsplit_once(" at ")
                .ok_or("Use Remind me: name at 2026-12-01T09:00:00+01:00")?;
            DateTime::parse_from_rfc3339(due.trim())
                .map_err(|_| "The reminder needs an exact date/time and timezone offset")?;
            let reminder=self.save_record(json!({"kind":"reminder","name":name.trim(),"dueAt":due.trim(),"state":"pending","catchUp":"deliver"}),None)?;
            return Ok(
                json!({"message":format!("Saved reminder: {} at {}. Delivery requires Atlas running; overdue reminders catch up when it reopens.",name.trim(),due.trim()),"records":[reminder],"undo":{"method":"records.delete","payload":{"id":reminder["id"],"expectedVersion":reminder["version"]}}}),
            );
        }
        if q.contains("remind")
            && (q.contains("show") || q.contains("upcoming") || q.contains("list"))
        {
            let mut reminders: Vec<_> = records
                .iter()
                .filter(|r| {
                    string(r, "kind") == "reminder"
                        && ["pending", "snoozed"].contains(&string(r, "state"))
                })
                .cloned()
                .collect();
            reminders.sort_by(|a, b| string(a, "dueAt").cmp(string(b, "dueAt")));
            return Ok(
                json!({"message":format!("{} saved reminders are pending or snoozed. They run independently of local AI and catch up according to their saved policy.",reminders.len()),"records":reminders}),
            );
        }
        if q.contains("compare") || q.contains("stretch") {
            let comparison = planning::rank(&records, &settings, &json!({}))?;
            let offers: Vec<_> = comparison["ranked"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| r["record"].clone())
                .collect();
            return Ok(
                json!({"message":format!("Compared {} saved offers within their exact dates and currencies. Requirements and workspace evidence come first, followed by full known cost and recorded interests. Stretch options need a reason and stay below your cap. Unknown fees or facilities remain flagged; open Discover → Compare for the evidence.",offers.len()),"records":offers,"comparison":comparison}),
            );
        }
        if q.contains("extend") && (q.contains("week") || q.contains("7")) {
            let stays: Vec<_> = records
                .iter()
                .filter(|r| string(r, "kind") == "stay" && string(r, "status") == "confirmed")
                .collect();
            let selected = stays
                .iter()
                .copied()
                .find(|s| q.contains(&string(s, "name").to_lowercase()))
                .or_else(|| {
                    if stays.len() == 1 {
                        Some(stays[0])
                    } else {
                        None
                    }
                });
            if let Some(stay) = selected {
                let scenario = planning::what_if(
                    &records,
                    &settings,
                    &json!({"stayId":stay["id"],"additionalNights":7}),
                )?;
                return Ok(
                    json!({"message":format!("An extra seven nights at {} is estimated at {}, based on the recorded total per night. A new provider quote is required: one-time fees, discounts, availability and change costs may differ. {} recorded conflicts need review. Your booking and payments are unchanged.",string(stay,"name"),display_money(scenario["scenario"]["estimatedAdditionalMinor"].as_f64().unwrap_or(0.0),string(stay,"currency")),scenario["conflicts"].as_array().map_or(0,Vec::len)),"scenario":scenario,"records":[stay]}),
                );
            }
            return Ok(
                json!({"message":"Include the saved stay name in your request, or open its details and choose the extra-week scenario. A what-if estimate never changes a confirmed booking."}),
            );
        }
        if q.contains("cost") || q.contains("budget") || q.contains("allowance") {
            let month = regex::Regex::new(r"\b[0-9]{4}-[0-9]{2}\b")
                .expect("static month regex")
                .find(text)
                .map(|m| m.as_str().to_owned())
                .unwrap_or_else(|| Utc::now().format("%Y-%m").to_string());
            let summary = money::summary(&records, &settings, &json!({"month":month}))?;
            return Ok(
                json!({"message":format!("For {} across saved commitments, the forecast is {} over {} calendar days: {} per day. Flexible allowance remaining is {} over {} eligible days. Other currencies and tentative alternatives are excluded; no income is assumed.",month,display_money(summary["forecastMinor"].as_f64().unwrap_or(0.0),string(&summary,"currency")),summary["daysInMonth"],display_money(summary["dailyForecastMinor"].as_f64().unwrap_or(0.0),string(&summary,"currency")),display_money(summary["allowanceRemainingMinor"].as_f64().unwrap_or(0.0),string(&summary,"currency")),summary["remainingDays"]),"summary":summary}),
            );
        }
        if q.contains("fridge") || q.contains("workspace") || q.contains("apartment") {
            let matches: Vec<_> = records
                .iter()
                .filter(|r| {
                    ["stay", "quote", "place"].contains(&string(r, "kind"))
                        && r["facilities"]["refrigerator"] == true
                        && r["facilities"]["workspace"] == true
                })
                .cloned()
                .collect();
            return Ok(
                json!({"message":format!("{} saved places have explicitly recorded refrigerator and workspace availability. Unknown facilities do not qualify. Provider prices and availability have not been checked live.",matches.len()),"records":matches}),
            );
        }
        if q.contains("work") || q.contains("grocer") {
            let date = Utc::now().format("%Y-%m-%d").to_string();
            let work = schedule::window(
                &records,
                &settings,
                &json!({"date":date,"timezone":settings["currentTimezone"]}),
            )?;
            return Ok(
                json!({"message":"These are the configured work windows and recorded conflicts. Add a flexible objective to a stay to plan errands around protected work; verify venue hours and connectivity separately.","work":work}),
            );
        }
        if q.contains("remind") {
            return Ok(
                json!({"message":"Use the reminder form, or say “Remind me: checkout at 2026-12-01T09:00:00+01:00” with your exact date and offset. Saved reminders support snooze, dismiss and catch-up. No reminder has been created from this query.","suggestions":[{"action":"createReminder","label":"Create a reminder"}]}),
            );
        }
        let upcoming: Vec<_> = records
            .into_iter()
            .filter(|r| {
                ["stay", "transport", "objective"].contains(&string(r, "kind"))
                    && string(r, "status") != "cancelled"
            })
            .collect();
        Ok(
            json!({"message":"I can query saved costs, flexible allowance, verified fridge/workspace preferences, and work windows. Provider searches open supported websites. Optional local AI is configured separately; no live prices or bookings are implied.","records":upcoming}),
        )
    }
}

fn display_money(minor: f64, currency: &str) -> String {
    let scale = match currency {
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        "CLF" | "UYW" => 4,
        _ => 2,
    };
    format!("{currency} {:.*}", scale, minor / 10_f64.powi(scale as i32))
}

fn default(value: &mut Value, key: &str, fallback: Value) {
    if value.get(key).is_none() || value[key].is_null() {
        value[key] = fallback;
    }
}
fn merge(target: &mut Value, patch: &Value) {
    if let (Some(a), Some(b)) = (target.as_object_mut(), patch.as_object()) {
        for (k, v) in b {
            if v.is_object() && a.get(k).is_some_and(Value::is_object) {
                merge(a.get_mut(k).unwrap(), v);
            } else {
                a.insert(k.clone(), v.clone());
            }
        }
    }
}
fn merge_known(target: &mut Value, patch: &Value) {
    if let (Some(a), Some(b)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in b {
            if value.is_null() {
                continue;
            }
            if value.is_object() && a.get(key).is_some_and(Value::is_object) {
                merge_known(a.get_mut(key).expect("existing object"), value);
            } else {
                a.insert(key.clone(), value.clone());
            }
        }
    }
}
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    if s.len() != 10 {
        return Err("Dates must use YYYY-MM-DD".into());
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| "Invalid calendar date".into())
}
fn require_currency_amount(v: &Value, key: &str) -> Result<(), String> {
    if string(v, "currency").is_empty() || v.get(key).and_then(Value::as_i64).is_none() {
        return Err(format!(
            "Currency and {key} are required; unknown prices must be reviewed."
        ));
    }
    Ok(())
}
fn safe_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 200
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0'])
    {
        Err("Expected a safe filename, without a path".into())
    } else {
        Ok(())
    }
}
fn mime(ext: &str) -> &'static str {
    match ext {
        "pdf" => "application/pdf",
        "ics" => "text/calendar",
        "eml" => "message/rfc822",
        "txt" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stay() -> Value {
        json!({"kind":"stay","name":"Synthetic apartment","timezone":"Europe/Lisbon","checkIn":"2026-12-12","checkOut":"2027-01-11","currency":"USD","totalMinor":90000,"provider":"Example","reservationId":"SYNTH-001","status":"confirmed"})
    }
    #[test]
    fn records_survive_restart_and_conflicts_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Atlas::open(dir.path()).unwrap();
        let first = app.call("records.save", json!({"record":stay()})).unwrap();
        drop(app);
        let mut app = Atlas::open(dir.path()).unwrap();
        let mut updated = first.clone();
        updated["name"] = json!("Changed");
        assert_eq!(
            app.call("records.save", json!({"record":updated})).unwrap()["version"],
            2
        );
        assert!(app
            .call("records.save", json!({"record":first}))
            .unwrap_err()
            .contains("Conflict"));
        assert_eq!(
            app.call("records.list", json!({"kind":"stay"}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn contexts_do_not_filter_or_rewrite_history() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Atlas::open(dir.path()).unwrap();
        app.call("records.save", json!({"record":stay()})).unwrap();
        app.call("settings.save", json!({"settings":{"context":"on_road"}}))
            .unwrap();
        let all = app.call("records.list", json!({"kind":"stay"})).unwrap();
        assert_eq!(all.as_array().unwrap().len(), 1);
        assert_eq!(all[0]["context"], "on_foot");
    }
    #[test]
    fn reminders_are_persistent_and_catch_up_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Atlas::open(dir.path()).unwrap();
        app.call("records.save",json!({"record":{"kind":"reminder","name":"Synthetic checkout","dueAt":"2026-01-01T10:00:00Z"}})).unwrap();
        drop(app);
        let mut app = Atlas::open(dir.path()).unwrap();
        assert_eq!(
            app.call("reminders.due", json!({"now":"2026-01-02T10:00:00Z"}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(app
            .call("reminders.due", json!({"now":"2026-01-02T11:00:00Z"}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn rejects_invalid_money_dates_and_urls() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Atlas::open(dir.path()).unwrap();
        let mut r = stay();
        r["totalMinor"] = json!(1.5);
        assert!(app.call("records.save", json!({"record":r})).is_err());
        let mut r = stay();
        r["checkOut"] = r["checkIn"].clone();
        assert!(app.call("records.save", json!({"record":r})).is_err());
        let mut r = stay();
        r["url"] = json!("javascript:alert(1)");
        assert!(app.call("records.save", json!({"record":r})).is_err());
    }
    #[test]
    fn backup_restore_preserves_records_and_keeps_pre_restore_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Atlas::open(dir.path()).unwrap();
        let first = app.call("records.save", json!({"record":stay()})).unwrap();
        let backup = app.call("backup.export", json!({})).unwrap();
        let mut changed = first;
        changed["name"] = json!("After backup");
        app.call("records.save", json!({"record":changed})).unwrap();
        app.call("backup.restore", json!({"filename":backup["filename"]}))
            .unwrap();
        assert_eq!(
            app.call("records.list", json!({"kind":"stay"})).unwrap()[0]["name"],
            "Synthetic apartment"
        );
        assert!(
            app.call("backup.list", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .len()
                >= 2
        );
        assert!(app
            .call("backup.restore", json!({"filename":"../atlas.sqlite3"}))
            .is_err());
    }
    #[test]
    fn version_one_migration_makes_backup() {
        let dir = tempfile::tempdir().unwrap();
        let app = Atlas::open(dir.path()).unwrap();
        app.db.execute_batch("PRAGMA user_version=1;").unwrap();
        drop(app);
        let _app = Atlas::open(dir.path()).unwrap();
        assert!(fs::read_dir(dir.path().join("backups")).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("pre-migration-v1")));
    }
}
