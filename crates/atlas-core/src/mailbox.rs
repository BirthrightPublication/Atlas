//! Mailbox network and MIME work deliberately runs outside the application data mutex.
use crate::{
    email::{EmailService, SyncBatch},
    error, imports, Atlas,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, sync::Mutex};

pub(crate) fn load_accounts(root: &Path) -> Result<BTreeMap<String, EmailService>, String> {
    let mut accounts = BTreeMap::new();
    // Preserve the v0.3 credential-store key and configuration location.
    accounts.insert("default".into(), EmailService::new(root)?);
    let directory = root.join("email-accounts");
    if directory.exists() {
        for entry in fs::read_dir(directory).map_err(error)? {
            let entry = entry.map_err(error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if uuid::Uuid::parse_str(&name).is_ok() && entry.file_type().map_err(error)?.is_dir() {
                accounts.insert(name, EmailService::new(&entry.path())?);
            }
        }
    }
    Ok(accounts)
}
impl Atlas {
    fn mailbox(&self, payload: &Value) -> Result<(String, EmailService), String> {
        let account = payload["accountId"].as_str().unwrap_or("default");
        Ok((
            account.into(),
            self.emails
                .get(account)
                .cloned()
                .ok_or("Unknown mailbox account")?,
        ))
    }
    pub(crate) fn email_call(&mut self, method: &str, payload: &Value) -> Result<Value, String> {
        if method == "email.accounts" {
            return self
                .emails
                .iter()
                .map(|(id, service)| {
                    let mut status = service.status()?;
                    status["accountId"] = json!(id);
                    Ok(status)
                })
                .collect::<Result<Vec<_>, String>>()
                .map(|v| json!(v));
        }
        if method == "email.add" {
            if self.emails.len() >= 12 {
                return Err("Atlas supports up to 12 local mailbox connections.".into());
            }
            let id = uuid::Uuid::new_v4().to_string();
            let service = EmailService::new(&self.data_dir.join("email-accounts").join(&id))?;
            self.emails.insert(id.clone(), service);
            return Ok(json!({"accountId":id}));
        }
        let (id, service) = self.mailbox(payload)?;
        if method == "email.sync" || method == "email.auto_sync" {
            if method == "email.auto_sync" && !service.should_auto_sync(300) {
                return Ok(json!({"skipped":true}));
            }
            let batch = service.sync()?;
            let result = prepare(&id, &service, &batch)
                .and_then(|prepared| self.store_mail(&prepared, &batch, &service));
            if result.is_err() {
                service.abort();
            }
            return result;
        }
        simple(&service, method, payload)
    }
    fn store_mail(
        &mut self,
        prepared: &[(Vec<Value>, Vec<u8>, Value)],
        batch: &SyncBatch,
        service: &EmailService,
    ) -> Result<Value, String> {
        let mut drafts = vec![];
        let before = self.records(Some("draft"))?.len();
        for (parsed, bytes, metadata) in prepared {
            drafts.extend(
                self.persist_drafts(parsed.clone(), bytes, Some(metadata))?
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        let created = self.records(Some("draft"))?.len().saturating_sub(before);
        service.finish_statistics(batch.examined, prepared.len(), created)?;
        service.acknowledge(batch)?;
        Ok(
            json!({"drafts":drafts,"messagesProcessed":batch.examined,"hasMore":batch.has_more,"status":service.status()?}),
        )
    }
}
fn simple(service: &EmailService, method: &str, payload: &Value) -> Result<Value, String> {
    match method {
        "email.status" => service.status(),
        "email.configure" => service.configure(payload),
        "email.connect" => service.connect(),
        "email.pause" => service.pause(payload["paused"].as_bool().unwrap_or(true)),
        "email.disconnect" => service.disconnect(),
        _ => Err("Unknown mailbox operation".into()),
    }
}
fn prepare(
    account: &str,
    service: &EmailService,
    batch: &SyncBatch,
) -> Result<Vec<(Vec<Value>, Vec<u8>, Value)>, String> {
    let identity = service.identity()?;
    let mut prepared = vec![];
    for message in &batch.messages {
        if !imports::is_booking_message(&message.text, &message.format) {
            continue;
        }
        let mut metadata = serde_json::to_value(message).map_err(error)?;
        metadata
            .as_object_mut()
            .ok_or("Invalid mailbox metadata")?
            .remove("text");
        metadata["accountId"] = json!(account);
        metadata["connectionIdentity"] = json!(identity);
        let parsed = imports::parse(&message.text, &message.format)?;
        prepared.push((parsed, message.text.as_bytes().to_vec(), metadata));
    }
    Ok(prepared)
}
/// Called by native IPC and the development HTTP transport on a blocking worker.
pub fn query(atlas: &Mutex<Atlas>, method: &str, payload: &Value) -> Result<Value, String> {
    if payload.to_string().len() > 128 * 1024 {
        return Err("Mailbox configuration exceeds its size limit".into());
    }
    if method == "email.accounts" || method == "email.add" {
        return atlas
            .lock()
            .map_err(|_| "Application service unavailable")?
            .email_call(method, payload);
    }
    if method == "email.auto_sync" {
        let accounts: Vec<_> = atlas
            .lock()
            .map_err(|_| "Application service unavailable")?
            .emails
            .iter()
            .filter(|(_, s)| s.should_auto_sync(300))
            .map(|(id, _)| id.clone())
            .collect();
        let results: Vec<_> = accounts.iter().map(|id| match query(atlas,"email.sync",&json!({"accountId":id})) { Ok(result)=>json!({"accountId":id,"result":result}),Err(_)=>json!({"accountId":id,"error":"Sync failed; see this account's connection status."}) }).collect();
        return Ok(json!({"accounts":results}));
    }
    let (account, service) = atlas
        .lock()
        .map_err(|_| "Application service unavailable")?
        .mailbox(payload)?;
    if method != "email.sync" {
        return simple(&service, method, payload);
    }
    let batch = service.sync()?;
    let result = prepare(&account, &service, &batch).and_then(|prepared| {
        atlas
            .lock()
            .map_err(|_| "Application service unavailable")?
            .store_mail(&prepared, &batch, &service)
    });
    if result.is_err() {
        service.abort();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONFIRMATION:&str="Booking confirmation\nProvider: Fixture\nReservation ID: MAIL-1\nProperty: Sample\nCheck-in: 2028-12-01\nCheck-out: 2028-12-08\nTimezone: UTC\nTotal: USD 700.00";
    #[test]
    fn replay_and_accounts_have_durable_distinct_message_identities() {
        let dir = tempfile::tempdir().unwrap();
        let mut atlas = Atlas::open(dir.path()).unwrap();
        let mail = tempfile::tempdir().unwrap();
        let service = crate::email::fixture_service(mail.path());
        let batch = crate::email::fixture_batch(&service, CONFIRMATION);
        let parsed = prepare("first-account", &service, &batch).unwrap();
        let first = atlas.store_mail(&parsed, &batch, &service).unwrap();
        let replay = atlas.store_mail(&parsed, &batch, &service).unwrap();
        assert_eq!(first["drafts"][0]["id"], replay["drafts"][0]["id"]);
        assert_eq!(service.status().unwrap()["statistics"]["drafts"], 0);
        let parsed = prepare("second-account", &service, &batch).unwrap();
        let second = atlas.store_mail(&parsed, &batch, &service).unwrap();
        assert_ne!(first["drafts"][0]["id"], second["drafts"][0]["id"]);
        assert_eq!(atlas.records(Some("source")).unwrap().len(), 2);
        drop(atlas);
        let atlas = Atlas::open(dir.path()).unwrap();
        assert_eq!(atlas.records(Some("draft")).unwrap().len(), 2);
    }
    #[test]
    fn malformed_batch_rolls_back_sources_and_never_acknowledges() {
        let dir = tempfile::tempdir().unwrap();
        let mut atlas = Atlas::open(dir.path()).unwrap();
        let mail = tempfile::tempdir().unwrap();
        let service = crate::email::fixture_service(mail.path());
        let batch = crate::email::fixture_batch(&service, CONFIRMATION);
        let mut prepared = prepare("account", &service, &batch).unwrap();
        prepared[0].0[0]["id"] = json!("malformed-not-uuid");
        assert!(atlas.store_mail(&prepared, &batch, &service).is_err());
        service.abort();
        assert!(atlas.records(Some("source")).unwrap().is_empty());
        assert!(service.status().unwrap()["lastSync"].is_null());
        let valid = prepare("account", &service, &batch).unwrap();
        assert_eq!(
            atlas.store_mail(&valid, &batch, &service).unwrap()["drafts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

#[cfg(test)]
mod responsiveness {
    use super::*;
    use std::{
        net::TcpListener,
        sync::{mpsc, Arc},
        thread,
        time::Duration,
    };
    #[test]
    fn a_slow_tls_probe_does_not_hold_the_application_data_lock() {
        let root = tempfile::tempdir().unwrap();
        let mail = tempfile::tempdir().unwrap();
        let service = crate::email::fixture_service(mail.path());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        service.configure(&json!({"provider":"imap","host":"127.0.0.1","port":port,"username":"fixture","password":"synthetic-only","tlsMode":"tls"})).unwrap();
        let (accepted, received) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let server = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            accepted.send(()).unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(5));
            drop(socket);
        });
        let mut atlas = Atlas::open(root.path()).unwrap();
        atlas.emails.insert("default".into(), service);
        let atlas = Arc::new(Mutex::new(atlas));
        let worker = atlas.clone();
        let probe = thread::spawn(move || query(&worker, "email.connect", &json!({})));
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(atlas
            .try_lock()
            .unwrap()
            .call("snapshot", json!({}))
            .is_ok());
        release.send(()).unwrap();
        server.join().unwrap();
        assert!(probe.join().unwrap().is_err());
        assert_eq!(
            query(&atlas, "email.status", &json!({})).unwrap()["connected"],
            false
        );
    }
}
