#!/usr/bin/env bash
# build.sh — build the sharecli desktop client (macOS) and bundle it as an .app.
#
# Usage:
#   ./desktop/build.sh                      # debug build, no install
#   ./desktop/build.sh --release            # release build, no install
#   ./desktop/build.sh --release --install            # release + install to ~/Applications
#   ./desktop/build.sh --release --system --install   # release + install to /Applications (sudo)
#
# Prerequisites (macOS):
#   - Rust toolchain (rustup)
#   - Xcode Command Line Tools (swift, swiftc)
#   - iconutil (Apple) for .icns generation
#   - The sharecli repo root is the working directory

set -euo pipefail

RELEASE=""
DO_INSTALL=0
SYSTEM_INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --release) RELEASE="--release" ;;
    --install) DO_INSTALL=1 ;;
    --system)  SYSTEM_INSTALL=1 ;;
    *) echo "Unknown arg: $arg" >&2; exit 2 ;;
  esac
done

PROFILE="debug"
CARGO_FLAGS=()
if [[ -n "$RELEASE" ]]; then
    PROFILE="release"
    CARGO_FLAGS+=(--release)
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$REPO_ROOT/target/$PROFILE"

# Build macOS .icns from the Backbone-2 iconset so cargo bundle / swift build
# can pick it up. iconutil is a macOS-only tool; skip on other platforms.
if [[ "$(uname -s)" == "Darwin" ]]; then
    ICONSET="$REPO_ROOT/assets/icons/sharecli.iconset"
    ICNS="$REPO_ROOT/assets/icons/sharecli.icns"
    if [[ -d "$ICONSET" ]]; then
        echo "==> Building .icns from .iconset"
        iconutil -c icns "$ICONSET" -o "$ICNS"
    fi
fi

echo "==> Building Rust crates (profile: $PROFILE)"
cd "$REPO_ROOT"
cargo build ${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"} -p sharecli-ipc -p sharecli-ffi

IPC_BIN="$TARGET_DIR/sharecli-ipc"
FFI_LIB="$TARGET_DIR/libsharecli_ffi.dylib"

echo "    sharecli-ipc → $IPC_BIN"
echo "    sharecli-ffi → $FFI_LIB"

echo ""
echo "==> Building Swift tray app"
cd "$REPO_ROOT/desktop/ShareCLITray"

# Linker needs to find the Rust dylib at build time.
export SHARECLI_FFI_LIB_DIR="$TARGET_DIR"

swift build \
    -c "$PROFILE" \
    -Xlinker "-L$TARGET_DIR" \
    -Xlinker "-lsharecli_ffi" \
    -Xlinker "-rpath" \
    -Xlinker "@executable_path/../Frameworks"

SWIFT_BIN=".build/$PROFILE/ShareCLITray"
echo "    Swift tray → $SWIFT_BIN"
echo ""
echo "==> Build complete."
echo "    Tray binary: $(pwd)/$SWIFT_BIN"
echo "    FFI dylib:   $FFI_LIB"
echo "    IPC binary:  $IPC_BIN"

if [[ "$DO_INSTALL" -eq 1 ]]; then
    echo ""
    echo "==> Bundling .app and installing"
    INSTALL_ARGS=()
    [[ "$SYSTEM_INSTALL" -eq 1 ]] && INSTALL_ARGS+=(--system)
    "$REPO_ROOT/scripts/install-tray-macos.sh" "${INSTALL_ARGS[@]}"
fi
