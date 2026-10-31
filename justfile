# sharecli — task runner
# Spec: https://github.com/casey/just
# Use `just` (no args) to list available recipes.

set shell := ["bash", "-uc"]
set dotenv-load := true
set positional-arguments := true

# -------- project metadata --------
app      := "sharecli"
registry := env_var_or_default("CARGO_REGISTRY", "crates-io")

# -------- default recipe (lists everything) --------
default:
    @just --list --unsorted

# -------- setup --------
[group: 'setup']
install-tools:
    @echo ">> installing cargo extensions (deny, audit, tarpaulin, typos, nextest)"
    @command -v cargo-deny    >/dev/null 2>&1 || cargo install --locked cargo-deny
    @command -v cargo-audit   >/dev/null 2>&1 || cargo install --locked cargo-audit
    @command -v cargo-llvm-cov >/dev/null 2>&1 || cargo install --locked cargo-llvm-cov
    @command -v typos         >/dev/null 2>&1 || cargo install --locked typos-cli
    @command -v cargo-nextest >/dev/null 2>&1 || cargo install --locked cargo-nextest
    @echo ">> tools ready"

# -------- formatting --------
[group: 'fmt']
fmt:
    @echo ">> cargo fmt"
    @cargo fmt --all

[group: 'fmt']
fmt-check:
    @echo ">> cargo fmt --check"
    @cargo fmt --all -- --check

# -------- linting --------
[group: 'lint']
lint:
    @echo ">> cargo clippy (all targets, all features, deny warnings)"
    @cargo clippy --all-targets --all-features --locked -- -D warnings

[group: 'lint']
lint-pedantic:
    @echo ">> cargo clippy pedantic"
    @cargo clippy --all-targets --all-features --locked -- -W clippy::pedantic -D warnings

[group: 'lint']
vale:
    @echo ">> vale inclusive-language (C09 L81.10)"
    @./scripts/lint/vale.sh

[group: 'lint']
secret-scan:
    @echo ">> gitleaks + trufflehog (C04 L31)"
    @bash scripts/ci/secret_scan.sh

# -------- build --------
[group: 'build']
build:
    @echo ">> cargo build (debug)"
    @cargo build --locked --all-features

[group: 'build']
build-release:
    @echo ">> cargo build --release"
    @cargo build --release --locked --all-features

# -------- MVP finality / OS parity (docs/deploy/FINALITY.md) --------
[group: 'parity']
build-cli: build-release
    @echo ">> CLI release binary ready (lane A / GA)"

[group: 'parity']
[windows]
build-cli-windows:
    @echo ">> Windows CLI native release (lane A / GA)"
    @cargo build --release --locked --bin sharecli

[group: 'parity']
[unix]
build-cli-windows:
    @echo ">> Windows CLI cross-compile (optional; CI uses windows-latest)"
    @echo "    rustup target add x86_64-pc-windows-msvc  # plus appropriate linker"
    -cargo build --release --locked --bin sharecli --target x86_64-pc-windows-msvc

[group: 'parity']
build-tray-linux:
    @echo ">> Linux StatusNotifier tray (lane B / beta)"
    @cargo build -p sharecli-tray-linux --release --locked

[group: 'parity']
build-tray-macos:
    @echo ">> macOS Swift tray + desktop (lane B+C / beta)"
    @cargo build -p sharecli-ffi --release --locked
    @cargo build -p sharecli-ipc --release --locked
    @cd desktop/ShareCLITray && swift build -c release \
        -Xlinker -L -Xlinker ../../target/release \
        -Xlinker -lsharecli_ffi \
        -Xlinker -rpath -Xlinker @executable_path/../Frameworks

install-tray-macos: build-tray-macos
    @./scripts/install-tray-macos.sh

[group: 'parity']
build-desktop-macos: build-tray-macos
    @echo ">> alias: ShareCLITray includes DashboardView"

[group: 'parity']
build-tray-windows:
    @echo ">> Windows WinUI tray (lane B / beta)"
    @dotnet build windows/ShareCLITray/ShareCLITray.csproj -c Release

[group: 'parity']
wsl-parity-check:
    @echo ">> WSL parity checklist (see docs/deploy/FINALITY.md)"
    @grep -q 'WSL bridge' docs/deploy/FINALITY.md
    @grep -q '127.0.0.1:9000' docs/deploy/FINALITY.md
    @grep -q 'WSL (CLI in WSL' README.md
    @grep -q 'x86_64-pc-windows-msvc' .github/workflows/release.yml
    @grep -q 'tray-macos' .github/workflows/release.yml
    @echo "1) WSL: sharecli serve --bind 0.0.0.0:9000"
    @echo "2) Windows: curl http://127.0.0.1:9000/healthz  OR  ShareCLITray"
    @echo "3) Optional WSLg: sharecli-tray on Linux session"
    @echo ">> wsl-parity-check OK"

# -------- FUSE mount smoke matrix (AC-009.22+) --------
[group: 'test']
fuse-smoke *args:
    @echo ">> fuse-smoke matrix (docs/ops/fuse-mount-smoke-matrix.md)"
    @cargo run -p fuse-smoke-runner --locked -- {{args}}

[group: 'test']
fuse-smoke-colima:
    @echo ">> Mac-hosted Linux FUSE smoke via Colima (no host macFUSE reboot)"
    @cargo run -p fuse-smoke-runner --locked -- --cell mac_host_linux_colima --json

# -------- testing --------
[group: 'test']
test:
    @echo ">> cargo test (all features, all targets)"
    @cargo test --locked --all-features --all-targets

[group: 'test']
test-nextest:
    @echo ">> cargo nextest run (faster, JUnit XML)"
    @command -v cargo-nextest >/dev/null 2>&1 || cargo install --locked cargo-nextest
    @cargo nextest run --locked --all-features --profile ci

[group: 'test']
test-e2e:
    @echo ">> cargo nextest run e2e/chaos tier (C07 L64)"
    @command -v cargo-nextest >/dev/null 2>&1 || cargo install --locked cargo-nextest
    @cargo nextest run --locked --all-features --profile e2e -E 'test(/_e2e_/)'

[group: 'test']
test-doc:
    @echo ">> cargo test --doc"
    @cargo test --doc --locked --all-features

# -------- coverage --------
[group: 'coverage']
coverage:
    @echo ">> cargo llvm-cov (lcov)"
    @command -v cargo-llvm-cov >/dev/null 2>&1 || cargo install --locked cargo-llvm-cov
    @cargo llvm-cov --lib --locked --all-features --workspace \
      --ignore-filename-regex '(^|/)(commands|cast)/|wasm_opcode\.rs$|coordination\.rs$|(^|/)src/runtime/|speculative_cache\.rs$|(^|/)crates/' \
      --lcov --output-path lcov.info
    @echo ">> coverage written to lcov.info"

# The scope above mirrors the quality-gate.yml unit coverage gate so that a
# local run reproduces the gate. To enforce the same 85% floor locally, use
# `just coverage-gate` instead.
[group: 'coverage']
coverage-gate:
    @echo ">> cargo llvm-cov (lcov) with the 85% unit coverage gate"
    @command -v cargo-llvm-cov >/dev/null 2>&1 || cargo install --locked cargo-llvm-cov
    @cargo llvm-cov --lib --locked --all-features --workspace \
      --fail-under-lines 85 \
      --ignore-filename-regex '(^|/)(commands|cast)/|wasm_opcode\.rs$|coordination\.rs$|(^|/)src/runtime/|speculative_cache\.rs$|(^|/)crates/' \
      --lcov --output-path lcov.info
    @echo ">> coverage written to lcov.info"

# -------- security --------
[group: 'security']
audit:
    @echo ">> cargo audit (RustSec advisories)"
    @command -v cargo-audit >/dev/null 2>&1 || cargo install --locked cargo-audit
    @cargo audit

[group: 'security']
deny:
    @echo ">> cargo deny check (licenses, bans, advisories, sources)"
    @command -v cargo-deny >/dev/null 2>&1 || cargo install --locked cargo-deny
    @cargo deny check

[group: 'security']
osv-scan:
    @echo ">> osv-scanner HIGH/CRITICAL on Cargo.lock (C04 L38 hard parity)"
    @bash scripts/ci/osv_scan.sh

# -------- doc --------
[group: 'doc']
doc:
    @echo ">> cargo doc (open in browser)"
    @cargo doc --no-deps --locked --all-features --open

[group: 'doc']
doc-build:
    @echo ">> cargo doc (build only)"
    @cargo doc --no-deps --locked --all-features

[group: 'doc']
man:
    @echo ">> clap_mangen → share/man/man1/sharecli.1 (C09 L81.13)"
    @cargo run --locked --bin sharecli -- man --install

# -------- quality gates --------
[group: 'gate']
gate: lint test audit deny fmt-check
    @echo ">> all gates green"

# Soft hermetic: fetch then offline build (C06 L54). See docs/ops/hermetic-builds.md
[group: 'gate']
hermetic:
    @echo ">> cargo fetch --locked && cargo build --locked --offline -p sharecli"
    @cargo fetch --locked
    @cargo build --locked --offline -p sharecli
    @echo ">> hermetic soft gate green"

# C06 L56 — hard cosign dry-run (no GHCR push). Full publish: container-cosign.yml
[group: 'gate']
container-cosign-hard:
    @echo ">> container cosign hard dry-run (SKIP_GHCR_PUSH)"
    @SKIP_GHCR_PUSH=true GITHUB_REPOSITORY="${GITHUB_REPOSITORY:-KooshaPari/sharecli}" bash scripts/container-cosign-hard.sh

# Deploy-side verify; pass image ref as first arg, e.g. just container-cosign-verify ghcr.io/...@sha256:...
[group: 'gate']
container-cosign-verify image="":
    @bash scripts/container-cosign-verify.sh {{image}}

[group: 'gate']
gate-release: lint-pedantic test audit deny fmt-check build-release
    @echo ">> release gate green"

# -------- repo hygiene --------
[group: 'hygiene']
typos:
    @echo ">> typos (spellcheck)"
    @command -v typos >/dev/null 2>&1 || cargo install --locked typos-cli
    @typos --config _typos.toml

[group: 'hygiene']
clean:
    @echo ">> cargo clean (target/)"
    @cargo clean

[group: 'hygiene']
outdated:
    @echo ">> cargo outdated"
    @command -v cargo-outdated >/dev/null 2>&1 || cargo install --locked cargo-outdated
    @cargo outdated --workspace

# -------- observability --------
[group: 'hygiene']
grade:
    @echo ">> audit scorecard snapshot"
    @test -f audit_scorecard.json && cat audit_scorecard.json | jq '{repo, overall, grade}' \
        || echo "no audit_scorecard.json present"

# -------- observability (C05 L45) --------
[group: 'observability']
pyro-push-sample:
    @echo ">> curl pprof from sharecli serve and push to Pyroscope (see docs/ops/pyroscope.md)"
    @test -n "${SHARECLI_PPROF:-}" || { echo "error: set SHARECLI_PPROF=1 and run sharecli serve first"; exit 1; }
    @PYRO_URL="${SHARECLI_PYROSCOPE_URL:-http://127.0.0.1:4040}"; \
     APP="${SHARECLI_PYROSCOPE_APP_NAME:-sharecli}"; \
     PPROF_URL="${SHARECLI_PPROF_URL:-http://127.0.0.1:9000/debug/pprof/profile}"; \
     SECS="${SHARECLI_PPROF_SECONDS:-10}"; \
     OUT="/tmp/sharecli-profile-$$.pb"; \
     CURL_AUTH=(); \
     if [ -n "${SHARECLI_SERVE_TOKEN:-}" ]; then CURL_AUTH=(-H "Authorization: Bearer ${SHARECLI_SERVE_TOKEN}"); fi; \
     echo ">> sampling ${PPROF_URL}?seconds=${SECS}"; \
     curl -fsS "${CURL_AUTH[@]}" -o "$$OUT" "$${PPROF_URL}?seconds=$${SECS}"; \
     if command -v pyroscope >/dev/null 2>&1; then \
       echo ">> pyroscope push -> $${PYRO_URL}"; \
       pyroscope push "$$OUT" --application-name="$${APP}" --server-address="$${PYRO_URL}"; \
     elif [ -n "$${SHARECLI_PYROSCOPE_USER:-}" ] && [ -n "$${SHARECLI_PYROSCOPE_PASSWORD:-}" ]; then \
       echo ">> curl ingest -> $${PYRO_URL}/ingest"; \
       curl -fsS -u "$${SHARECLI_PYROSCOPE_USER}:$${SHARECLI_PYROSCOPE_PASSWORD}" \
         -H "Content-Type: application/vnd.google.protobuf" \
         --data-binary @"$$OUT" \
         "$${PYRO_URL}/ingest?format=pprof&name=$${APP}"; \
     else \
       echo ">> no pyroscope CLI or Grafana Cloud creds — saved $$OUT"; \
       echo ">> install pyroscope CLI or set SHARECLI_PYROSCOPE_USER/PASSWORD; see docs/ops/pyroscope.md"; \
       go tool pprof -top "$$OUT" 2>/dev/null || true; \
     fi; \
     rm -f "$$OUT"

# -------- local CI simulation --------
# Mirrors .github/workflows/ci.yml — useful for `act` or local debugging
[group: 'ci']
ci: install-tools fmt-check lint test build audit deny
    @echo ">> local CI pipeline green"

[group: 'ci']
ci-fast: fmt-check lint test build
    @echo ">> local CI fast lane green"

# -------- release --------
[group: 'release']
version:
    @grep '^version' Cargo.toml | head -1 | cut -d'"' -f2

[group: 'release']
changelog:
    @echo ">> generating CHANGELOG.md via git-cliff"
    @command -v git-cliff >/dev/null 2>&1 || cargo install --locked git-cliff
    @git-cliff --tag "$(just version)" --output CHANGELOG.md

[group: 'release']
publish: build-release
    @echo ">> cargo publish --dry-run to {{ registry }}"
    @cargo publish --dry-run --locked

# C06 L52 — bit-identical release binary (unix only; skips on Windows).
[group: 'release']
repro-check:
    @echo ">> reproducible build digest check (SOURCE_DATE_EPOCH)"
    @bash scripts/repro-check.sh

# -------- C07 DevEx (append-only) --------
[group: 'devex']
dev: install-tools
    @echo ">> one-command local bootstrap (Rust + Zig + smoke + seed verify)"
    @command -v zig >/dev/null 2>&1 || { echo "error: zig 0.14.1 required (see .devcontainer/post-create.sh)"; exit 1; }
    @cargo build --locked --all-features
    @cargo run --locked --quiet -- --help >/dev/null
    @bash scripts/dev/verify_seed.sh
    @echo ">> ready — next: just test-nextest | just gate"

[group: 'devex']
mutants:
    @echo ">> cargo-mutants hard gate (thermal-tui examine set; zero survivors)"
    @command -v cargo-mutants >/dev/null 2>&1 || cargo install --locked cargo-mutants
    @cargo mutants --timeout 60 --jobs 2 -p sharecli-thermal-tui -- --locked

[group: 'devex']
fuzz:
    @echo ">> cargo fuzz toml_lite (30s; requires nightly + cargo-fuzz)"
    @command -v cargo-fuzz >/dev/null 2>&1 || cargo install --locked cargo-fuzz
    @rustup toolchain install nightly --profile minimal
    @cargo +nightly fuzz run toml_lite -- -max_total_time=30

# Soft: validate docs/eval/corpus scenario JSON
eval-corpus:
    bash scripts/eval/run-corpus.sh

# -------- C05 load (soft) --------
[group: 'ops']
load-soft:
    @echo ">> soft healthz burst (requires curl; starts serve on :9000)"
    @cargo build --locked --release -p sharecli
    @bash -c 'set -euo pipefail; ./target/release/sharecli serve --bind 127.0.0.1:9000 & pid=$!; trap "kill $$pid 2>/dev/null || true" EXIT; for i in $$(seq 1 30); do curl -sf -o /dev/null http://127.0.0.1:9000/healthz && break; sleep 1; done; SHARECLI_LOAD_URL=http://127.0.0.1:9000/healthz SHARECLI_LOAD_N=50 bash scripts/load/healthz_burst.sh'

load-soak:
    @echo ">> soft healthz soak 5m (requires curl; starts serve on :9000)"
    @cargo build --locked --release -p sharecli
    @bash -c 'set -euo pipefail; ./target/release/sharecli serve --bind 127.0.0.1:9000 & pid=$!; trap "kill $$pid 2>/dev/null || true" EXIT; for i in $$(seq 1 30); do curl -sf -o /dev/null http://127.0.0.1:9000/healthz && break; sleep 1; done; SHARECLI_LOAD_URL=http://127.0.0.1:9000/healthz SHARECLI_SOAK_SEC=300 bash scripts/load/soak_healthz.sh'

chaos-soft:
    @echo ">> soft chaos restart (requires curl; kill + recover /healthz < 30s)"
    @cargo build --locked --release -p sharecli
    @SHARECLI_LOAD_URL=http://127.0.0.1:9000/healthz SHARECLI_SERVE_BIND=127.0.0.1:9000 bash scripts/load/chaos_restart.sh

chaos-hard:
    @echo ">> hard chaos restart (CI parity; kill + recover /healthz < 30s)"
    @cargo build --locked --release -p sharecli
    @SHARECLI_LOAD_URL=http://127.0.0.1:9000/healthz SHARECLI_SERVE_BIND=127.0.0.1:9000 SHARECLI_SERVE_BIN=./target/release/sharecli bash scripts/load/chaos_restart.sh

# C05 L46 — validate MWMB burn-rate alert pairs + error-budget policy refs
[group: 'gate']
slo-alerts-validate:
    @echo ">> C05 L46 MWMB alert + error-budget policy gate"
    @cargo test --locked --test c05_l46_error_budget_mwmb -- --nocapture

[group: 'parity']
packaging-deb:
    @echo ">> build unsigned .deb (C11 L108 phase 3)"
    @cargo build --locked --release -p sharecli
    @bash scripts/packaging/build_deb.sh

# -------- C00 memory soft --------
[group: 'ops']
rss-soft:
    @echo ">> soft idle RSS sample (docs/ops/memory.md)"
    @cargo build --locked --release -p sharecli --features jemalloc
    @bash scripts/ops/rss_soft.sh

dhat-soft:
    @echo ">> soft dhat heap sample (docs/ops/alloc-profiling.md)"
    @bash scripts/ops/dhat_soft.sh

# C00 L7 — loom model of ProcessPool pid index (hard gate parity)
[group: 'gate']
loom:
    @echo ">> loom pool_index + relaxed counter (C00 L7)"
    @RUSTFLAGS="--cfg loom" cargo test --release --locked -p sharecli-sync --test loom_pool_index

serve-jemalloc:
    @echo ">> build serve binary with jemalloc (C00 L8)"
    @cargo build --locked --release -p sharecli --features jemalloc
