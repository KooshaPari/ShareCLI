#!/bin/sh
# install.sh — POSIX sh compatible installer for sharecli
# Usage: curl -fsSL https://raw.githubusercontent.com/KooshaPari/sharecli/main/install.sh | sh
set -eu

REPO="KooshaPari/sharecli"
BINARY="sharecli"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# --- Helpers ---------------------------------------------------------------

info()  { printf '=> %s\n' "$1"; }
error() { printf '!! %s\n' "$1" >&2; exit 1; }

need() {
  command -v "$1" >/dev/null 2>&1 || error "required command not found: $1"
}

# --- Detect OS and architecture --------------------------------------------

detect_platform() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux*)  OS="linux" ;;
    Darwin*) OS="darwin" ;;
    *)       error "unsupported OS: $os (only Linux and macOS are supported)" ;;
  esac

  case "$arch" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)  ARCH="aarch64" ;;
    *)              error "unsupported architecture: $arch" ;;
  esac

  # Map to Rust target triples
  if [ "$OS" = "linux" ]; then
    TARGET="x86_64-unknown-linux-gnu"
  elif [ "$OS" = "darwin" ]; then
    TARGET="aarch64-apple-darwin"
  fi
}

# --- Resolve latest version from GitHub API --------------------------------

resolve_version() {
  if [ -n "${VERSION:-}" ]; then
    info "Using specified version: $VERSION"
    return
  fi

  need curl

  info "Resolving latest release version..."
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"v\?\([^"]*\)".*/\1/')

  if [ -z "$VERSION" ]; then
    error "could not determine latest release version"
  fi
  info "Latest version: $VERSION"
}

# --- Download and install --------------------------------------------------

install_binary() {
  need curl
  need uname

  ARCHIVE="sharecli-${TARGET}.tar.gz"
  URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARCHIVE}"

  info "Downloading $URL"
  TMPDIR="${TMPDIR:-/tmp}/sharecli-install-$$"
  mkdir -p "$TMPDIR"
  trap 'rm -rf "$TMPDIR"' EXIT

  if ! curl -fsSL -o "$TMPDIR/$ARCHIVE" "$URL"; then
    error "download failed — check that v${VERSION} has a release asset for ${TARGET}"
  fi

  info "Extracting..."
  tar -xzf "$TMPDIR/$ARCHIVE" -C "$TMPDIR"

  # Find the binary (may be nested in a directory)
  BIN_PATH=$(find "$TMPDIR" -name "$BINARY" -type f -not -name '*.sha256' | head -1)
  if [ -z "$BIN_PATH" ]; then
    error "binary '$BINARY' not found in archive"
  fi
  chmod +x "$BIN_PATH"

  # Install
  if [ -w "$INSTALL_DIR" ]; then
    cp "$BIN_PATH" "$INSTALL_DIR/$BINARY"
  else
    info "Using sudo to install to $INSTALL_DIR"
    sudo cp "$BIN_PATH" "$INSTALL_DIR/$BINARY"
  fi
  chmod 555 "$INSTALL_DIR/$BINARY"

  info "Installed $BINARY to $INSTALL_DIR/$BINARY"
  info "Run: $BINARY --help"
}

# --- Main ------------------------------------------------------------------

main() {
  need uname
  need curl
  need tar

  detect_platform
  resolve_version
  install_binary
}

main "$@"
