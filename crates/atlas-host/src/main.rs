use atlas_host::{local_control, router, Config, HostState};
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("Atlas host: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut config = Config {
        data_dir: default_data_dir()?,
        static_dir: None,
        port: 47832,
        dev: false,
        origin: None,
    };
    let mut command: Option<String> = None;
    let mut device_id = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dev" => config.dev = true,
            "--data-dir" => {
                config.data_dir = PathBuf::from(args.next().ok_or("--data-dir needs a path")?)
            }
            "--static-dir" => {
                config.static_dir = Some(PathBuf::from(
                    args.next().ok_or("--static-dir needs a path")?,
                ))
            }
            "--origin" => {
                config.origin = Some(args.next().ok_or("--origin needs an HTTPS origin")?)
            }
            "--port" => {
                config.port = args
                    .next()
                    .ok_or("--port needs a number")?
                    .parse()
                    .map_err(|_| "Invalid port")?
            }
            "--pair" => command = Some("pair".into()),
            "--devices" => command = Some("devices".into()),
            "--revoke" => {
                command = Some("revoke".into());
                device_id = Some(args.next().ok_or("--revoke needs a device ID")?);
            }
            "--help" | "-h" => {
                println!("Birthright: Atlas companion host\n\nDevelopment: atlas-host --dev [--data-dir PATH]\nCompanion:   atlas-host --origin https://private-host --static-dir dist [--data-dir PATH]\nPair phone:  atlas-host --pair [--data-dir PATH]\nList phones: atlas-host --devices [--data-dir PATH]\nRevoke:      atlas-host --revoke DEVICE_ID [--data-dir PATH]\n\nAll modes bind 127.0.0.1 only; default port 47832 (--port overrides).\nCompanion mode requires a separately configured trusted HTTPS reverse proxy.\nNever forward or expose --dev mode to a network.");
                return Ok(());
            }
            _ => return Err(format!("Unknown argument {arg}; use --help")),
        }
    }
    if let Some(command) = command {
        let result = local_control(
            &config.data_dir,
            config.port,
            &command,
            device_id.as_deref(),
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let port = config.port;
    let dev = config.dev;
    let state = HostState::open(config)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .map_err(|e| format!("Bind local service: {e}"))?;
        eprintln!(
            "Atlas {} service running on loopback port {port}",
            if dev {
                "development"
            } else {
                "paired companion"
            }
        );
        axum::serve(listener, router(state))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .map_err(|e| e.to_string())
    })
}

fn default_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join("org.birthright.atlas"))
            .ok_or("APPDATA is unavailable; pass --data-dir".into());
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(|p| PathBuf::from(p).join("Library/Application Support/org.birthright.atlas"))
            .ok_or("Home directory unavailable; pass --data-dir".into());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(path).join("org.birthright.atlas"));
        }
        std::env::var_os("HOME")
            .map(|p| PathBuf::from(p).join(".local/share/org.birthright.atlas"))
            .ok_or("Home directory unavailable; pass --data-dir".into())
    }
}
