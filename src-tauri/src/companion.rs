use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::Manager;

#[derive(Clone, Default)]
pub struct CompanionState(pub Arc<Mutex<Companion>>);

#[derive(Default)]
pub struct Companion {
    child: Option<Child>,
    origin: Option<String>,
}

impl Drop for Companion {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Companion {
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.origin = None;
    }

    fn status(&mut self) -> Value {
        let running = self
            .child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_none());
        if !running {
            self.child = None;
            self.origin = None;
        }
        json!({"running":running,"origin":self.origin,"port":47832,
            "message":"Pairing requires a trusted HTTPS private-access proxy. The companion host binds only to loopback; it stops when Atlas exits."})
    }
}

fn executable() -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|_| "Cannot locate the desktop executable")?;
    let parent = current
        .parent()
        .ok_or("Cannot locate the desktop executable directory")?;
    let name = if cfg!(windows) {
        "atlas-host.exe"
    } else {
        "atlas-host"
    };
    let bundled = parent.join(name);
    if bundled.is_file() {
        return Ok(bundled);
    }
    #[cfg(debug_assertions)]
    {
        let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        for profile in ["debug", "release"] {
            let path = checkout.join("target").join(profile).join(name);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err("The companion executable is missing. Install a bundle built with scripts/build-desktop.mjs.".into())
}

fn command() -> Result<Command, String> {
    let mut command = Command::new(executable()?);
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    Ok(command)
}

fn control(app: &tauri::AppHandle, action: &str, device_id: Option<&str>) -> Result<Value, String> {
    let mut command = command()?;
    command.arg("--data-dir").arg(
        app.path()
            .app_data_dir()
            .map_err(|_| "Application data directory unavailable")?,
    );
    command.arg(match action {
        "pair" => "--pair",
        "devices" => "--devices",
        "revoke" => "--revoke",
        _ => return Err("Unsupported companion action".into()),
    });
    if action == "revoke" {
        let id = device_id
            .filter(|id| !id.is_empty() && id.len() < 100)
            .ok_or("Device ID is required")?;
        command.arg(id);
    }
    let output = command
        .output()
        .map_err(|_| "Could not contact the companion host")?;
    if !output.status.success() {
        return Err("Companion control failed. Start the companion host first and check its private-access configuration.".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "Invalid companion response".into())
}

pub fn call(
    app: &tauri::AppHandle,
    state: &CompanionState,
    method: &str,
    payload: &Value,
) -> Result<Value, String> {
    match method {
        "companion.pair" => control(app, "pair", None),
        "companion.devices" => control(app, "devices", None),
        "companion.revoke" => control(
            app,
            "revoke",
            payload.get("deviceId").and_then(Value::as_str),
        ),
        "companion.status" => Ok(state
            .0
            .lock()
            .map_err(|_| "Companion state unavailable")?
            .status()),
        "companion.stop" => {
            let mut companion = state.0.lock().map_err(|_| "Companion state unavailable")?;
            companion.stop();
            Ok(companion.status())
        }
        "companion.start" => {
            let origin = payload
                .get("origin")
                .and_then(Value::as_str)
                .ok_or("Trusted HTTPS origin is required")?;
            let parsed = url::Url::parse(origin).map_err(|_| "Invalid companion origin")?;
            if parsed.scheme() != "https"
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
                || parsed.path() != "/"
            {
                return Err(
                    "Use a trusted HTTPS origin with no path or embedded credentials".into(),
                );
            }
            let origin = parsed.origin().ascii_serialization();
            let mut companion = state.0.lock().map_err(|_| "Companion state unavailable")?;
            if companion.status()["running"] == true {
                return Err("Stop the existing companion host before changing its origin".into());
            }
            if std::net::TcpStream::connect_timeout(
                &"127.0.0.1:47832".parse().unwrap(),
                Duration::from_millis(200),
            )
            .is_ok()
            {
                return Err("Port 47832 is already in use. Stop the existing development or companion host first.".into());
            }
            let resources = app
                .path()
                .resource_dir()
                .map_err(|_| "Application resources unavailable")?
                .join("companion-ui");
            #[cfg(debug_assertions)]
            let resources = if !resources.is_dir() {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist")
            } else {
                resources
            };
            if !resources.join("index.html").is_file() {
                return Err("Companion UI resources missing. Build the frontend before starting the companion.".into());
            }
            let mut child = command()?
                .args(["--origin", &origin, "--static-dir"])
                .arg(resources)
                .arg("--data-dir")
                .arg(
                    app.path()
                        .app_data_dir()
                        .map_err(|_| "Application data directory unavailable")?,
                )
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| "Could not start companion host")?;
            for _ in 0..20 {
                if child
                    .try_wait()
                    .map_err(|_| "Could not inspect companion process")?
                    .is_some()
                {
                    return Err("Companion host exited during startup".into());
                }
                if control(app, "devices", None).is_ok() {
                    companion.child = Some(child);
                    companion.origin = Some(origin);
                    return Ok(companion.status());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
            Err("Companion host did not pass its authenticated readiness check".into())
        }
        _ => Err("Unknown companion method".into()),
    }
}
