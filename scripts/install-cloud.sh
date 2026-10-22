#!/usr/bin/env bash
# Reproduce the SDK in an unprivileged Debian 13 Codex workspace.
set -euo pipefail
cd /workspace/Atlas
export CARGO_HOME=/workspace/.cargo
export RUSTUP_HOME=/workspace/.rustup
export PATH="/workspace/.cargo/bin:$PATH"
if ! command -v rustup >/dev/null; then
  atlas_download=$(mktemp -d /tmp/atlas-rustup.XXXXXX)
  trap 'rm -rf "$atlas_download"' EXIT
  curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 \
    https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init -o "$atlas_download/rustup-init"
  curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 \
    https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init.sha256 -o "$atlas_download/rustup-init.sha256"
  (cd "$atlas_download" && sha256sum --check rustup-init.sha256)
  chmod +x "$atlas_download/rustup-init"
  "$atlas_download/rustup-init" -y --no-modify-path --profile minimal --default-toolchain 1.90.0
fi
rustup toolchain install 1.90.0 --profile minimal --component rustfmt --component clippy
atlas_sdk=/workspace/atlas-sysroot
mkdir -p "$atlas_sdk/apt/lists/partial" "$atlas_sdk/apt/archives/partial"
printf '%s\n' 'deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian trixie main' > "$atlas_sdk/apt/sources.list"
atlas_apt=(/usr/bin/apt-get -o Dir::Etc::sourcelist="$atlas_sdk/apt/sources.list" -o Dir::Etc::sourceparts=- -o Dir::Etc::parts=- -o Dir::State::lists="$atlas_sdk/apt/lists" -o Dir::Cache::archives="$atlas_sdk/apt/archives" -o Debug::NoLocking=1)
"${atlas_apt[@]}" update
"${atlas_apt[@]}" --download-only --no-install-recommends -y install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev patchelf xvfb
python3 - <<'PY'
from pathlib import Path
import os
import subprocess
root = Path('/workspace/atlas-sysroot')
for package in sorted((root / 'apt/archives').glob('*.deb')):
    subprocess.run(['dpkg-deb', '-x', str(package), str(root)], check=True)
# Deduplicate symlink aliases so a .pc file is relocated only once.
for pc in {p.resolve() for p in root.glob('usr/**/pkgconfig/*.pc')}:
    pc.write_text(pc.read_text().replace('/usr', str(root / 'usr')))
for link in root.glob('usr/lib/**/*'):
    if link.is_symlink() and not link.exists():
        target = Path('/') / link.relative_to(root).parent / os.readlink(link)
        if target.exists():
            link.unlink()
            link.symlink_to(target.resolve())
PY
source scripts/cloud-env.sh
node -e 'if (Number(process.versions.node.split(".")[0]) < 22) throw new Error("Node 22 or newer required; validated on 24.19.0")'
npm ci --cache /workspace/.npm-cache
cargo fetch --locked
pkg-config --modversion gtk+-3.0 webkit2gtk-4.1 dbus-1
