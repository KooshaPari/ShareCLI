#!/usr/bin/env bash
# Local parity for C04 L31 dual secret scanners (gitleaks + trufflehog).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

run_gitleaks() {
  if command -v gitleaks >/dev/null 2>&1; then
    gitleaks detect --source . --verbose --redact --config .gitleaks.toml
    return
  fi
  if command -v docker >/dev/null 2>&1; then
    docker run --rm -v "$ROOT:/repo" -w /repo \
      ghcr.io/gitleaks/gitleaks:v8.22.1 \
      detect --source . --verbose --redact --config .gitleaks.toml
    return
  fi
  echo "gitleaks not found (install via brew or use docker)" >&2
  exit 1
}

run_trufflehog() {
  if command -v trufflehog >/dev/null 2>&1; then
    trufflehog filesystem . --fail --only-verified
    return
  fi
  if command -v docker >/dev/null 2>&1; then
    docker run --rm -v "$ROOT:/repo" -w /repo \
      trufflesecurity/trufflehog:3.93.6 \
      filesystem /repo --fail --only-verified
    return
  fi
  echo "trufflehog not found (install via brew or use docker)" >&2
  exit 1
}

echo ">> gitleaks"
run_gitleaks
echo ">> trufflehog (verified only)"
run_trufflehog
echo ">> secret scan OK"
