//! Compiled only into disposable CI test installers; absent from preview/release builds.
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf};
use tauri::Manager;
use tauri_plugin_updater::UpdaterBuilder;

fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    // NSIS RunAsUser can create a fresh user environment after installation.
    // These test-only binaries retain the build runner's exact temporary root.
    if option_env!("GITHUB_ACTIONS") != Some("true") {
        return Err("Controlled updater tests require a disposable CI runner".into());
    }
    let temp = std::fs::canonicalize(option_env!("RUNNER_TEMP").ok_or("RUNNER_TEMP missing")?)
        .map_err(|_| "Invalid runner temp")?;
    let data = std::fs::canonicalize(
        app.path()
            .app_data_dir()
            .map_err(|_| "Application data unavailable")?,
    )
    .map_err(|_| "Test data unavailable")?;
    if !data.starts_with(&temp) {
        return Err("Refusing to test updates against a non-temporary profile".into());
    }
    Ok(data)
}

fn test_case() -> Option<String> {
    std::env::args()
        .find_map(|arg| arg.strip_prefix("--atlas-update-test=").map(str::to_owned))
        .filter(|case| {
            matches!(
                case.as_str(),
                "older"
                    | "wrong-platform"
                    | "bad-signature"
                    | "interrupted"
                    | "malformed"
                    | "missing-release"
                    | "valid"
            )
        })
}

pub fn trusted_test_client(builder: UpdaterBuilder) -> Result<UpdaterBuilder, String> {
    let endpoint =
        url::Url::parse(option_env!("ATLAS_UPDATE_ENDPOINT").ok_or("No controlled feed")?)
            .map_err(|_| "Invalid controlled feed")?;
    if endpoint.scheme() != "https" || endpoint.host_str() != Some("localhost") {
        return Err("Disposable test builds require a localhost HTTPS feed".into());
    }
    let pem = option_env!("ATLAS_UPDATE_TEST_CA_PEM")
        .ok_or("A controlled test trust root must be embedded")?;
    let cert =
        reqwest::Certificate::from_pem(pem.as_bytes()).map_err(|_| "Invalid test trust root")?;
    Ok(builder
        .configure_client(move |client| client.https_only(true).add_root_certificate(cert.clone())))
}

pub fn record_progress(app: &tauri::AppHandle, p: &Value) {
    let Ok(root) = root(app) else {
        return;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("controlled-update-progress.jsonl"))
    {
        let _ = writeln!(file, "{}", json!({"case":test_case(),"progress":p}));
    }
}

pub fn start(app: tauri::AppHandle) {
    let Some(case) = test_case() else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let result = run(&app, &case).await;
        if let Ok(root) = root(&app) {
            let report = json!({"case":case,"version":app.package_info().version.to_string(),"passed":result.is_ok(),"error":result.as_ref().err()});
            let _ = std::fs::write(
                root.join(format!("controlled-update-{case}.json")),
                report.to_string(),
            );
        }
        app.exit(if result.is_ok() { 0 } else { 1 });
    });
}

async fn run(app: &tauri::AppHandle, case: &str) -> Result<(), String> {
    let root = root(app)?;
    // The actual newly installed executable writes this marker after its restart.
    if app.package_info().version.pre.as_str() == "update-test.1" {
        if case != "valid" {
            return Err("Unexpected restart into target test version".into());
        }
        if std::net::TcpStream::connect("127.0.0.1:47832").is_ok() {
            return Err("The old packaged companion process survived replacement".into());
        }
        let before = std::fs::read(root.join("controlled-companion-before.json"))
            .map_err(|_| "Companion state was not captured")?;
        let after = std::fs::read(root.join("companion-devices.json"))
            .map_err(|_| "Companion configuration was lost")?;
        if before != after {
            return Err("Companion configuration changed during the update".into());
        }
        std::fs::write(
            root.join("controlled-update-restarted.json"),
            json!({"version":app.package_info().version.to_string(),"nativeRestart":true})
                .to_string(),
        )
        .map_err(|_| "Could not record native restart")?;
        return Ok(());
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let state = app.state::<crate::updates::UpdateState>();
    let check = crate::updates::check(app, &state).await;
    if ["malformed", "missing-release"].contains(&case) {
        return if check.is_err() {
            Ok(())
        } else {
            Err("An unavailable or malformed feed was accepted".into())
        };
    }
    if case == "wrong-platform" {
        return if check.as_ref().err().is_some_and(|e| {
            e.to_lowercase().contains("target") || e.to_lowercase().contains("platform")
        }) {
            Ok(())
        } else {
            Err("Wrong-platform feed was not specifically rejected".into())
        };
    }
    let status = check?;
    if case == "older" {
        return if status["available"] == false {
            Ok(())
        } else {
            Err("Older release was presented as newer".into())
        };
    }
    if status["available"] != true {
        return Err("Controlled newer release was not available".into());
    }
    if case == "valid" {
        let companion = app.state::<crate::CompanionState>();
        crate::companion::call(
            app,
            &companion,
            "companion.start",
            &json!({"origin":"https://localhost"}),
        )?;
        // A fresh host writes its device store only after a pairing mutation.
        // Generate disposable pairing state through the real service and keep
        // its returned credential out of reports and logs.
        let _ = crate::companion::call(app, &companion, "companion.pair", &json!({}))?;
        std::fs::copy(
            root.join("companion-devices.json"),
            root.join("controlled-companion-before.json"),
        )
        .map_err(|_| "Could not capture disposable companion configuration")?;
    }
    let atlas = app.state::<crate::AtlasState>();
    let result = crate::updates::install(app, &atlas, &state).await;
    if ["bad-signature", "interrupted"].contains(&case) {
        let error = result.err().ok_or("Unsafe update unexpectedly installed")?;
        if !error.contains("Download or signature verification failed") {
            return Err(format!(
                "Failure did not reach download/signature validation: {error}"
            ));
        }
        let log = std::fs::read_to_string(root.join("controlled-update-progress.jsonl"))
            .map_err(|_| "No update progress evidence")?;
        let phase = if case == "bad-signature" {
            "Verifying signature"
        } else {
            "Downloading"
        };
        if !log
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .any(|v| v["case"] == case && v["progress"]["phase"] == phase)
        {
            return Err("Expected update validation phase was not reached".into());
        }
        return Ok(());
    }
    // On Windows a successful install launches NSIS and exits this process;
    // the updated application writes the restart marker above.
    result
}
