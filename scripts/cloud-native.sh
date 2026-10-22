#!/usr/bin/env bash
# Run a native build/smoke command with the verified SDK at canonical /usr paths.
# Requires user namespaces. If an outer command sandbox prevents UID mapping,
# use the execution tool's normal escalation flow; never disable WebKit sandbox.
set -euo pipefail
cd /workspace/Atlas
source scripts/cloud-env.sh
atlas_union=$(mktemp -d /workspace/atlas-native-usr.XXXXXX)
# Keep this path stable: changing PKG_CONFIG_PATH forces Cargo to rebuild the
# entire GTK/WebKit dependency tree even when SDK contents are unchanged.
atlas_pkgconfig=/workspace/atlas-native-pkgconfig
mkdir -p "$atlas_pkgconfig"
trap 'rm -rf "$atlas_union"' EXIT
python3 - "$atlas_union" "$atlas_pkgconfig" <<'PY'
from pathlib import Path
import shutil
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
# The GTK bundler copies these runtime trees with cp --archive/realpath. Give
# them real files at canonical paths instead of links outside the namespace;
# all other SDK/system entries remain lightweight read-only links.
runtime_trees = [
    'lib/x86_64-linux-gnu/gtk-3.0',
    'lib/x86_64-linux-gnu/gio/modules',
    'lib/x86_64-linux-gnu/gdk-pixbuf-2.0',
    'lib/x86_64-linux-gnu/girepository-1.0',
    'share/glib-2.0/schemas',
]
for name in runtime_trees:
    relative = Path(name)
    for parent in reversed(relative.parents):
        destination = output / parent
        if destination.is_symlink():
            destination.unlink()
            destination.mkdir()
            merge(parent)
    destination = output / relative
    if destination.is_symlink(): destination.unlink()
    elif destination.exists(): shutil.rmtree(destination)
    for source in [system / relative, sdk / relative]:
        if source.is_dir():
            shutil.copytree(source, destination, dirs_exist_ok=True, ignore_dangling_symlinks=True)
for source in sdk.glob('**/pkgconfig/*.pc'):
    destination = pkgconfig / source.name
    content = source.read_text().replace(str(sdk), '/usr')
    if not destination.exists() or destination.read_text() != content:
        destination.write_text(content)
PY
export PKG_CONFIG_PATH="$atlas_pkgconfig"
export LD_LIBRARY_PATH=/usr/lib/x86_64-linux-gnu
export LIBRARY_PATH=/usr/lib/x86_64-linux-gnu
export XDG_CACHE_HOME=/workspace/atlas-build-cache
# linuxdeploy cannot map namespace symlinks back to dpkg ownership, so retain
# the combined SDK/system notices explicitly in this cloud build.
export ATLAS_SDK_DOCS=/usr/share/doc
bwrap --ro-bind / / --bind /workspace /workspace --bind /tmp /tmp \
  --ro-bind /usr /tmp/atlas-original-usr --ro-bind "$atlas_union" /usr \
  --dev /dev --proc /proc "$@"
