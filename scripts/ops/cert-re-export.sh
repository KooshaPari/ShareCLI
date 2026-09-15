#!/usr/bin/env bash
# cert-re-export.sh
# Re-export a .p12 file with a fresh passphrase so its password is known.
#
# Usage:
#   ./scripts/ops/cert-re-export.sh <input.p12> <original-password> <new-password> <output.p12>
#
# Requires: openssl (should be available on macOS runners)

set -euo pipefail

INPUT_P12="$1"
OLD_PASS="$2"
NEW_PASS="$3"
OUTPUT_P12="$4"

CERT_PEM=$(mktemp /tmp/cert-XXXXXX.pem)
KEY_PEM=$(mktemp /tmp/key-XXXXXX.pem)
trap 'rm -f "$CERT_PEM" "$KEY_PEM"' EXIT

echo "==> Extracting cert from $INPUT_P12"
openssl pkcs12 -in "$INPUT_P12" -passin pass:"$OLD_PASS" \
  -clcerts -nokeys -out "$CERT_PEM" -nodes

echo "==> Extracting key from $INPUT_P12"
openssl pkcs12 -in "$INPUT_P12" -passin pass:"$OLD_PASS" \
  -nocerts -nodes -out "$KEY_PEM"

echo "==> Re-exporting to $OUTPUT_P12 with fresh passphrase"
openssl pkcs12 -export \
  -in "$CERT_PEM" -inkey "$KEY_PEM" \
  -out "$OUTPUT_P12" \
  -passout pass:"$NEW_PASS"

echo "==> Verifying new .p12"
openssl pkcs12 -in "$OUTPUT_P12" -passin pass:"$NEW_PASS" -clcerts -nokeys > /dev/null

echo "==> Done. Output: $OUTPUT_P12"
