//! End-to-end application service journeys against real SQLite storage.
//! All addresses, reservations, documents and account identifiers are synthetic.
use atlas_core::Atlas;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

const CONFIRMATION: &str = "Booking confirmation\nProvider: Example Stays\nReservation ID: SYNTHETIC-ATLAS-001\nProperty: Riverside apartment\nAddress: Example Street, Lisbon, Portugal\nCheck-in: 2026-12-12\nCheck-out: 2027-01-11\nTimezone: Europe/Lisbon\nTotal: USD 900.00\nRefrigerator: yes\nWorkspace: yes\nStatus: confirmed\nThis is synthetic sample data for testing.";

fn call(app: &mut Atlas, method: &str, payload: Value) -> Value {
    app.call(method, payload)
        .unwrap_or_else(|e| panic!("{method} failed: {e}"))
}

fn save(app: &mut Atlas, record: Value) -> Value {
    call(app, "records.save", json!({"record":record}))
}

#[test]
fn import_review_budget_work_reminder_restart_and_revision() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    let drafts = call(
        &mut app,
        "imports.parse",
        json!({"text":CONFIRMATION,"format":"text"}),
    );
    assert_eq!(drafts.as_array().unwrap().len(), 1);
    assert_eq!(drafts[0]["record"]["totalMinor"], 90000);
    assert_eq!(drafts[0]["record"]["checkOut"], "2027-01-11");
    assert!(drafts[0]["record"]["facilities"]["internet"].is_null());
    let stay = call(&mut app, "imports.accept", json!({"id":drafts[0]["id"]}));
    assert_eq!(stay["status"], "confirmed");
    assert_eq!(stay["sourceIds"].as_array().unwrap().len(), 1);
    save(
        &mut app,
        json!({"kind":"payment","bookingId":stay["id"],"date":"2026-11-20","amountMinor":90000,"currency":"USD","type":"payment","status":"settled"}),
    );
    save(
        &mut app,
        json!({"kind":"budget","month":"2026-12","currency":"USD","incomeMinor":200000,"openingBalanceMinor":50000,"reserveMinor":20000,"allowanceMinor":31000}),
    );
    let objective = save(
        &mut app,
        json!({"kind":"objective","name":"Synthetic grocery stop","stayId":stay["id"],"category":"groceries","date":"2026-12-14","startTime":"10:00","endTime":"10:30","timezone":"Europe/Lisbon","currency":"USD","totalMinor":5000,"status":"planned"}),
    );
    let december = call(
        &mut app,
        "budget.summary",
        json!({"month":"2026-12","today":"2026-12-01"}),
    );
    let january = call(&mut app, "budget.summary", json!({"month":"2027-01"}));
    let november = call(&mut app, "budget.summary", json!({"month":"2026-11"}));
    assert_eq!(december["accommodationMinor"], 60000);
    assert_eq!(january["accommodationMinor"], 30000);
    assert_eq!(november["bookingCashFlowMinor"], 90000);
    assert_eq!(december["bookingCashFlowMinor"], 0);
    assert_eq!(december["occupiedNights"], 20);
    assert_eq!(december["daysInMonth"], 31);
    assert_eq!(december["forecastMinor"], 91000);
    assert_eq!(december["allowanceRemainingMinor"], 26000);
    assert_eq!(december["remainingDays"], 31);
    let work = call(
        &mut app,
        "work.window",
        json!({"date":"2026-12-14","timezone":"Europe/Lisbon"}),
    );
    assert_eq!(work["requiredMinutes"], 480);
    assert_eq!(work["requiredOverlapMinutes"], 240);
    assert_eq!(work["destinationStart"], "2026-12-14T16:00:00+00:00");
    assert_eq!(work["suggestionMeetsHours"], true);
    assert!(work["conflicts"].as_array().unwrap().is_empty());
    assert!(work["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["type"] == "unverified_connectivity"));
    let reminder = save(
        &mut app,
        json!({"kind":"reminder","name":"Synthetic checkout preparation","bookingId":stay["id"],"dueAt":"2027-01-10T18:00:00Z","catchUp":"deliver"}),
    );
    assert_eq!(reminder["state"], "pending");
    let calendar = call(&mut app, "calendar.export", json!({}));
    let ics = calendar["content"].as_str().unwrap();
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("DTEND;VALUE=DATE:20270111"));
    let snapshot = call(&mut app, "snapshot", json!({}));
    drop(app);

    let mut app = Atlas::open(directory.path()).unwrap();
    assert_eq!(call(&mut app, "snapshot", json!({})), snapshot);
    let persisted_objectives = call(&mut app, "records.list", json!({"kind":"objective"}));
    assert_eq!(persisted_objectives[0]["id"], objective["id"]);
    let due = call(
        &mut app,
        "reminders.due",
        json!({"now":"2027-01-11T08:00:00Z"}),
    );
    assert_eq!(due.as_array().unwrap().len(), 1);
    assert_eq!(due[0]["id"], reminder["id"]);
    assert_eq!(due[0]["state"], "delivered");
    assert!(call(
        &mut app,
        "reminders.due",
        json!({"now":"2027-01-11T09:00:00Z"})
    )
    .as_array()
    .unwrap()
    .is_empty());

    let revision =
        CONFIRMATION.replace("Total: USD 900.00", "Total: USD 930.00") + "\nRevised confirmation";
    let revised_drafts = call(
        &mut app,
        "imports.parse",
        json!({"text":revision,"format":"text"}),
    );
    assert_eq!(revised_drafts[0]["existingRecordId"], stay["id"]);
    let revised = call(
        &mut app,
        "imports.accept",
        json!({"id":revised_drafts[0]["id"]}),
    );
    assert_eq!(revised["id"], stay["id"]);
    assert_eq!(revised["totalMinor"], 93000);
    assert_eq!(revised["sourceIds"].as_array().unwrap().len(), 2);
    assert_eq!(
        call(&mut app, "records.list", json!({"kind":"stay"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let replay = call(
        &mut app,
        "imports.parse",
        json!({"text":revision,"format":"text"}),
    );
    assert_eq!(replay[0]["id"], revised_drafts[0]["id"]);
    let accepted_again = call(&mut app, "imports.accept", json!({"id":replay[0]["id"]}));
    assert_eq!(accepted_again["id"], stay["id"]);
    assert_eq!(accepted_again["version"], revised["version"]);
    let updated_budget = call(&mut app, "budget.summary", json!({"month":"2026-12"}));
    assert_eq!(updated_budget["accommodationMinor"], 62000);
    assert_eq!(updated_budget["paymentStatuses"][0]["settledMinor"], 90000);
    assert_eq!(
        updated_budget["paymentStatuses"][0]["outstandingMinor"],
        3000
    );
    let history = call(&mut app, "records.history", json!({"id":stay["id"]}));
    assert_eq!(history[0]["record"]["totalMinor"], 90000);
    assert_eq!(
        history.as_array().unwrap().last().unwrap()["record"]["totalMinor"],
        93000
    );
}

#[test]
fn populated_database_migration_and_document_restore_preserve_history() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    let drafts = call(&mut app, "imports.parse", json!({"text":CONFIRMATION}));
    let stay = call(&mut app, "imports.accept", json!({"id":drafts[0]["id"]}));
    let document = call(
        &mut app,
        "imports.file",
        json!({"filename":"synthetic-notes.bin","contentBase64":STANDARD.encode(b"Synthetic attachment; no private information.")}),
    );
    let document_id = document["document"]["id"].clone();
    assert!(document_id.is_string());
    let backup = call(&mut app, "backup.export", json!({}));
    drop(app);

    let db = rusqlite::Connection::open(directory.path().join("atlas.sqlite3")).unwrap();
    db.execute_batch("PRAGMA user_version=1;").unwrap();
    drop(db);
    let mut app = Atlas::open(directory.path()).unwrap();
    let restored_records = call(&mut app, "records.list", json!({"kind":"stay"}));
    assert_eq!(restored_records[0]["id"], stay["id"]);
    assert_eq!(restored_records[0]["totalMinor"], 90000);
    assert_eq!(
        call(&mut app, "records.history", json!({"id":stay["id"]}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(std::fs::read_dir(directory.path().join("backups"))
        .unwrap()
        .any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("pre-migration-v1")));
    let mut changed = restored_records[0].clone();
    changed["name"] = json!("Synthetic changed name");
    save(&mut app, changed);
    let result = call(
        &mut app,
        "backup.restore",
        json!({"filename":backup["filename"]}),
    );
    assert!(result["preRestoreBackup"]["filename"].is_string());
    assert_eq!(
        call(&mut app, "records.list", json!({"kind":"stay"}))[0]["name"],
        "Riverside apartment"
    );
    let content = call(&mut app, "documents.read", json!({"id":document_id}));
    assert_eq!(
        STANDARD
            .decode(content["contentBase64"].as_str().unwrap())
            .unwrap(),
        b"Synthetic attachment; no private information."
    );
}

#[test]
fn stale_import_review_cannot_overwrite_newer_manual_changes() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    let drafts = call(&mut app, "imports.parse", json!({"text":CONFIRMATION}));
    let stay = call(&mut app, "imports.accept", json!({"id":drafts[0]["id"]}));
    let revision = CONFIRMATION.replace("900.00", "910.00");
    let revised_drafts = call(&mut app, "imports.parse", json!({"text":revision}));
    let mut manual = stay;
    manual["notes"] = json!("Synthetic owner note written after import review opened");
    let saved = save(&mut app, manual);
    let conflict = app
        .call("imports.accept", json!({"id":revised_drafts[0]["id"]}))
        .unwrap_err();
    assert!(conflict.contains("Conflict"));
    assert_eq!(
        call(&mut app, "records.list", json!({"kind":"stay"}))[0],
        saved
    );
}

#[test]
fn recorded_grocery_cost_replaces_estimate_and_does_not_double_charge_allowance() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    save(
        &mut app,
        json!({"kind":"budget","month":"2026-12","currency":"USD","allowanceMinor":30000,"incomeMinor":100000}),
    );
    let objective = save(
        &mut app,
        json!({"kind":"objective","name":"Synthetic grocery errand","date":"2026-12-05","timezone":"UTC","currency":"USD","totalMinor":5000,"status":"planned"}),
    );
    save(
        &mut app,
        json!({"kind":"expense","name":"Synthetic grocery receipt","date":"2026-12-05","currency":"USD","amountMinor":4500,"objectiveId":objective["id"],"category":"groceries"}),
    );
    let summary = call(
        &mut app,
        "budget.summary",
        json!({"month":"2026-12","today":"2026-12-06"}),
    );
    assert_eq!(summary["expensesMinor"], 4500);
    assert_eq!(summary["objectivesMinor"], 0);
    assert_eq!(summary["allowanceRemainingMinor"], 25500);
    assert_eq!(summary["forecastMinor"], 30000);
    assert_eq!(summary["availableMinor"], 70000);
}
