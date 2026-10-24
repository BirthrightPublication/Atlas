use crate::AtlasState;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::{Error as UpdateError, Update, UpdaterExt};

fn check_error(error: UpdateError) -> String {
    match error {
        UpdateError::ReleaseNotFound => "The production update endpoint has no accessible published manifest. Check that a signed stable release has been published; this check did not establish that Atlas is up to date.".into(),
        UpdateError::TargetNotFound(_) | UpdateError::TargetsNotFound(_) => "The published release has no compatible update for this platform. The installed application was not changed.".into(),
        UpdateError::Serialization(_) => "The published update manifest is malformed. Atlas could not check for a compatible signed release.".into(),
        other => format!("Could not reach or validate the compatible signed update feed: {other}"),
    }
}

fn download_error(error: UpdateError) -> String {
    let reason = match error {
        UpdateError::Minisign(_) | UpdateError::Base64(_) | UpdateError::SignatureUtf8(_)
        | UpdateError::SignedVersionMismatch { .. } | UpdateError::MissingSignedVersion =>
            "Signature verification failed. The package did not authenticate for this release and signing identity.".to_owned(),
        UpdateError::Reqwest(error) if error.is_timeout() => "Download timed out. Reconnect and retry the signed release.".into(),
        UpdateError::Reqwest(error) if error.is_body() => "Download was interrupted before the package was complete. Retry the signed release.".into(),
        UpdateError::Reqwest(_) => "The HTTPS download failed. Check the network connection, certificate trust and release availability.".into(),
        UpdateError::Network(status) => format!("Download failed: {status}"),
        UpdateError::Io(error) => format!("The downloaded package could not be read or saved ({:?}).", error.kind()),
        _ => "The update package could not be downloaded and verified.".into(),
    };
    format!("{reason} No update was installed and no unsigned fallback was used; your data and backup are intact.")
}

#[derive(Default)]
pub struct UpdateState {
    candidate: Mutex<Option<Update>>,
    installing: AtomicBool,
    checking: AtomicBool,
}
struct Busy<'a>(&'a AtomicBool);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn progress(
    app: &tauri::AppHandle,
    phase: &str,
    downloaded: Option<u64>,
    total: Option<u64>,
    error: Option<&str>,
) {
    let payload = json!({"phase":phase,"downloaded":downloaded,"total":total,"error":error});
    let _ = app.emit("atlas:update-progress", &payload);
    #[cfg(feature = "update-smoke")]
    crate::update_smoke::record_progress(app, &payload);
}

#[tauri::command]
pub async fn update_status(
    app: tauri::AppHandle,
    state: State<'_, UpdateState>,
) -> Result<Value, String> {
    check(&app, &state).await
}

pub async fn check(app: &tauri::AppHandle, state: &UpdateState) -> Result<Value, String> {
    if state.installing.load(Ordering::Acquire) {
        return Err("An update is already in progress".into());
    }
    if state
        .checking
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("An update check is already running".into());
    }
    let _busy = Busy(&state.checking);
    if state.installing.load(Ordering::Acquire) {
        return Err("An update is already in progress".into());
    }
    *state.candidate.lock().map_err(|_| "Updater unavailable")? = None;
    let current = app.package_info().version.to_string();
    if cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_none() {
        return Ok(
            json!({"state":"unconfigured","configured":false,"available":false,"currentVersion":current,"reason":"Automatic Linux updates require the AppImage edition. Install a verified Debian package manually."}),
        );
    }
    let (Some(key), Some(endpoint)) = (
        option_env!("ATLAS_UPDATE_PUBLIC_KEY"),
        option_env!("ATLAS_UPDATE_ENDPOINT"),
    ) else {
        return Ok(
            json!({"state":"unconfigured","configured":false,"available":false,"currentVersion":current,"reason":"This preview has no production signing identity or trusted update feed. Install a verified release manually. A properly signed build can update subsequent releases with Update Now."}),
        );
    };
    let endpoint = url::Url::parse(endpoint).map_err(|_| "Invalid compiled update endpoint")?;
    if endpoint.scheme() != "https"
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || key.trim().is_empty()
    {
        return Err("Updates require an HTTPS feed and compiled verification key".into());
    }
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let target = format!("{platform}-{}", std::env::consts::ARCH);
    let exiting_app = app.clone();
    let builder = app
        .updater_builder()
        .pubkey(key)
        .target(&target)
        .endpoints(vec![endpoint])
        .map_err(|_| "Invalid update configuration")?
        .on_before_exit(move || {
            // The Windows installer exits without the ordinary Tauri exit event.
            // Release the packaged sidecar before NSIS replaces its executable.
            atlas_core::ai::cancel();
            atlas_core::model_store::cancel_download();
            if let Ok(mut companion) = exiting_app.state::<crate::CompanionState>().0.lock() {
                companion.stop();
            }
        })
        .timeout(Duration::from_secs(60));
    #[cfg(not(feature = "update-smoke"))]
    let builder = builder.configure_client(|client| client.https_only(true));
    #[cfg(feature = "update-smoke")]
    let builder = crate::update_smoke::trusted_test_client(builder)?;
    let mut update = builder
        .build()
        .map_err(|_| "Could not initialize signed updater")?
        .check()
        .await
        .map_err(check_error)?;
    if let Some(u) = &mut update {
        u.timeout = Some(Duration::from_secs(120));
        if u.target != target
            || u.download_url.scheme() != "https"
            || !u.download_url.username().is_empty()
            || u.download_url.password().is_some()
        {
            return Err(
                "Rejected update with an incompatible target or insecure download URL".into(),
            );
        }
        let newer = semver::Version::parse(&u.version).map_err(|_| "Invalid release version")?
            > semver::Version::parse(&current).map_err(|_| "Invalid installed version")?;
        if !newer {
            return Ok(
                json!({"state":"up_to_date","configured":true,"available":false,"currentVersion":current}),
            );
        }
    }
    let response = match &update {
        Some(u) => {
            let service = app.state::<AtlasState>().0.clone();
            let version = u.version.clone();
            tauri::async_runtime::spawn_blocking(move || {
                service
                    .lock()
                    .map_err(|_| "Application service unavailable")?
                    .record_update_notice(&version)
            })
            .await
            .map_err(|_| "Notification worker interrupted")??;
            json!({"state":"update_available","configured":true,"available":true,"currentVersion":current,"version":u.version,"notes":u.body,"date":u.raw_json["pub_date"],"target":target})
        }
        None => {
            json!({"state":"up_to_date","configured":true,"available":false,"currentVersion":current})
        }
    };
    *state.candidate.lock().map_err(|_| "Updater unavailable")? = update;
    Ok(response)
}

#[tauri::command]
pub async fn update_install(
    app: tauri::AppHandle,
    atlas: State<'_, AtlasState>,
    state: State<'_, UpdateState>,
) -> Result<(), String> {
    install(&app, &atlas, &state).await
}
pub async fn install(
    app: &tauri::AppHandle,
    atlas: &AtlasState,
    state: &UpdateState,
) -> Result<(), String> {
    if state
        .installing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("An update is already in progress".into());
    }
    let _busy = Busy(&state.installing);
    let result: Result<(),String>=async {
        if state.checking.load(Ordering::Acquire){return Err("Wait for the active update check to finish".into());}
        let update=state.candidate.lock().map_err(|_|"Updater unavailable")?.clone().ok_or("Check for a signed update before installing")?;
        progress(app,"Preparing backup",None,None,None);
        let service=atlas.0.clone();
        tauri::async_runtime::spawn_blocking(move||service.lock().map_err(|_|"Application service unavailable")?.call("backup.export",json!({}))).await.map_err(|_|"Backup worker interrupted")??;
        let mut downloaded=0u64;let handle=app.clone();let verify=app.clone();
        progress(app,"Downloading",Some(0),None,None);
        let bytes=update.download(move|chunk,total|{downloaded=downloaded.saturating_add(chunk as u64);progress(&handle,"Downloading",Some(downloaded),total,None);},move||progress(&verify,"Verifying signature",None,None,None)).await.map_err(download_error)?;
        progress(app,"Installing",None,None,None);
        update.install(bytes).map_err(|error|format!("Installation failed after signature verification: {error}. Your data backup is retained."))?;
        progress(app,"Restarting",None,None,None);
        app.restart();
    }.await;
    if let Err(reason) = &result {
        progress(app, "Failed", None, None, Some(reason));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_manifest_and_platform_never_report_up_to_date() {
        assert!(check_error(UpdateError::ReleaseNotFound).contains("did not establish"));
        assert!(
            check_error(UpdateError::TargetNotFound("windows-x86_64".into()))
                .contains("no compatible update")
        );
    }

    #[test]
    fn signature_and_transport_failures_are_distinct() {
        assert!(download_error(UpdateError::MissingSignedVersion)
            .starts_with("Signature verification failed"));
        let transport = download_error(UpdateError::Network(
            "Download request failed with status: 503".into(),
        ));
        assert!(transport.starts_with("Download failed"));
        assert!(transport.contains("503"));
        assert!(!transport.contains("Signature verification failed"));
        let io = download_error(UpdateError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "synthetic private path must not be echoed",
        )));
        assert!(io.contains("PermissionDenied"));
        assert!(!io.contains("synthetic private path"));
    }
}
