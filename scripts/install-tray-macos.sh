#!/usr/bin/env bash
# Install ShareCLITray.app with bundled FFI dylib and IPC sidecar.
#
# Usage:
#   ./scripts/install-tray-macos.sh               # install to ~/Applications (user-scope, no sudo)
#   ./scripts/install-tray-macos.sh --system      # install to /Applications (requires sudo)
#
# Prerequisite: build artifacts present (run `./desktop/build.sh --release` first).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE=release
TARGET="$REPO_ROOT/target/$PROFILE"
TRAY_PKG="$REPO_ROOT/desktop/ShareCLITray"
BIN_DIR="$(cd "$TRAY_PKG" && swift build -c release --show-bin-path)"
TRAY_BIN="$BIN_DIR/ShareCLITray"
FFI_DYLIB="$TARGET/libsharecli_ffi.dylib"
IPC_BIN="$TARGET/sharecli-ipc"
APP_NAME="ShareCLITray.app"
APP_PATH=""

SYSTEM_INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --system) SYSTEM_INSTALL=1 ;;
    --user)   SYSTEM_INSTALL=0 ;;
    *) echo "Unknown arg: $arg" >&2; exit 2 ;;
  esac
done

if [[ "$SYSTEM_INSTALL" -eq 1 ]]; then
  INSTALL_DIR="/Applications"
  APP_PATH="$INSTALL_DIR/$APP_NAME"
  if [[ ! -w "$INSTALL_DIR" ]]; then
    echo "Re-running with sudo to write $APP_PATH"
    exec sudo "$0" --system "$@"
  fi
else
  INSTALL_DIR="${HOME}/Applications"
  APP_PATH="$INSTALL_DIR/$APP_NAME"
fi

for f in "$TRAY_BIN" "$FFI_DYLIB" "$IPC_BIN"; do
  if [[ ! -f "$f" ]]; then
    echo "Missing build artifact: $f (run './desktop/build.sh --release' first)" >&2
    exit 1
  fi
done

ICNS="$REPO_ROOT/assets/icons/sharecli.icns"
if [[ -d "$REPO_ROOT/assets/icons/sharecli.iconset" && ! -f "$ICNS" ]]; then
  iconutil -c icns "$REPO_ROOT/assets/icons/sharecli.iconset" -o "$ICNS" 2>/dev/null || true
fi

rm -rf "$APP_PATH"
mkdir -p "$APP_PATH/Contents/MacOS" "$APP_PATH/Contents/Frameworks" "$APP_PATH/Contents/Resources/bin"

cat > "$APP_PATH/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>ShareCLITray</string>
  <key>CFBundleIdentifier</key>
  <string>phenotype.sharecli.tray</string>
  <key>CFBundleName</key>
  <string>ShareCLI</string>
  <key>CFBundleDisplayName</key>
  <string>ShareCLI</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.3.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

if [[ -f "$ICNS" ]]; then
  cp "$ICNS" "$APP_PATH/Contents/Resources/AppIcon.icns"
  /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string AppIcon" "$APP_PATH/Contents/Info.plist" 2>/dev/null \
    || /usr/libexec/PlistBuddy -c "Set :CFBundleIconFile AppIcon" "$APP_PATH/Contents/Info.plist"
fi

cp "$TRAY_BIN" "$APP_PATH/Contents/MacOS/ShareCLITray"
cp "$FFI_DYLIB" "$APP_PATH/Contents/Frameworks/"
cp "$IPC_BIN" "$APP_PATH/Contents/Resources/bin/sharecli-ipc"
chmod +x "$APP_PATH/Contents/MacOS/ShareCLITray" "$APP_PATH/Contents/Resources/bin/sharecli-ipc"

# P4-16: Bundle Sparkle.framework so the auto-updater can resolve at runtime.
#
# SwiftPM's layout differs between toolchains, so probe the known locations
# instead of assuming one. This previously looked only at
# "$BIN_DIR/ExecutableModules/Sparkle.framework", which does not exist on this
# toolchain (the framework sits directly at "$BIN_DIR/Sparkle.framework"), and
# the copy was skipped *silently*. The result was an installed .app that died on
# launch with:
#     dyld: Library not loaded: @rpath/Sparkle.framework/Versions/B/Sparkle
# A framework the binary links against is therefore a hard error when missing,
# not a silent skip.
SPARKLE_FW=""
for candidate in \
    "$BIN_DIR/Sparkle.framework" \
    "$BIN_DIR/ExecutableModules/Sparkle.framework" \
    "$BIN_DIR/PackageFrameworks/Sparkle.framework"; do
    if [[ -d "$candidate" ]]; then
        SPARKLE_FW="$candidate"
        break
    fi
done

if [[ -n "$SPARKLE_FW" ]]; then
    rm -rf "$APP_PATH/Contents/Frameworks/Sparkle.framework"
    cp -R "$SPARKLE_FW" "$APP_PATH/Contents/Frameworks/"
    chmod -R +w "$APP_PATH/Contents/Frameworks/Sparkle.framework"
elif otool -L "$TRAY_BIN" 2>/dev/null | grep -q 'Sparkle\.framework'; then
    echo "ERROR: the tray binary links against Sparkle.framework, but the" >&2
    echo "       framework was not found next to the build product. Installing" >&2
    echo "       anyway would ship an .app that crashes on launch with" >&2
    echo "       'Library not loaded: @rpath/Sparkle.framework/Versions/B/Sparkle'." >&2
    echo "       Searched:" >&2
    echo "         $BIN_DIR/Sparkle.framework" >&2
    echo "         $BIN_DIR/ExecutableModules/Sparkle.framework" >&2
    echo "         $BIN_DIR/PackageFrameworks/Sparkle.framework" >&2
    echo "       Run 'swift package resolve && swift build -c release' in" >&2
    echo "       $TRAY_PKG and retry." >&2
    exit 1
fi

# t-70: Generate a per-channel appcast feed and copy it into the .app's
# Resources so the bundled Sparkle updater can resolve an offline feed
# for QA / smoke. Release tagging (CI / release.yml) is responsible
# for promoting these stubs to signed, delta-aware feeds; this step
# stays best-effort so a local install never fails on a missing toolchain.
APPCAST_SCRIPT="$REPO_ROOT/scripts/build-appcast.sh"
if [[ -x "$APPCAST_SCRIPT" ]]; then
    echo "==> Generating appcast feeds"
    if ! SHARECLI_TRAY_APP="$APP_PATH" "$APPCAST_SCRIPT" --install \
            --prefix "${SHARECLI_DOWNLOAD_PREFIX:-https://sharecli.example/downloads}" \
            2>/dev/null; then
        echo "    (skipped: appcast generation failed; continuing install)" >&2
    fi
fi

install_name_tool -change "@rpath/libsharecli_ffi.dylib" "@executable_path/../Frameworks/libsharecli_ffi.dylib" \
  "$APP_PATH/Contents/MacOS/ShareCLITray" 2>/dev/null || true

# SwiftPM linked the tray binary against libsharecli_ffi.dylib with an
# absolute build-tree path baked into LC_LOAD_DYLIB (because we passed
# -Ltarget_dir + -lbasename — dyld records the resolved absolute path).
# Rewrite that absolute path back to the relative @rpath form so the .app is
# portable and the bundled dylib in Contents/Frameworks resolves correctly.
install_name_tool -change \
  "$REPO_ROOT/target/release/libsharecli_ffi.dylib" \
  "@rpath/libsharecli_ffi.dylib" \
  "$APP_PATH/Contents/MacOS/ShareCLITray" 2>/dev/null || true
# Also handle the deps/ path that cargo actually emits:
install_name_tool -change \
  "$REPO_ROOT/target/release/deps/libsharecli_ffi.dylib" \
  "@rpath/libsharecli_ffi.dylib" \
  "$APP_PATH/Contents/MacOS/ShareCLITray" 2>/dev/null || true

# Rewrite the bundled dylib's own ID so it no longer points at the build tree
# (prevents dyld warnings when launching from /Applications where the build
# tree no longer exists or is inaccessible).
install_name_tool -id "@rpath/libsharecli_ffi.dylib" \
  "$APP_PATH/Contents/Frameworks/libsharecli_ffi.dylib" 2>/dev/null || true

# Code-sign ad-hoc so launchd / Gatekeeper accept on first launch (developer-id
# signing is preferred for distribution; ad-hoc is enough for local use).
codesign --force --deep --sign - "$APP_PATH" 2>/dev/null || true

echo "Installed: $APP_PATH"
if [[ "$SYSTEM_INSTALL" -eq 1 ]]; then
  echo "  (system-wide; visible in Launchpad, Spotlight)"
else
  echo "  (user-scope; visible in ~/Applications). For /Applications, rerun with --system."
fi
