#!/usr/bin/env bash
# Run a native build/smoke command with the verified SDK at canonical /usr paths.
# Requires user namespaces. If an outer command sandbox prevents UID mapping,
# use the execution tool's normal escalation flow; never disable WebKit sandbox.
set -euo pipefail
cd /workspace/Atlas
source scripts/cloud-env.sh
atlas_union=$(mktemp -d /workspace/atlas-native-usr.XXXXXX)
atlas_pkgconfig=$(mktemp -d /workspace/atlas-native-pkgconfig.XXXXXX)
trap 'rm -rf "$atlas_union" "$atlas_pkgconfig"' EXIT
python3 - "$atlas_union" "$atlas_pkgconfig" <<'PY'
from pathlib import Path
import sys
system = Path('/usr')
sdk = Path('/workspace/atlas-sysroot/usr')
output = Path(sys.argv[1])
pkgconfig = Path(sys.argv[2])
def merge(relative):
    source = system / relative
    extra = sdk / relative
    names = {p.name for p in source.iterdir()} if source.is_dir() else set()
    if extra.is_dir(): names.update(p.name for p in extra.iterdir())
    for name in names:
        rel = relative / name
        src, addition, destination = system / rel, sdk / rel, output / rel
        if src.is_dir() and not src.is_symlink() and addition.is_dir() and not addition.is_symlink():
            destination.mkdir()
            merge(rel)
        elif addition.exists() or addition.is_symlink():
            target = addition.resolve()
            if target.is_relative_to(system): target = Path('/tmp/atlas-original-usr') / target.relative_to(system)
            destination.symlink_to(target)
        else:
            destination.symlink_to(Path('/tmp/atlas-original-usr') / rel)
merge(Path())
for source in sdk.glob('**/pkgconfig/*.pc'):
    (pkgconfig / source.name).write_text(source.read_text().replace(str(sdk), '/usr'))
PY
export PKG_CONFIG_PATH="$atlas_pkgconfig"
export LD_LIBRARY_PATH=/usr/lib/x86_64-linux-gnu
export LIBRARY_PATH=/usr/lib/x86_64-linux-gnu
export XDG_CACHE_HOME=/workspace/atlas-build-cache
export ATLAS_SDK_DOCS=/workspace/atlas-sysroot/usr/share/doc
bwrap --ro-bind / / --bind /workspace /workspace --bind /tmp /tmp \
  --ro-bind /usr /tmp/atlas-original-usr --ro-bind "$atlas_union" /usr \
  --dev /dev --proc /proc "$@"
