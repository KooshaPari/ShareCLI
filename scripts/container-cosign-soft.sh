#!/usr/bin/env bash
# C06 L56 — soft container cosign gate (no GHCR required).
# Builds sharecli:ci when podman is available; keyless sign-blob on main only.
# podman: this repo does not use the Docker engine
# Hard GHCR path: scripts/container-cosign-hard.sh + .github/workflows/container-cosign.yml
set -euo pipefail

IMAGE_TAG="${IMAGE_TAG:-sharecli:ci}"
BLOB_FILE="${BLOB_FILE:-sharecli-ci-image-id.txt}"
BUNDLE_FILE="${BUNDLE_FILE:-sharecli-ci-image-id.txt.sigstore.json}"
IDENTITY_REGEXP="${COSIGN_IDENTITY_REGEXP:-https://github.com/KooshaPari/sharecli/.*}"
OIDC_ISSUER="${COSIGN_OIDC_ISSUER:-https://token.actions.githubusercontent.com}"

if [[ ! -f Containerfile ]]; then
  echo "::notice::Containerfile missing — skip container cosign soft gate"
  exit 0
fi

echo "Containerfile present — container cosign soft gate"
cosign version

if ! command -v podman >/dev/null 2>&1; then
  echo "::notice::podman CLI unavailable — cosign version + Containerfile gate only"
  exit 0
fi

if ! podman info >/dev/null 2>&1; then
  echo "::notice::podman storage unavailable — cosign version + Containerfile gate only"
  exit 0
fi

echo "Building ${IMAGE_TAG} from Containerfile"
# podman: this repo does not use the Docker engine
# --format docker keeps the HEALTHCHECK that the Containerfile declares; the
# default OCI output format drops it.
podman build --format docker -f Containerfile -t "${IMAGE_TAG}" .

IMAGE_ID="$(podman inspect --format='{{.Id}}' "${IMAGE_TAG}")"
printf '%s\n' "${IMAGE_ID}" >"${BLOB_FILE}"
echo "Image ID: ${IMAGE_ID}"

# GHCR keyless image sign — only when publish is explicitly enabled (not wired by default).
if [[ "${GHCR_COSIGN_PUSH:-false}" == "true" ]]; then
  GHCR_IMAGE="${GHCR_IMAGE:-ghcr.io/${GITHUB_REPOSITORY,,}/sharecli:ci}"
  echo "GHCR_COSIGN_PUSH=true — tagging and pushing ${GHCR_IMAGE}"
  if [[ -n "${GITHUB_TOKEN:-}" ]]; then
    # podman: this repo does not use the Docker engine
    # cosign reads the Docker keychain, not the podman auth file, so write the
    # login in Docker-compatible form to the path cosign resolves.
    DOCKER_AUTH_DIR="${DOCKER_CONFIG:-${HOME}/.docker}"
    mkdir -p "${DOCKER_AUTH_DIR}"
    printf '%s\n' "${GITHUB_TOKEN}" \
      | podman login "${GHCR_IMAGE%%/*}" \
        --username "${GITHUB_ACTOR:-github-actions}" \
        --password-stdin \
        --compat-auth-file "${DOCKER_AUTH_DIR}/config.json"
  else
    echo "::error::GHCR_COSIGN_PUSH=true requires GITHUB_TOKEN to push/sign ${GHCR_IMAGE}" >&2
    exit 1
  fi
  podman tag "${IMAGE_TAG}" "${GHCR_IMAGE}"
  podman push --authfile "${DOCKER_AUTH_DIR}/config.json" "${GHCR_IMAGE}"
  # podman: this repo does not use the Docker engine
  # --certificate-identity-regexp / --certificate-oidc-issuer are verify-time
  # flags only; `cosign sign` rejects them.
  cosign sign --yes "${GHCR_IMAGE}"
  cosign verify "${GHCR_IMAGE}" \
    --certificate-identity-regexp "${IDENTITY_REGEXP}" \
    --certificate-oidc-issuer "${OIDC_ISSUER}"
  exit 0
fi

# Soft path: keyless sign-blob of the local image ID on main (no registry secrets).
if [[ "${GITHUB_ACTIONS:-}" == "true" && "${GITHUB_REF_NAME:-}" == "main" ]]; then
  echo "Keyless cosign sign-blob (main)"
  cosign sign-blob --yes "${BLOB_FILE}" \
    --bundle "${BUNDLE_FILE}" \
    --output-signature "${BLOB_FILE}.sig" \
    --output-certificate "${BLOB_FILE}.cert"
  cosign verify-blob "${BLOB_FILE}" \
    --bundle "${BUNDLE_FILE}" \
    --certificate-identity-regexp "${IDENTITY_REGEXP}" \
    --certificate-oidc-issuer "${OIDC_ISSUER}"
  exit 0
fi

echo "::notice::Non-main or local run — build + digest only (no keyless sign-blob)"
echo "Dry-run verify (after main CI produces a bundle):"
echo "  cosign verify-blob ${BLOB_FILE} \\"
echo "    --bundle ${BUNDLE_FILE} \\"
echo "    --certificate-identity-regexp '${IDENTITY_REGEXP}' \\"
echo "    --certificate-oidc-issuer '${OIDC_ISSUER}'"
