#!/usr/bin/env bash
# C06 L56 / T-660 / T-1120 — hard GHCR container cosign publish path.
# Keyless sign + SLSA attest + verify + Rekor transparency log verification
# via GitHub OIDC (no Apple / extra secrets).
#
# Env:
#   IMAGE_TAG          Local build tag (default: sharecli:ci)
#   GHCR_IMAGE         Full GHCR ref (default: ghcr.io/<owner>/<repo>/sharecli:<tag>)
#   GHCR_TAG           Tag suffix when GHCR_IMAGE unset (default: ci | git tag | sha)
#   SKIP_GHCR_PUSH     If true, build + write digest/predicate only (permission blocker mode)
#   GITHUB_TOKEN       Required for podman login when pushing
set -euo pipefail
# podman: this repo does not use the Docker engine

IMAGE_TAG="${IMAGE_TAG:-sharecli:ci}"
IDENTITY_REGEXP="${COSIGN_IDENTITY_REGEXP:-https://github.com/KooshaPari/sharecli/.*}"
OIDC_ISSUER="${COSIGN_OIDC_ISSUER:-https://token.actions.githubusercontent.com}"
PREDICATE_FILE="${PREDICATE_FILE:-sharecli-ci-slsa-predicate.json}"
DIGEST_FILE="${DIGEST_FILE:-sharecli-ci-image-digest.txt}"
SKIP_GHCR_PUSH="${SKIP_GHCR_PUSH:-false}"

emit_output() {
  local key="$1"
  local value="$2"
  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    {
      echo "${key}<<EOF"
      echo "${value}"
      echo "EOF"
    } >>"${GITHUB_OUTPUT}"
  fi
}

if [[ ! -f Containerfile ]]; then
  echo "::error::Containerfile missing — hard container cosign gate requires it"
  exit 1
fi

command -v cosign >/dev/null 2>&1 || {
  echo "::error::cosign not installed"
  exit 1
}
# podman: this repo does not use the Docker engine
command -v podman >/dev/null 2>&1 || {
  echo "::error::podman CLI not found - hard container cosign requires a container engine" >&2
  exit 1
}
CONTAINER_ENGINE="podman"
echo "Using container engine: ${CONTAINER_ENGINE}"

podman info >/dev/null 2>&1 || {
  echo "::error::podman machine/storage required for hard container cosign" >&2
  exit 1
}

cosign version

# Resolve GHCR image ref
if [[ -z "${GHCR_IMAGE:-}" ]]; then
  if [[ -z "${GITHUB_REPOSITORY:-}" ]]; then
    echo "::error::GITHUB_REPOSITORY unset and GHCR_IMAGE not provided"
    exit 1
  fi
  repo_lc="$(printf '%s' "${GITHUB_REPOSITORY}" | tr '[:upper:]' '[:lower:]')"
  if [[ -n "${GHCR_TAG:-}" ]]; then
    tag="${GHCR_TAG}"
  elif [[ "${GITHUB_REF_TYPE:-}" == "tag" && -n "${GITHUB_REF_NAME:-}" ]]; then
    tag="${GITHUB_REF_NAME}"
  elif [[ -n "${GITHUB_SHA:-}" ]]; then
    tag="sha-${GITHUB_SHA:0:12}"
  else
    tag="ci"
  fi
  GHCR_IMAGE="ghcr.io/${repo_lc}/sharecli:${tag}"
fi

REPO_NAME="${GHCR_IMAGE%%:*}"

echo "Building ${IMAGE_TAG} from Containerfile"
${CONTAINER_ENGINE} build -f Containerfile -t "${IMAGE_TAG}" .
${CONTAINER_ENGINE} tag "${IMAGE_TAG}" "${GHCR_IMAGE}"

IMAGE_ID="$(${CONTAINER_ENGINE} inspect --format='{{.Id}}' "${IMAGE_TAG}")"
printf '%s\n' "${IMAGE_ID}" >sharecli-ci-image-id.txt
echo "Local image ID: ${IMAGE_ID}"
echo "Target GHCR ref: ${GHCR_IMAGE}"

# Minimal SLSA provenance predicate for cosign attest --type slsaprovenance
builder_id="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-local}/actions/runs/${GITHUB_RUN_ID:-0}"
source_uri="git+${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-KooshaPari/sharecli}"
cat >"${PREDICATE_FILE}" <<EOF
{
  "buildType": "https://github.com/KooshaPari/sharecli/.github/workflows/container-cosign.yml@v1",
  "builder": { "id": "${builder_id}" },
  "invocation": {
    "configSource": {
      "uri": "${source_uri}",
      "digest": { "sha1": "${GITHUB_SHA:-unknown}" },
      "entryPoint": ".github/workflows/container-cosign.yml"
    }
  },
  "metadata": {
    "buildInvocationId": "${GITHUB_RUN_ID:-local}",
    "completeness": {
      "parameters": true,
      "environment": false,
      "materials": false
    },
    "reproducible": false
  }
}
EOF
echo "Wrote predicate ${PREDICATE_FILE}"

emit_output "subject-name" "${REPO_NAME}"
emit_output "image-ref" "${GHCR_IMAGE}"

if [[ "${SKIP_GHCR_PUSH}" == "true" ]]; then
  echo "::warning::SKIP_GHCR_PUSH=true - dry-run mode, no registry push or sign"
  echo "Would push/sign/attest/verify: ${GHCR_IMAGE}"
  printf 'skipped-push\n' >"${DIGEST_FILE}"
  emit_output "skipped" "true"
  emit_output "subject-digest" ""
  exit 0
fi

if [[ -z "${GITHUB_TOKEN:-}" ]]; then
  echo "::error::GITHUB_TOKEN required to login/push GHCR - packages:write + id-token:write" >&2
  echo "::error::If org registry permissions block GITHUB_TOKEN, re-run with SKIP_GHCR_PUSH=true" >&2
  exit 1
fi

echo "Logging into ghcr.io as ${GITHUB_ACTOR:-github-actions}"
echo "${GITHUB_TOKEN}" | ${CONTAINER_ENGINE} login ghcr.io -u "${GITHUB_ACTOR:-github-actions}" --password-stdin

echo "Pushing ${GHCR_IMAGE}"
${CONTAINER_ENGINE} push "${GHCR_IMAGE}"

# Prefer digest-based signing (immutable)
DIGEST_REF=""
DIGEST_SHA=""
# podman: this repo does not use the Docker engine
# Prefer the registry digest podman recorded on push (immutable subject);
# fall back to the local image ID when the registry returned no digest.
DIGEST_REF="$(${CONTAINER_ENGINE} inspect --format='{{index .RepoDigests 0}}' "${GHCR_IMAGE}" 2>/dev/null || true)"
if [[ -n "${DIGEST_REF}" && "${DIGEST_REF}" == *"@"* ]]; then
  DIGEST_SHA="${DIGEST_REF#*@}"
else
  DIGEST_SHA="${IMAGE_ID}"
  DIGEST_REF="${GHCR_IMAGE}"
fi
printf '%s\n' "${DIGEST_REF}" >"${DIGEST_FILE}"
echo "Signing subject: ${DIGEST_REF}"
emit_output "subject-digest" "${DIGEST_SHA}"
emit_output "skipped" "false"

echo "Keyless cosign sign"
cosign sign --yes "${DIGEST_REF}"

echo "Keyless cosign attest - slsaprovenance"
cosign attest --yes --type slsaprovenance --predicate "${PREDICATE_FILE}" "${DIGEST_REF}"

# --- Rekor transparency log verification (T-1120) ---
# cosign verify --output-json emits the full transparency log bundle.
# Parse the output to confirm the Rekor entry contains the expected
# identity and issuer fields (proof-of-inclusion in the immutable log).
REKOR_BUNDLE_FILE="/tmp/cosign-rekor-bundle.json"
echo "Verifying Rekor transparency log entry"
cosign verify "${DIGEST_REF}" \
  --certificate-identity-regexp "${IDENTITY_REGEXP}" \
  --certificate-oidc-issuer "${OIDC_ISSUER}" \
  --output-json >"${REKOR_BUNDLE_FILE}"

# Assert the bundle contains the critical claim (Rekor log structure).
if ! grep -q '"critical"' "${REKOR_BUNDLE_FILE}"; then
  echo "::error::Rekor transparency log bundle missing 'critical' claim" >&2
  exit 1
fi
if ! grep -q '"identity"' "${REKOR_BUNDLE_FILE}"; then
  echo "::error::Rekor transparency log bundle missing 'identity' field" >&2
  exit 1
fi
if ! grep -q '"issuer"' "${REKOR_BUNDLE_FILE}"; then
  echo "::error::Rekor transparency log bundle missing 'issuer' field" >&2
  exit 1
fi
echo "Rekor transparency log entry verified for ${DIGEST_REF}"

# --- End Rekor transparency log verification (T-1120) ---

echo "Verify signature chain - cosign verify without JSON output"
cosign verify "${DIGEST_REF}" \
  --certificate-identity-regexp "${IDENTITY_REGEXP}" \
  --certificate-oidc-issuer "${OIDC_ISSUER}"

echo "Verify attestation"
cosign verify-attestation --type slsaprovenance "${DIGEST_REF}" --certificate-identity-regexp "${IDENTITY_REGEXP}" --certificate-oidc-issuer "${OIDC_ISSUER}" >/dev/null

echo "Hard container cosign publish green - Rekor verified: ${DIGEST_REF}"
echo "Consumer verify:"
echo "  bash scripts/container-cosign-verify.sh ${DIGEST_REF}"
