use atlas_core::{model_store, Atlas};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
fn stop(name: &str, a: &str, b: &str) -> Value {
    json!({"name":name,"arrival":a,"departure":b,"timezone":"Europe/Rome","context":"on_foot","currency":"USD","totalMinor":90000,"dailyBudgetMinor":1000})
}
#[test]
fn draft_journey_costs_stay_hypothetical_and_confirmed_dates_survive() {
    let dir = tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    let stay=a.call("records.save",json!({"record":{"kind":"stay","name":"Original stay","checkIn":"2026-12-12","checkOut":"2027-01-11","timezone":"Europe/Rome","currency":"USD","totalMinor":90000,"status":"confirmed"}})).unwrap();
    let before = a.call("snapshot", json!({})).unwrap();
    let mut s = stop("Rome", "2026-12-12", "2027-01-11");
    s["stayId"] = stay["id"].clone();
    let preview = a
        .call("journeys.preview", json!({"stops":[s.clone()]}))
        .unwrap();
    assert_eq!(preview["recordsChanged"], false);
    let costs = preview["monthlyScenarioCosts"].as_array().unwrap();
    assert_eq!(costs[0]["accommodationMinor"], 60000);
    assert_eq!(costs[1]["accommodationMinor"], 30000);
    assert_eq!(a.call("snapshot", json!({})).unwrap(), before);
    s["departure"] = json!("2027-01-12");
    let p = a
        .call("journeys.preview", json!({"stops":[s.clone()]}))
        .unwrap();
    assert!(p["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["type"] == "confirmed_dates"));
    let saved = a
        .call(
            "journeys.save",
            json!({"record":{"kind":"trip","name":"Review journey","stops":[s]}}),
        )
        .unwrap();
    assert_eq!(saved["status"], "draft");
    assert_eq!(
        a.call("records.list", json!({"kind":"stay"})).unwrap()[0],
        stay
    );
    assert!(a.call("journeys.preview",json!({"stops":[stop("B","2027-02-01","2027-02-10"),stop("A","2027-01-01","2027-01-10")]})).is_err());
}
#[test]
fn schedule_proposal_requires_individual_decisions_and_rechecks_conflicts() {
    let dir = tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    let p = a
        .call(
            "journeys.propose",
            json!({"start":"2027-01-09","timezone":"Europe/Rome","days":2}),
        )
        .unwrap();
    assert_eq!(p["items"].as_array().unwrap().len(), 2);
    assert!(a
        .call("records.list", json!({"kind":"objective"}))
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    let item = &p["items"][0];
    let objective = item["record"].clone();
    a.call("records.save", json!({"record":objective})).unwrap();
    assert!(a
        .call(
            "journeys.accept",
            json!({"id":p["id"],"itemId":item["id"],"expectedVersion":p["version"]})
        )
        .is_err());
    let item = &p["items"][1];
    let accepted=a.call("journeys.accept",json!({"id":p["id"],"itemId":item["id"],"expectedVersion":p["version"],"name":"Reviewed museum visit"})).unwrap();
    assert_eq!(accepted["items"][1]["state"], "accepted");
    assert!(a
        .call(
            "journeys.reject",
            json!({"id":p["id"],"itemId":p["items"][0]["id"],"expectedVersion":p["version"]})
        )
        .is_err());
    drop(a);
    let mut a = Atlas::open(dir.path()).unwrap();
    assert_eq!(
        a.call("records.list", json!({"kind":"proposal"})).unwrap()[0],
        accepted
    );
}
#[test]
fn routes_retain_unknowns_and_offline_documents_require_selection() {
    let dir = tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    let from=a.call("records.save",json!({"record":{"kind":"place","name":"Saved start","address":"Roma Termini","timezone":"Europe/Rome"}})).unwrap();
    let to=a.call("records.save",json!({"record":{"kind":"place","name":"Saved end","address":"Florence SMN","timezone":"Europe/Rome"}})).unwrap();
    let route=a.call("routes.draft",json!({"fromId":from["id"],"toId":to["id"],"mode":"transit","date":"2027-01-09","notes":"Check official timetable"})).unwrap();
    assert_eq!(route["verified"], false);
    assert!(route["durationMinutes"].is_null());
    assert!(route["directionsUrl"]
        .as_str()
        .unwrap()
        .starts_with("https://www.google.com/maps/dir/"));
    let trip=a.call("journeys.save",json!({"record":{"kind":"trip","name":"Offline journey","stops":[stop("Rome","2027-01-09","2027-01-12")]}})).unwrap();
    a.call(
        "imports.file",
        json!({"filename":"synthetic-v05.txt","contentBase64":"U3ludGhldGljIG9ubHk="}),
    )
    .unwrap();
    let docs = a.call("records.list", json!({"kind":"document"})).unwrap();
    let pack = a
        .call("journeys.offline", json!({"id":trip["id"]}))
        .unwrap();
    assert_eq!(pack["documents"].as_array().unwrap().len(), 0);
    assert!(!pack["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "source"));
    let pack = a
        .call(
            "journeys.offline",
            json!({"id":trip["id"],"documentIds":[docs[0]["id"]]}),
        )
        .unwrap();
    assert_eq!(pack["documents"].as_array().unwrap().len(), 1);
}
#[test]
fn model_catalog_pinned_and_download_not_started_without_approval() {
    let dir = tempdir().unwrap();
    let a = Arc::new(Mutex::new(Atlas::open(dir.path()).unwrap()));
    let c = model_store::query(&a, "ai.models", &json!({})).unwrap();
    assert_eq!(c["models"]["0.6b"]["bytes"], 639446688);
    assert_eq!(c["models"]["1.7b"]["sha256"].as_str().unwrap().len(), 64);
    assert!(
        model_store::query(&a, "ai.install", &json!({"model":"0.6b"}))
            .unwrap_err()
            .contains("Approve")
    );
    assert!(!dir.path().join("local-ai/Qwen3-0.6B-Q8_0.gguf").exists());
    assert!(model_store::query(&a, "ai.verify", &json!({"model":"../../secret"})).is_err());
    assert!(model_store::query(&a, "ai.enable", &json!({"model":"0.6b"})).is_err());
}
