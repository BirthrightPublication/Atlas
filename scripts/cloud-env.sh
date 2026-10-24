#!/usr/bin/env bash
# Source this only for the prepared Codex/Linux userspace SDK.
# Standard developer installations do not need these overrides.
export CARGO_HOME=/workspace/.cargo
export RUSTUP_HOME=/workspace/.rustup
export PATH="/workspace/.cargo/bin:/workspace/atlas-sysroot/usr/bin:$PATH"
export PKG_CONFIG_PATH="/workspace/atlas-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:/workspace/atlas-sysroot/usr/share/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export LIBRARY_PATH="/workspace/atlas-sysroot/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
export LD_LIBRARY_PATH="/workspace/atlas-sysroot/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export C_INCLUDE_PATH="/workspace/atlas-sysroot/usr/include:/workspace/atlas-sysroot/usr/include/x86_64-linux-gnu${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"
export CPLUS_INCLUDE_PATH="$C_INCLUDE_PATH${CPLUS_INCLUDE_PATH:+:$CPLUS_INCLUDE_PATH}"
export CARGO_BUILD_JOBS=3
export XDG_CACHE_HOME=/workspace/atlas-build-cache
