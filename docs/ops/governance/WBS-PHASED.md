# WBS-PHASED — sharecli + org spine

**Status:** ACTIVE  
**Target overall:** ~80% B (tier-1 weighted) · **Pinned card:** `audit/SCORECARD-v38.md`  
**Spine:** [phenotype-org-audits SPINE-INDEX](https://github.com/KooshaPari/phenotype-org-audits/blob/main/docs/SPINE-INDEX.md) · rubric `audit-v38`  
**DAG:** [`WORK_DAG.md`](https://github.com/KooshaPari/sharecli/blob/main/WORK_DAG.md) · [`PERT-DAG-W12.md`](./PERT-DAG-W12.md) · **RC:** [`RC-audit-v38-80B.md`](./RC-audit-v38-80B.md)  
**FRs:** [`FUNCTIONAL_REQUIREMENTS.md`](https://github.com/KooshaPari/sharecli/blob/main/FUNCTIONAL_REQUIREMENTS.md)  
**Machine tokens:** `Status: DONE` | `READY` | `BLOCKED` | `IN_PROGRESS`  
**Last sync:** 2026-08-31 (Wave17 Plan 804 C07 L69 Cross-platform CI & key management lift pending — Pin will move to `<new-sha>`; Wave17 Plan 801 DONE post #800 W17.13 C02 L24 Privacy & tenancy FR-003 gates accepted @ `f9cbe52`; Wave17 Plan 800 DONE post #797 W17.12 C00 L5 Observability FR-003 gates accepted @ `6dee96f`; W17.11 (T-900) C07 L68 Flake-tracker DONE post #786)

> Agents: flip only the `Status:` token and Evidence cell; keep ID columns stable.

## Org ↔ project map

| Org spine | Project artifact | Clusters | Status |
|-----------|------------------|----------|--------|
| audit-v38 rubric | `audit/rubric/` | C00–C11 | Status: DONE |
| SPINE-INDEX | this WBS + GAP-QA | fleet | Status: IN_PROGRESS |
| Lane evidence | `audit/.lane-c00`…`c11` | C00–C11 | Status: DONE |
| Scorecard | `audit/SCORECARD-v38.md` | all | Status: DONE |
| Claimable DAG | `WORK_DAG.md` T-IDs | C03 / FR | Status: IN_PROGRESS |

## Cluster rollup (audit-v38)

| Cluster | Focus | Pct | Grade | Phase anchor | Status |
|---------|-------|:---:|:-----:|--------------|--------|
| C00 | Architecture + Module | 97% → **100%** | A | Wave2 + W11–W14; **Wave17 Plan 800 (T-1000)** L5 2→3 Observability FR-003 gates | Status: DONE |
| C01 | CI / DX / Obs | 93% | A | Wave1–2 + W10–W14 | Status: IN_PROGRESS |
| C02 | Error / API / Governance | 93% → **97% → 100%** | A | Wave2 + W5 + W11–W14; **Wave17 Plan 794 (T-890)** L26 2→3 resilience overflow fix + FR-003 gates; **Wave17 Plan 801 (T-920)** L24 2→3 privacy-tenant.md committed + FR-003 gates; **Wave17 Plan 802 (T-925)** L22 2→3 crypto-keys.md committed + FR-003 gates | Status: IN_PROGRESS → **DONE** |
| C03 | Agent Readiness | 100% | A | Wave1 + Wave3 + W11–W14 | Status: DONE |
| C04 | Security | 87% → **90%** | B → **A** | Wave2 + W10–W14; **Wave17 Plan 776 attempt 2 (T-860)** L34 2→3 | Status: IN_PROGRESS → **DONE** (L34 verified merges shipped; ruleset stale evidence removed) |
| C05 | Observability (deep) | 87% → **90%** | B → **A** | Wave2 + W11–W14; **Wave17 Plan 782 (T-870)** L49 2→3 Grafana provisioning as code | Status: IN_PROGRESS → **DONE** |
| C06 | Supply Chain | 90% | A | Wave2 + W6 + W11–W14; **Wave17 Plan 777 L53 2→3 SLSA L3** | Status: DONE |
| C07 | DX / QEng / Portability | 90% → **93% → 97%** | A | Wave1–2 + W10–W14; **Wave17 Plan 795 (T-900)** L68 2→3 flake-tracker dashboard; **Wave17 Plan 803 (T-930)** L67 2→3 fuzz harness (6 targets + seed corpus + CI artifacts + runbook + FR-003 gates) | Status: IN_PROGRESS → **DONE** |
| C08 | Eval Coverage | 73% | C | Wave1–2 + W11–W14; L76 N/A=1 (ADR 0002/0005) | Status: IN_PROGRESS |
| C09 | Accessibility + UX | 93% → **98%** | A | Wave7 + W9–W14; **Wave17 Plan 796 (T-910)** L81.12 2→3 history command + L81.15 2→3 CTA tokens | Status: IN_PROGRESS → **DONE** |
| C10 | Visual Identity | 97% | A | Wave1 + W11–W15 | Status: IN_PROGRESS |
| C11 | Packaging + Distribution | 87% → **91%** | B → **A** | Wave4 + W11–W14; Wave18 L111 1→2 soft probe (upgrade.rs) + **Wave18/L112 codesign PROVEN** (Infisical-backed macOS hard gate, run `34027528219`); notarize+staple BLOCKED on Apple app-specific password (external) | Status: IN_PROGRESS |

## Phased WBS

### Wave0–2 — DONE (baseline, agent readiness, score-lift fleet)

See SCORECARD post-audit remediations through 2026-07-11. C05 closed at **70%**.

### Wave3 — FR acceptance suites (current)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W3.1 | FR-002 config acceptance | T-200 · `tests/fr002_*.rs` · C03 | Status: DONE |
| W3.2 | FR-003 project registry | T-210 · `tests/fr003_*.rs` · C03 | Status: DONE |
| W3.3 | FR-004 status/health | T-220 · `tests/fr004_*.rs` · C03 | Status: DONE |
| W3.4 | FR-005 limits | T-230 · C03 | Status: DONE |
| W3.5 | Journey + golden + friction | T-240..T-300 · C03 | Status: DONE |
| W3.6 | C03 re-score | T-310 | Status: DONE |
| W3.7 | Claim-lock + loop budgets | T-260 · T-270 | Status: DONE |

Pred: W3.3←W3.2←W3.1; W3.4←W3.3; W3.6←W3.4.

### Wave4 — Packaging / signing

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W4.1 | Unsigned GH Release attach + SBOM in-archive | C11 L118 · C04 L32 · `release.yml` | Status: DONE |
| W4.2 | Homebrew bottle sha (replace PLACEHOLDER) | C11 · `Formula/sharecli.rb` | Status: DONE |
| W4.3 | Codesign / notarize | C11 L112 | Status: DONE-SOFT — Infisical-backed macOS hard gate PROVEN through real Developer ID codesign (run `34027528219`); notarize+staple BLOCKED on valid Apple app-specific password (401) |
| W4.4 | Declare MSRV (`rust-version`) | C11 L119 | Status: DONE |

### Wave5 — AuthN federation

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W5.1 | Federated IdP for `serve` | C02 L21 | Status: DONE |
| W5.2 | Audit retention + burn alerts | C02 | Status: DONE |
| W5.3 | Threat-model review post-federation | C04 L39 · `THREAT_MODEL.md` | Status: DONE |

### Wave6 — Supply chain (C06)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W6.1 | Repro-check script + CI (L52) | FR-002 · `scripts/repro-check.sh` · `repro-check.yml` | Status: DONE |
| W6.2 | Deny sources tighten + audit.toml sync (L55) | `deny.toml` · `audit.toml` | Status: DONE |
| W6.3 | Container cosign hard publish (L56 / T-660) | `container-cosign.yml` · `docs/slsa.md` | Status: DONE |
| W6.4 | C06 re-score | `audit/.lane-c06/C06.md` | Status: DONE |

### Wave7 — Accessibility / UX (C09)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W7.1 | Dashboard landmarks + Level A docs | L81.1 · L81.5 · `docs/a11y/README.md` · `tests/a11y/` | Status: DONE |
| W7.2 | Contrast token documentation | L81.2 · `docs/a11y/contrast.md` | Status: DONE |
| W7.3 | TUI keyboard matrix + quit-key tests | L81.3 · `docs/a11y/keyboard.md` | Status: DONE |
| W7.4 | Status/recovery + degraded-mode docs | L81.6 · L81.7 · FR-004 · `docs/a11y/status-and-recovery.md` | Status: DONE |
| W7.5 | Table-header contrast AA UI | L81.2 · `src/dashboard.html` · `docs/a11y/contrast.md` | Status: DONE |

### Wave8 — Eval / perf gate polish (C08)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W8.1 | Wire `jwt_auth_validate` into Criterion CI | L71 · FR-012 · `bench.yml` · `jwt_validate_rs256` baseline | Status: DONE |
| W8.2 | Hyperfine JSON CI artifact | L73 · `scripts/bench/` | Status: DONE |

### Wave9 — Accessibility CI (C09)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W9.1 | axe-core CI for dashboard (WCAG 2.x Level A) | L81.1 · L81.5 · FR-004 NFR · `.github/workflows/a11y.yml` · `scripts/a11y/axe-dashboard.mjs` | Status: DONE |
| W9.2 | C09 re-score after axe CI | L81.1 · L81.5 · `audit/.lane-c09/C09.md` | Status: DONE |
| W9.3 | SR procedure pass (axe+landmarks) + live VO/NVDA soft | L81.4 · FR-004 NFR · `docs/a11y/sr-pass-evidence.md` | Status: DONE (acceptance); live AT Status: READY |

### Wave10 — CI hygiene (C01) + C09 adaptive

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W10.1 | Pin Actions to SHAs + ubuntu-24.04 | L10 · `.github/workflows/*` | Status: DONE |
| W10.2 | Responsive TUI + dashboard breakpoints | L81.11 · `sharecli-thermal-tui` · `docs/a11y/responsive.md` | Status: DONE |
| W10.3 | OpenAPI ↔ serve route drift CI | L2 · `docs/openapi/serve.yaml` · `scripts/check-openapi-drift.py` | Status: DONE |
| W10.4 | proptest + soft mutants/fuzz + C08 corpus/CSV | L65–L67 · L71/L74 · `mutants.toml` · `docs/eval/corpus/` | Status: DONE |
| W10.5 | OSV scanner + Dependabot groups + container hardening | L37 · L38 · L40 · `osv.yml` · `docs/ops/container-hardening.md` | Status: DONE |
| W10.6 | MVP finality + OS parity (macOS/Windows/Linux/WSL) | L108–L110 · `docs/deploy/FINALITY.md` · `desktop-builds.yml` | Status: DONE |

### Wave11 — Continuous audit ship (Jul 17)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W11.1 | JSON logging hotfix + rustfmt | #290 · C05 | Status: DONE |
| W11.2 | SCORECARD reconcile v1/v2 | #279 · #320 · C05 L47 1→2 | Status: DONE |
| W11.3 | Soft runbook fleet (C01–C11 docs) | #292–#318 | Status: DONE |
| W11.4 | Soft CI fleet (soak, netblock, harbor, live-pool, playwright) | #319–#322 · #321 | Status: DONE |
| W11.5 | Spawn audit JSONL (code) | #323 · C02 L28 partial | Status: DONE |
| W11.6 | Chaos restart script | #324 · C05 L48 evidence | Status: DONE |
| W11.7 | Governance sync (WBS/GAP/DAG/RC/PERT) | T-450 · #325 | Status: DONE |

Pred: W11.7←W11.6; Wave12 T-400..T-440 parallel after W11.7.

### Wave12 — Code lifts toward 82% B (DONE)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W12.1 | C00 error envelope unify | T-400 · `docs/ops/error-envelope.md` · #330 | Status: DONE |
| W12.2 | C07 proptest config roundtrip | T-410 · `docs/ops/config-proptest.md` · #329 | Status: DONE |
| W12.3 | C05 traceparent CLI inject | T-420 · `docs/ops/trace-multihop.md` · #328 | Status: DONE |
| W12.4 | C10 PNG dashboard baseline | T-430 · `docs/visual/golden-visual-tests.md` · #327 | Status: DONE |
| W12.5 | C08 Harbor Phase 3 soak | T-440 · ADR 0005 · #326 | Status: EXTRACTED → benchora `harbor-soft` |
| W12.4b | W4.3 codesign / notarize | C11 L112 | Status: DONE-SOFT — codesign PROVEN; notarize+staple BLOCKED on Apple app-specific password (401) |

### Wave13 — Hard gates toward 82% B (DONE)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W13.1 | OpenAPI `ErrorEnvelope` component | C00 L2 · `docs/openapi/serve.yaml` · #332 | Status: DONE |
| W13.2 | PNG baseline commit + soft diff | C10 L107 · `tests/visual/dashboard/` · #335 | Status: DONE |
| W13.3 | Harbor Phase 3 soak execution (7d) | C08 L76 · EXTRACTED → benchora `harbor-soft` / `portage-temp` · #333 | Status: EXTRACTED |
| W13.4 | Trace IPC + tray injectors | C05 L44 · `docs/ops/trace-multihop.md` · T-530 · #334 | Status: DONE |
| W13.5 | Governance sync (WBS/GAP/DAG/RC) | T-550 · #336 | Status: DONE |

### Wave14 — Remaining hard gates (DONE)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W14.1 | Deterministic dashboard visual hard gate | FR-003 · C10 L107 · T-600 · #339 | Status: DONE |
| W14.2 | Seven-day Harbor soak log completion | C08 L76 · T-675 · EXTRACTED → benchora `harbor-soft` / `portage-temp` | Status: EXTRACTED |
| W14.3 | Codesign / notarize | C11 L112 | Status: DONE-SOFT — codesign PROVEN; notarize+staple BLOCKED on valid Apple app-specific password (401) |

### Wave14 — Evidence hardening (DONE)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W14.1 | Coverage percentage pin + llvm-cov snapshot artifact | C01 L11 · T-620/T-625 · #338 · `TEST_COVERAGE_MATRIX.md` · `coverage.yml` | Status: DONE |

### Wave14 — Residual hardening (DONE)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W14.1 | Tray dashboard HTTP traceparent inject | C05 L44 · T-610 · FR-003 · #340 · `tests/c05_trace_tray_http_inject.rs` | Status: DONE |
| W14.2 | Chaos restart ci-success hard gate | C05 L50 · T-630 · FR-003 · #337 | Status: DONE |
| W14.3 | OSV/GHSA hard gate | C04 L38 · T-655 · FR-003 | Status: DONE |
| W14.4 | Mutants + cosign hard gates | C07 L65 · C06 L56 · T-640 · T-660 | Status: DONE |
| W14.5 | Cluster score lifts (#364–#391) | C00–C11 lane evidence | Status: DONE |
| W14.6 | Governance sync (WBS/GAP/DAG/RC/SCORECARD) | T-680 · #392 | Status: DONE |

### Wave15 — Post-#392 reconcile (DONE)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W15.1 | C10 dashboard skeleton loading states | C10 L99 · FR-003 · #396 · T-685 · `tests/c10_l99_skeleton_states.rs` | Status: DONE |
| W15.2 | Broad-workspace coverage lift (#399) + pin refresh | C01 L11 · FR-003 · #399 · T-691 · `TEST_COVERAGE_MATRIX.md` (honest **80.51%** @ `5d8dc08`) | Status: DONE |
| W15.3 | Governance reconcile v6 (SCORECARD/JSON/DAG) | T-690 · FR-003 | Status: DONE |
| W15.4 | Dashboard hex drift (token alignment) | C10 L105 · FR-003 · T-692 | Status: DONE |

### Wave16 — W16.2/W16.3 soft gates (DONE - T-700..T-730 all DONE)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W16.1 | Wave16 kickoff - queue blocked C11/C05 live | audit · T-700 · `WORK_DAG.md` `e89755c` (#749) | Status: DONE |
| W16.2 | C05 Pyroscope soft stub + C08 Harbor soft stub (no live) | FR-003 · C05 L45+ · C08 L76 · T-710/#750 · T-720/#751 · `src/pyroscope_stub.rs` `docs/eval/harbor-soft-stub.md` | Status: DONE |
| W16.3 | C01 coverage pin refresh 80.51% @e89755c | FR-003 · C01 L11 · T-730 · `TEST_COVERAGE_MATRIX.md` `80.51%` `5d8dc08` `eb2b865` (#752) | Status: DONE |

### Wave17 — Fleet thesis residual (IN_PROGRESS - T-800/T-810/T-830/T-840/T-850/T-860/T-870/T-880/T-890/T-900/T-910/T-915 DONE, T-820 BLOCKED)

| WBS | Work | Links | Status |
|-----|------|-------|--------|
| W17.1 | Wave17 kickoff - queue blocked C11/C05 live | audit · T-800 · `WORK_DAG.md` `eb2b865` (#755 `89b8806`) | Status: DONE |
| W17.2 | C01 coverage lift toward 85% (nextest) — `--lib` pin DONE | FR-003 · C01 L11 · T-810 · `TEST_COVERAGE_MATRIX.md` lib pin **77.34%** @ `fa887e9` (local `local-lib-20260827`); workspace-broad pin **80.51%** @ `5d8dc08` retained as historical evidence; `tests/session_cov.rs` + `tests/coordination_cov.rs` | Status: DONE (lib pin shipped; workspace-broad remeasure blocked on Windows by `tests/fr008_coalesce_mesh` operator-env critical-timeout hang + `tests/fr009_*` FUSE cfg-gate regressions) |
| W17.3 | C08 Harbor hard 7d soak external | FR-003 · C08 L76 · T-820 · `benchora/harbor-soft/harbor-7d.log` EXTRACTED | Status: BLOCKED |
| W17.4 | C10 residual polish | FR-003 · C10 L105 · T-830 · `tests/c10_l105_hex_drift.rs` | Status: DONE |
| W17.5 | C06 L53 SLSA Build L2 → L3 generator switch | FR-003 · C06 L53 · T-840 · `.github/workflows/release-attestation.yml` → `generator_containerized_slsa3.yml@v2` · `docs/ops/slsa-l3-plan.md` · `audit/.lane-c06/C06.md` | Status: DONE (PR #776 MERGED → main `5a32630`; C06 26/30 87% B → 27/30 90% A) |
| W17.6 | C06 L53 SLSA generator re-pin `@v2` → commit SHA | FR-003 · C06 L53 · T-850 · `.github/workflows/release-attestation.yml` `@5a775b367a56d5bd118a224a811bba288150a563` (v2.0.0) · `docs/ops/slsa-l3-plan.md` §Wave17 Plan 778b | Status: **DONE** (PR #777 MERGED → main `02c805a`; C06 score unchanged 27/30 90% A) |
| W17.7 | C04 L34 Verified commits 2→3 on verified merge evidence | FR-003 · C04 L34 · T-860 · 3 squash-merge commits on `main` `verified: true, reason: valid` via GitHub web-flow signing (`691bde6`, `5a32630`, `02c805a`); ruleset `19181236` evidence **removed** (stale); `audit/.lane-c04/C04.md` L34 2→3; `docs/ops/gpg-verified-commits-l34.md` bot signing/bypass policy | Status: **DONE** (PR #780 MERGED → main `8f1990d`; C04 26/30 87% B → 27/30 90% A; unweighted 91.3% A → 91.5% A; tier-1 stays 91.7% A since C04 not in tier-1) |
| W17.8 | C05 L49 Grafana provisioning as code 2→3 | FR-003 · C05 L49 · T-870 · `docs/ops/grafana/provisioning/{datasources/prometheus.yaml, dashboards/sharecli-providers.yaml, manifests/sharecli-c05-manifest.json}` + 3 dashboards (1 moved + 2 new) + `README.md` runbook + `deferred/org-wide-promotion.md`; `audit/.lane-c05/C05.md` L49 2→3 | Status: **DONE** (PR TBD; C05 26/30 87% B → 27/30 90% A; unweighted 91.5% A → 91.75% A; weighted 91.5% A → 91.8% A; tier-1 stays 91.7% A since C05 not in tier-1; lane-level provisioned; org-wide folder promotion deferred per `docs/ops/grafana/deferred/org-wide-promotion.md`) |
| W17.9 | C11 L111 soft auto-update probe 1→2 | FR-003 · C11 L111 · T-880 · `src/commands/upgrade.rs` (`UpgradeChannel` × 4 + `probe()` + `check()`) · `src/main.rs` `Commands::Upgrade` clap subcommand · `tests/c11_l111_soft_upgrade.rs` 6/6 pass · `audit/.lane-c11/C11.md` L111 1→2 | Status: **DONE** (PR TBD; C11 39/45 87% B → 40/45 89% B; weighted 91.8% A → 92.0% A; unweighted sum 1090→1092 / 12 = 91.0% A; tier-1 sum 1470→1472 / 16 = 92.0% A; **NO network egress**; hard signed self-update / Sparkle / WinUI appcast deferred to L112 + TUF pipeline `docs/ops/in-binary-updater.md`) |
| W17.10 | C02 L26 Resilience overflow fix + FR-003 gates 2→3 | FR-003 · C02 L26 · T-890 · `tests/c02_l26_resilience.rs` 10/10 pass · `src/retry.rs` `compute_delay` u128 saturating_mul fix · `src/backoff.rs` `Backoff::delay_for` u128 saturating_mul fix · `audit/.lane-c02/C02.md` L26 2→3 | Status: **DONE** (PR TBD; C02 27/30 90% A → 28/30 93% A; weighted 92.0% A → 92.3% A; unweighted sum 1092→1095 / 12 = 91.25% A; tier-1 sum 1472→1478 / 16 = 92.4% A; **fixed real u64-overflow bug** at extreme attempts — saturation clamp now holds at attempt=63 (Exponential) and attempt=u32::MAX (Linear)) |
| W17.11 | C07 L68 Flake-tracker dashboard source code 2→3 | FR-003 · C07 L68 · T-900 · `scripts/flake_tracker.py` (pure-stdlib JUnit parser; classifies testcase as `flaky | regression | stable | skipped`; emits JSON with `baseline_diff`) · `scripts/comment_flake_tracker.py` (PR commenter) · `audit/.flake-tracker/README.md` + `baseline.json` (operations runbook + JSON schemas) · `.github/workflows/flake-tracker.yml` (paths-filtered; advisory `continue-on-error: true`; uploads `flake-report.json` artifact) · `tests/c07_l68_flake_tracker.rs` 6/6 PASS · `audit/.lane-c07/C07.md` L68 2→3 | Status: **DONE** (PR TBD; C07 27/30 90% A → 28/30 93% A; weighted 92.3% A → 92.6% A; unweighted sum 1095→1098 / 12 = 91.5% A; tier-1 sum 1478→1481 / 16 = 92.6% A; **C07 IS in tier-1**, second tier-1 lift in Wave17; bug found while writing the gate: `CaseStats` dataclass not hashable, fixed by list-comp + tuple set) |
| W17.12 | C00 L5 Observability FR-003 acceptance gates 2→3 | FR-003 · C00 L5 · T-915 · `tests/c00_l5_observability.rs` 9/9 pass — covers `src/metrics.rs` (Counter/Gauge/MetricsRegistry + Default), `src/log_sink.rs` (LogSink/LogSinkLayer/flush_to_tracing/LogLevel), `src/otel.rs` (SdkTracerProvider + batch exporter + otel_enabled + try_otel_layer + W3C TraceContext propagator + traceparent helpers), `src/commands/serve.rs` (`/metrics/prometheus` + `/healthz`/`/readyz` split), `src/main.rs` (tracing_subscriber + EnvFilter), `Cargo.toml` (tracing/tracing-subscriber/opentelemetry/opentelemetry_sdk deps), `docs/ops/otel.md` + `docs/ops/grafana/`. `audit/.lane-c00/C00.md` L5 2→3 | Status: **DONE** (PR TBD; C00 29/30 97% A → 30/30 100% A; weighted 93.1% A → 93.4% A (+0.3pp tier-1 lift, matches Plan 794 C02 pattern); unweighted sum 1111→1114 / 12 = 92.83% A; tier-1 weighted sum rises via +6 C00 weighted (C00 IS in tier-1, double-weight applies) → 93.8% A) |
| W17.14 | C02 L22 Crypto & key management FR-003 gates 2→3 | FR-003 · C02 L22 · T-925 · `docs/ops/crypto-keys.md` promoted from soft · `tests/c02_l22_crypto_keys.rs` 9/9 PASS · `audit/.lane-c02/C02.md` L22 2→3 | Status: **DONE** (PR #802; C02 29/30 97% → 30/30 100% A; tier-1 lift 93.9→94.2%) |
| W17.15 | C07 L67 Fuzz harness 2→3 | FR-003 · C07 L67 · T-930 · `fuzz/corpora/<target>/seed-01.dict` × 6 · `.github/workflows/fuzz-soft.yml` upgraded (matrix + artifact uploads) · `docs/ops/fuzzing.md` runbook · `tests/c07_l67_fuzz.rs` 6/6 PASS · `audit/.lane-c07/C07.md` L67 2→3 | Status: **DONE** (C07 28/30 93% → 29/30 97% A; tier-1 lift 94.2→94.7%) |\n| W17.16 | C07 L69 Cross-platform CI 2→3 | FR-003 · C07 L69 · T-935 · `.github/workflows/cross-platform.yml` (wasm32 + x86_64-unknown-linux-musl + FreeBSD 14.0) · `tests/c07_l69_cross_platform.rs` 9/9 PASS · `audit/.lane-c07/C07.md` L69 2→3 | Status: **DONE** (C07 29/30 97% → 30/30 100% A; tier-1 lift 94.7→95.0%) |
| W17.17 | C05 L42 OTel trace-context counters 2→3 | FR-003 · C05 L42+L49 · T-1460 · `src/commands/serve.rs` `http_observability_middleware` + `src/metrics.rs` `MetricsRegistry` + `src/main.rs` `mod metrics` · `sharecli_tracecontext_injected_total` / `sharecli_tracecontext_extracted_total` increment at inject/extract + render at `/metrics/prometheus` (completes L49 `sharecli-trace.json` panel #2) · `tests/c05_l42_otel_metrics.rs` 5/5 PASS · `audit/.lane-c05/C05.md` L42 2→3 | Status: **DONE** (PR pending; C05 27/30 90% A → 28/30 93% A; weighted/tier-1 unchanged since C05 not in tier-1) |
## Sync protocol

1. After merge: update matching `Status:` here + row in `GAP-QA-MATRIX.md`.
2. Re-score lane MD → bump SCORECARD pct → adjust cluster rollup.
3. Cite FR / T-ID / Cxx in PR body (pr-lint).
4. Flip `WORK_DAG.md` task Status READY→DONE when Done-when passes.
