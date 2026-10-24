//! Explicit opt-in local benchmark using synthetic confirmations only.
//! cargo run -p atlas-core --example benchmark_ai -- /absolute/private/settings.json [--idle]
use atlas_core::ai;
use serde_json::{json, Value};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
fn child_peak_rss_kib() -> u64 {
    let pid = std::process::id();
    let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) else {
        return 0;
    };
    let mut peak = 0;
    for task in tasks.flatten() {
        let Ok(children) = fs::read_to_string(task.path().join("children")) else {
            continue;
        };
        for child in children.split_whitespace() {
            if let Ok(status) = fs::read_to_string(format!("/proc/{child}/status")) {
                for line in status
                    .lines()
                    .filter(|l| l.starts_with("VmHWM:") || l.starts_with("VmRSS:"))
                {
                    peak = peak.max(
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0),
                    );
                }
            }
        }
    }
    peak
}

#[cfg(not(target_os = "linux"))]
fn child_peak_rss_kib() -> u64 {
    0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("Pass a private settings JSON path. No model is downloaded by this benchmark.")?;
    let settings: Value = serde_json::from_slice(&fs::read(path)?)?;
    if !ai::status(&settings)["configured"]
        .as_bool()
        .unwrap_or(false)
    {
        return Err("Inference configuration is incomplete".into());
    }
    let running = Arc::new(AtomicBool::new(true));
    let maximum = Arc::new(AtomicU64::new(0));
    let sampler = {
        let running = running.clone();
        let maximum = maximum.clone();
        thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                maximum.fetch_max(child_peak_rss_kib(), Ordering::Relaxed);
                thread::sleep(Duration::from_millis(25));
            }
        })
    };
    let examples = [
        ("complete", "Booking confirmed\nProperty: Synthetic Terrace Apartment\nProvider: Example Travel\nReservation ID: DEMO-AI-001\nCheck-in: 2026-12-12\nCheck-out: 2027-01-11\nTimezone: Europe/Lisbon\nTotal: USD 900.00\nAddress: 12 Example Street, Synthetic City"),
        ("missing_information", "Booking confirmed\nProperty: Synthetic Garden Room\nReservation ID: DEMO-AI-002\nCheck-in: 2026-12-15\nCheck-out: 2026-12-18\nTotal: $240.00\nInternet has not been confirmed."),
        ("revised", "Revised booking confirmation\nProperty: Synthetic Terrace Apartment\nProvider: Example Travel\nReservation ID: DEMO-AI-001\nCheck-in: 2026-12-12\nCheck-out: 2027-01-12\nTimezone: Europe/Lisbon\nTotal: USD 930.00\nAddress: 12 Example Street, Synthetic City"),
    ];
    let mut results = Vec::new();
    for (name, source) in examples {
        let started = Instant::now();
        let result = ai::extract(source, &settings);
        let elapsed = started.elapsed().as_millis();
        let record = match result {
            Ok(draft) => json!({"case":name,"elapsedMs":elapsed,"ok":true,"draft":draft}),
            Err(error) => json!({"case":name,"elapsedMs":elapsed,"ok":false,"error":error}),
        };
        println!("{}", record);
        results.push(record);
    }
    let before_idle = ai::status(&settings)["running"].as_bool().unwrap_or(false);
    let mut idle_stopped = Value::Null;
    if args.iter().any(|arg| arg == "--idle") {
        let started = Instant::now();
        while ai::status(&settings)["running"].as_bool().unwrap_or(false)
            && started.elapsed() < Duration::from_secs(145)
        {
            thread::sleep(Duration::from_secs(2));
        }
        idle_stopped = json!(!ai::status(&settings)["running"].as_bool().unwrap_or(false));
    }
    let cancellation = if results.iter().any(|r| r["ok"] == true) {
        let config = settings.clone();
        let worker = thread::spawn(move || {
            ai::extract("Booking confirmed\nProperty: Synthetic Cancellation Test\nCheck-in: 2026-12-12\nCheck-out: 2026-12-13\nTimezone: Europe/Lisbon\nTotal: USD 90.00",&config)
        });
        let started = Instant::now();
        while !ai::status(&settings)["busy"].as_bool().unwrap_or(false)
            && started.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(100));
        ai::cancel();
        let result = worker.join().map_err(|_| "Inference worker panicked")?;
        json!({"returnedError":result.is_err(),"elapsedMs":started.elapsed().as_millis(),"runtimeStopped":!ai::status(&settings)["running"].as_bool().unwrap_or(false)})
    } else {
        Value::Null
    };
    ai::cancel();
    running.store(false, Ordering::Relaxed);
    let _ = sampler.join();
    println!(
        "{}",
        json!({"summary":{"cases":results.len(),"succeeded":results.iter().filter(|r|r["ok"]==true).count(),"peakChildRssKiB":maximum.load(Ordering::Relaxed),"runtimeWasRunning":before_idle,"idleStopped":idle_stopped,"cancellation":cancellation,"model":settings["ai"]["modelName"],"platform":std::env::consts::OS,"scope":"This machine and these synthetic fixtures only."}})
    );
    if results.iter().any(|r| r["ok"] != true) {
        return Err("At least one inference case failed; inspect the JSON results".into());
    }
    Ok(())
}
