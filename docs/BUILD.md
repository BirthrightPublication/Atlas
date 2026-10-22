# Development and packaging

The desktop is a Tauri 2 native window with React/TypeScript assets and Rust
services. Data belongs in the OS application-data directory for
`org.birthright.atlas`, separate from the checkout and program files. No local
database, document, private email or model should be committed.

Use the existing checkout in an isolated cloud task. Do not create another Git
worktree unless explicitly requested. Windows is the first intended release
platform; a workflow definition alone does not establish Windows compatibility.

## Toolchain

Use Rust 1.90.0 (`rust-toolchain.toml`) and Node 24.19.0. `Cargo.lock` and
`package-lock.json` retain the tested dependency resolution. Install with
`npm ci` and use `cargo ... --locked`. No software license has been selected for
the application. Dependencies retain their respective licenses.

Windows development requires Visual Studio 2022 C++ build tools with the Windows
SDK, Rust's MSVC target, and Microsoft Edge WebView2. The NSIS installer uses the
official WebView2 bootstrapper when WebView2 is missing; this first installation
requires Internet access. The installed app needs no developer toolchain.

On Debian/Ubuntu, install `build-essential pkg-config libwebkit2gtk-4.1-dev
libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev patchelf`.
macOS needs Xcode Command Line Tools. Follow the current official Tauri platform
prerequisites if system package names differ.

The prepared unprivileged Debian 13 cloud workspace uses
`bash scripts/install-cloud.sh` to verify/download Rust and signed Debian
packages into `/workspace`. Before Rust commands in this workspace, run
`source scripts/cloud-env.sh`. The script is specific to this Linux cloud
machine; ordinary desktop installations use their native package managers.

The cloud SDK is extracted under `/workspace`, so WebKit's canonical subprocess
paths need a read-only namespace mapping. `scripts/cloud-native.sh <command>`
constructs that mapping, keeps the installed `/usr` unchanged, and preserves
WebKit's subprocess sandbox. The outer execution sandbox may block nested UID
mapping; in that case use the execution tool's normal escalation flow rather
than disabling WebKit sandboxing. Browser/service validation does not require
this namespace. Native smoke example, with an already running Xvfb display:

```sh
DISPLAY=:97 XDG_DATA_HOME=/tmp/atlas-native-smoke \
  bash scripts/cloud-native.sh dbus-run-session -- \
  target/x86_64-unknown-linux-gnu/release/atlas
```

Use synthetic data in the smoke directory. Verify a stay can be saved through
the native drawer, appears as a calendar band, and remains after closing and
reopening the window. Native packaging in this cloud workspace also runs inside
the mapping: `bash scripts/cloud-native.sh node scripts/build-desktop.mjs
--target x86_64-unknown-linux-gnu --bundles appimage,deb`.

## Run and validate

From the repository root:

```sh
npm ci
cargo test --workspace --locked
node --test scripts/release.test.mjs
npm run build
npm run desktop
```

`npm run desktop` starts Vite and launches the native window; native requests use
IPC directly and do not require the companion server. The browser development
workflow uses `atlas-host` at `127.0.0.1:47832` and Vite at `127.0.0.1:1420`.
See the companion documentation for its explicit startup arguments and security
boundaries. A cloud onboarding task uses loopback requests for checks; it does
not create a public preview.

The desktop polls persisted due reminders every 30 seconds and asks the email
service whether an automatic sync is due every minute. These workers stop when
the app exits or the computer sleeps. On reopen, persisted catch-up rules apply.
OS notification permission can prevent a banner; the notification inbox still
shows delivered records. Phone push delivery is a separate capability.

## Native packages

```sh
# Run on Windows (MSVC).
node scripts/build-desktop.mjs --target x86_64-pc-windows-msvc --bundles nsis
# Run on Linux.
node scripts/build-desktop.mjs --target x86_64-unknown-linux-gnu --bundles appimage,deb
# Run on an Apple Silicon Mac (use x86_64-apple-darwin on an Intel runner).
node scripts/build-desktop.mjs --target aarch64-apple-darwin --bundles dmg
```

The wrapper packages the companion host executable and frontend resources as
well as the desktop window, so installed users do not need Rust or Node to
enable companion access. Trusted HTTPS/private networking is configured
separately. Bundles are under `target/<target>/release/bundle`. `scripts/stage-release.mjs`
copies explicitly named artifacts to platform directories and writes SHA-256
metadata. The window and UI display **Birthright: Atlas**; filesystem product
names omit the colon because Windows paths cannot contain it.

`.github/workflows/build.yml` tests and packages on native Windows, Linux, Apple
Silicon, and Intel macOS runners. Successful CI artifacts still need native
installation/startup checks. The local Linux build cannot establish Windows or
macOS installation, publisher-signing, or real phone support.
