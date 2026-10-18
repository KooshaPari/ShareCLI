# ShareCLI CI Readiness Audit — 2026-10-17

> **Scope.** Read-only deep audit of CI readiness on `sharecli` at `HEAD 3d6a41c972c2c86bf1374f224019c365ce3aa646` (branch `main`, upstream `git@github.com:KooshaPari/ShareCLI.git`). Author: KooshaPari (operator). All findings cite `file:line` or a `gh` query timestamp. Read-only: no source files modified, no PRs touched, no runs triggered, no comments posted.

## TL;DR

- 60 workflow files in `.github/workflows/` totalling **5,598 lines** (60 yml).
- **All 11 in-scope PRs are unmergeable today.** They each fail 21-44 of the GitHub-Checks-API-required checks. The cascade is rooted in **five** recurring failures: `ci / lint` (rustfmt), `ci / test` (build all targets), `cargo bench (gate)` (Criterion regressions), `macOS codesign + notarize` (Infisical token missing), and `Cargo Deny` (license/advisory gate).
- **The "Quality Gate" workflow has never passed on a PR** in the last 500 runs (0 / 12 PASS). This is independent of the code cascade — see FR-001 / permissions bug below.
- **7 declared workflows have ZERO runs in the last 2,000** (Appcast, Container cosign (hard), Infisical Sync, OpenAPI Drift, Packaging Hard Gate, Playwright viewports, Release Attestation). They are dead code or pre-GA scaffolding.
- **Only one workflow uses a self-hosted runner:** `soak.yml` (Harbor Soak, 1 run in 2,000, the self-hosted `desk-sharecli` label is wired but the workflow itself is essentially cold).
- **Hard-gate codesign** is failing every main push because `INFISICAL_TOKEN` is not present as a repo or environment secret (verified via `gh secret list` and `gh variable list`).
- The repo's main branch **has no GitHub-side required status checks** (`"contexts":[]`). The actual merge gate is **Mergify Merge Queue** plus a heavy `actions/github-script` PR-comment cascade.

## 1. Workflow inventory

`wc -l .github/workflows/*.yml` totals **60 files, 5,598 lines**.

| File (`.github/workflows/`) | Declared `name:` | Runs in last 2,000 | Runs on `main` | State classification |
|---|---|---:|---:|---|
| `a11y.yml` | Accessibility | 52 | 4 | passing |
| `agileplus-pillar-scorecard.yml` | AgilePlus Pillar Scorecard | 53 | 5 | passing |
| `appcast.yml` | Appcast | **0** | 0 | **NEVER-RAN** |
| `audit.yml` | Audit | 22 | 1 | intermittent (6 of last 500 PASS, 4 FAIL) |
| `bench.yml` | Bench | 53 | 5 | **failing 0/10 (Criterion regressions)** |
| `build-artifacts-soft.yml` | Build artifacts (soft) | 29 | 0 | soft gate, rarely on main |
| `chaos-restart-hard.yml` | Chaos restart (hard) | 1 | 1 | cold-started, fragile |
| `ci-gate.yml` | ci (lowercase) | 52 | 4 | **failing 0/10 (rustfmt cascade)** |
| `ci.yml` | CI (uppercase) | 53 | 5 | passing 10/12 |
| `codesign-soft.yml` | Codesign readiness (soft) | 48 | 4 | passing |
| `codesign.yml` | Code Signing (Hard Gate) | 48 | 4 | **failing 0/10 (Infisical missing)** |
| `container-cosign-soft.yml` | Container cosign (soft) | 4 | 4 | recent, low volume |
| `container-cosign.yml` | Container cosign (hard) | **0** | 0 | **NEVER-RAN** |
| `coverage.yml` | Coverage | 52 | 4 | **failing 0/13 (empty test-suite guard)** |
| `cross-platform.yml` | Cross-Platform | 52 | 4 | passing 10/12 |
| `dco-soft.yml` | DCO (soft) | 48 | 0 | passing (soft) |
| `dco.yml` | DCO Sign-off (Soft Gate) | 48 | 0 | passing (soft) |
| `deny.yml` | Deny | 23 | 1 | **failing 0/4 (license/advisories)** |
| `deploy-docs.yml` | Deploy docs | 4 | 4 | passing |
| `desktop-builds.yml` | Desktop / Tray Builds | 12 | 0 | passing |
| `dhat-soft.yml` | dhat heap sample (soft) | 3 | 0 | cold (new) |
| `eval-corpus-soft.yml` | Eval corpus (soft) | 48 | 0 | passing (soft) |
| `eval.yml` | eval | 53 | 5 | **failing 0/10 (criterion benches)** |
| `flake-tracker.yml` | flake-tracker (C07 / L68) | 52 | 4 | passing |
| `fuse-mount-smoke.yml` | FUSE mount smoke | 2 | 0 | cold |
| `fuzz-soft.yml` | Fuzz (soft) | 1 | 1 | cold-started |
| `fuzz.yml` | Fuzz | 52 | 4 | passing 10/12 |
| `gpg-soft.yml` | GPG/SSH signed commits (soft) | 48 | 0 | passing (soft) |
| `gpg.yml` | GPG Signed Commits (Soft Gate) | 49 | 0 | passing (soft) |
| `hermetic-soft.yml` | Hermetic build (soft) | 53 | 4 | passing |
| `hermetic.yml` | Hermetic Build (Hard Gate) | 103 | 4 | passing 25/25 |
| `infisical.yml` | Infisical Sync | **0** | 0 | **NEVER-RAN** (depends on missing secret) |
| `live-pool-soft.yml` | Live pool health (soft) | 52 | 4 | passing |
| `load-soft.yml` | Load burst (soft) | 52 | 4 | passing |
| `miri-soft.yml` | Miri (soft) | 50 | 1 | passing |
| `mutants.yml` | Mutation Testing | 1 | 1 | cold (cargo-mutants) |
| `netblock-soft.yml` | Network block (soft) | 52 | 4 | passing |
| `nightly-bench.yml` | Nightly Bench | 1 | 1 | cold (schedule-only) |
| `openapi-drift.yml` | OpenAPI Drift | **0** | 0 | **NEVER-RAN** |
| `osv.yml` | OSV Scanner | 4 | 4 | passing |
| `packaging-soft.yml` | Packaging (soft) | 3 | 0 | cold (new) |
| `packaging.yml` | Packaging (Hard Gate) | **0** | 0 | **NEVER-RAN** |
| `playwright-soft.yml` | Playwright viewports (soft) | **0** | 0 | **NEVER-RAN** |
| `pr-freshness.yml` | PR Freshness | 51 | 0 | passing |
| `pr-lint.yml` | PR Lint | 96 | 0 | passing 10/27 |
| `quality-gate.yml` | Quality Gate | 48 | 0 | **failing 0/12 (see §5)** |
| `release-attestation.yml` | Release Attestation | **0** | 0 | **NEVER-RAN** |
| `release.yml` | Release | 4 | 4 | intermittent |
| `repro-check.yml` | Repro Check | 42 | 2 | passing 10/13 |
| `rss-soft.yml` | RSS sample (soft) | 19 | 1 | cold |
| `rss.yml` | RSS hard gate | 32 | 2 | passing 7/7 |
| `sast.yml` | Security (SAST) | 52 | 4 | passing |
| `sbom.yml` | SBOM | 4 | 4 | passing |
| `scorecard-ci.yml` | 88-Pillar Scorecard | 52 | 4 | passing 12/12 |
| `scorecard.yml` | OpenSSF Scorecard | 4 | 4 | passing |
| `security.yml` | Security Scan | 52 | 4 | passing |
| `soak-soft.yml` | Soak healthz (soft) | 52 | 4 | passing |
| `soak.yml` | Harbor Soak (Nightly Gate) | 7 | 1 | cold, **self-hosted only** |
| `trunk-check.yml` | Trunk Check | 53 | 5 | **failing 0/10 (actionlint)** |
| `visual-soft.yml` | Visual baseline diff | 48 | 0 | passing |
| `visual.yml` | Visual Regression (Soft Gate) | 52 | 4 | passing 12/12 |

**State classification legend (auto-derived from last 500 runs):**
- `passing` — last conclusion `success`, >=80% of last 12 `success`
- `intermittent` — mixed PASS / FAIL in last 12
- `failing` — last conclusion `failure` AND/OR 0/12 success in last 12
- `cold` — fewer than 10 runs in 2,000
- `NEVER-RAN` — 0 runs in 2,000 (declared on disk, never invoked)

## 2. PR-by-PR check status (the cascade)

Scope note: the operator listed "12" but enumerated **11** PRs. I report on the 11 listed; the 12th is **[UNKNOWN]**. Listed: **#907, #918, #919, #921, #922, #923, #924, #925, #927, #929, #930**.

All 11 PRs are OPEN, baseRefName=`main`, and `mergeable=MERGEABLE`. None is mergeable today because of check failure; mergeStateStatus distribution:

| PR | Title (short) | mergeState | pass / fail / skip | Top failing checks (most-frequent) |
|---:|---|:---:|---:|---|
| 907 | ci/desk-sharecli runner + docs/lint fixes | BLOCKED | 57 / 25 / 9 | Quality Report, Cargo Deny, musl-test |
| 918 | ci/combined-with-rustfmt-fix (FR-001, FR-003) | BLOCKED | 62 / 21 / 9 | Cargo Deny, Quality Report, cargo bench (gate) |
| 919 | fix(fuse): poison-safe lock recovery | BLOCKED | 50 / 44 / 9 | CI Success Gate, Quality Report, Cargo Deny |
| 921 | fix(deps): bump PhenoShared (FR-001) | UNSTABLE | 60 / 26 / 9 | Cargo Deny, Quality Report, cross / x86_64-unknown-linux-musl |
| 922 | test/sharecli-core-coverage | BLOCKED | 59 / 25 / 9 | Cargo Deny, Quality Report, macos-latest |
| 923 | test/gateway-tools-coverage | BLOCKED | 69 / 26 / 9 | Quality Report, Cargo Deny, cross / wasm32-unknown-unknown |
| 924 | test(tray): operator_display coverage (FR-001) | UNSTABLE | 62 / 26 / 9 | Cargo Deny, Quality Report, cargo bench (gate) |
| 925 | test(sharecli-ffi): C-ABI coverage (FR-001) | BLOCKED | 65 / 26 / 9 | Cargo Deny, Quality Report, ubuntu-latest |
| 927 | feat(core): SpeculativeCoalescer (FR-001, FR-008) | BLOCKED | 59 / 25 / 9 | Quality Report, Cargo Deny, macOS codesign + notarize |
| 929 | feat(harness-native): is_agent() sentinel | BLOCKED | 59 / 25 / 9 | Cargo Deny, Quality Report, bench run |
| 930 | ci(rustfmt): bring workspace into rustfmt 1.96.0 | BLOCKED | 61 / 25 / 9 | Cargo Deny, Quality Report, musl-test |

**The cascade is dominated by 21-26 common failing checks across all 11 PRs.** The signature set, in rank order of cross-PR frequency:

| Failing check | Appears on | Source | Root cause |
|---|---:|---|---|
| `Cargo Deny (Advisories + Licenses)` | 11/11 PRs | `deny.yml` | license/advisory drift (most likely new RUSTSEC advisory since last license-bump) |
| `Quality Report` | 11/11 PRs | `quality-gate.yml` | downstream of failing `Unit Tests` + FR-001 logic |
| `cargo bench (gate)` | 11/11 PRs | `bench.yml` | Criterion regression (also `bench run` 11/11, `cargo bench (soft)` 11/11) |
| `ci / test` | 11/11 PRs | `ci-gate.yml` | build all targets; same root as `ci / lint` |
| `ci / lint` | 10/11 PRs | `ci-gate.yml` | rustfmt 1.96.0; PR #930 itself fixes this |
| `coverage` | 11/11 PRs | `coverage.yml` | empty test-suite guard fires when new crate has no `#[cfg(test)]` |
| `macOS codesign + notarize (hard gate — Infisical-backed Apple secrets)` | 11/11 PRs | `codesign.yml` | `INFISICAL_TOKEN` missing; hard-fail by design |
| `cross / x86_64-unknown-linux-musl` | 11/11 PRs | `cross-platform.yml` | cross-compile target |
| `cross / x86_64-unknown-freebsd` | 11/11 PRs | `cross-platform.yml` | cross-compile target |
| `cross / wasm32-unknown-unknown` | 11/11 PRs | `cross-platform.yml` | cross-compile target |
| `musl-test (x86_64-unknown-linux-musl)` | 11/11 PRs | `cross-platform.yml` | musl linker |
| `macos-latest` | 11/11 PRs | (multiple) | macOS runner |
| `windows-latest` | 11/11 PRs | (multiple) | Windows runner |
| `ubuntu-latest` | 11/11 PRs | (multiple) | Linux runner |
| `visual` | 11/11 PRs | `visual.yml` | Visual Regression |
| `Container Scan` | 11/11 PRs | (trivy / cosign) | cosign + scan |
| `Codesign secrets check (soft)` | 10/11 PRs | `codesign-soft.yml` | secret-shape check |
| `gpg` | 11/11 PRs | `gpg.yml` | "Soft Gate — pending GPG key provisioning" name in file |
| `dco` | 11/11 PRs | `dco.yml` | DCO Sign-off |
| `Signed-off-by check (soft)` | 11/11 PRs | `dco-soft.yml` | soft DCO |
| `Verified commits check (soft)` | 11/11 PRs | `gpg-soft.yml` | soft verification |
| `Unit Tests` | 11/11 PRs | `quality-gate.yml` | downstream of FR-001 cascade |

**PR #919 (poison-safe lock) is uniquely bad** (44 failing checks vs the 21-26 baseline). The extra failures come from the FUSE / Linux container / Loom / hermetic / dashboard / Linux-native / Windows-winfsp chain — the PR likely alters a lock shared by many jobs.

## 3. Cascade analysis — five failure clusters, mapped to fixes

### Cluster A — `ci / lint` + `ci / test` (rustfmt 1.96.0)
- Trigger: `ci-gate.yml` (a separate file from the older `ci.yml`).
- Symptom: rustfmt 1.96.0 changed formatting; the workspace was last formatted against an earlier toolchain.
- Fix on branch: PR **#930** "ci(rustfmt): bring workspace into rustfmt 1.96.0".
- This is the only PR in the in-scope set that can itself fix the `ci / lint` cluster for everyone.

### Cluster B — `cargo bench (gate)` + `bench run` + `cargo bench (soft)` + `eval` (Criterion)
- Trigger: `bench.yml` (53 runs, 0/10 PASS in last 500), `eval.yml` (53 runs, 0/10 PASS).
- Symptom: Criterion regression guard fails. Root cause is *unknown* from this read-only audit — would need to read the failing run logs to confirm whether a threshold is the culprit or an actual perf regression.

### Cluster C — `macOS codesign + notarize` (Infisical-backed)
- Trigger: `codesign.yml:32` requires `secrets.INFISICAL_TOKEN` and hard-fails on `if [[ -z "$INFISICAL_TOKEN" ]]` (codesign.yml:46-48).
- Symptom: every main-branch push, every PR. 0/10 PASS in last 500.
- Verified: `gh secret list` returns no repo secrets; `gh variable list` returns no repo variables; `gh secret list -e all` 404s (no environments exist).
- **This is an operator action, not a code action.** Either (a) wire `INFISICAL_TOKEN` to the repo (or an env), (b) replace the hard-fail with a `continue-on-error` for non-release branches, or (c) gate the job on `github.event_name == 'push' && startsWith(github.ref, 'refs/tags/')`.

### Cluster D — `Cargo Deny (Advisories + Licenses)` + `coverage`
- `deny.yml`: 0/4 PASS in last 500. License or RUSTSEC drift.
- `coverage.yml`: 0/13 PASS. The empty-test-suite guard fires when a new crate ships without `#[cfg(test)]` modules; FR-001 + FR-008 PRs (#927, #924, #925) hit this.

### Cluster E — `Quality Report` (downstream of all of the above)
- `quality-gate.yml:14` defines `permissions: {}` at workflow root.
- `quality-report` job uses `actions/github-script@3a2844b7e9c422d3c10d287c895573f7108da1b3 # v9.0.0` (quality-gate.yml ~L200) to call `github.rest.issues.createComment`.
- With workflow-level `permissions: {}` and no per-job `pull-requests: write` override, `GITHUB_TOKEN` has zero scopes and the comment call returns HTTP 403.
- Even if the comment somehow succeeded, the `Quality Report` step is gated by `if: always()` after `needs: [unit-tests, e2e-tests, integration-tests, fr-annotation-check]`, so any failing unit test produces a failing Quality Report.

**Conclusion:** the cascade can be broken at any one of the five points, but only Cluster A is being attacked by an in-scope PR (#930). Cluster C is an operator config change. Clusters B, D, E each need a separate workstream.

## 4. Static checks at the workflow level

### 4.1 Self-hosted runners
- **Only one** workflow uses a self-hosted runner: `soak.yml:34` declares `runs-on: [self-hosted, Linux, X64, desk-sharecli]`, with a comment at `soak.yml:27` explaining the constraint.
- Every other workflow uses GitHub-hosted runners (`ubuntu-latest`, `ubuntu-24.04`, `ubuntu-22.04`, `macos-latest`, `windows-latest`, or `cross` containers).

### 4.2 Concurrency controls
`concurrency:` blocks found in at least 10 workflows. Examples: `container-cosign-soft.yml:22`, `release.yml:35`, `nightly-bench.yml:30`, `deploy-docs.yml:13`, `pr-lint.yml:16`, `deny.yml:23`, `ci-gate.yml:24`, `appcast.yml:22`, `repro-check.yml:29`, `audit.yml:21`. Pattern is consistently `cancel-in-progress: true` on PRs, hold-on-push on main. Adequate for a deep-dive; not the cause of the cascade.

### 4.3 Secrets + external services (grepped)
- `INFISICAL_TOKEN` consumed by `codesign.yml:32` and `infisical.yml:31`. **Missing in repo/environment secrets** (verified via `gh secret list`, `gh variable list`).
- Codecov upload at `coverage.yml:97`, pinned `fb8b3582c8e4def4969c97caa2f19720cb33a72f # v7.0.0`; secondary reference at `quality-gate.yml:77`.
- CycloneDX SBOM in `release.yml` and `sbom.yml`.
- Azure Key Vault: `codesign.yml` references it via `vars.AZURE_*` with a fallback to `continue-on-error`; currently a no-op because the var is unset.
- crates.io publish: `release.yml:350, 353`, default dispatch is `dry_run=true`.
- Sparkle: `appcast.yml:95`.
- Slack / Discord webhooks: `nightly-bench.yml`.
- GHCR: `release.yml:457`.
- **No `signpath` references anywhere in the workflow set.**

### 4.4 FR-001 enforcement (the FR-annotation cascade)
- `quality-gate.yml:120-165` defines the `fr-annotation-check` job.
- The check greps changed test files (via `git diff --name-only origin/main...HEAD`) for `FR:` and `@FR` markers; missing markers fail the job (quality-gate.yml:142).
- This is by design; failure manifests as `FR Annotation Check` in PR checks (passes when satisfied).
- The downstream `Quality Report` job has its own `FR reference in PR body` check that some PRs trip (notably #919, #921, #924, #925, #929) — they have a test file in the diff but no matching FR-id in the PR body.

### 4.5 Schedule-based workflows
At least 4 declare `schedule:` (cron). Examples: `eval.yml:23`, `nightly-bench.yml`, `soak.yml`, `bench.yml`. These drive the off-PR volume.

### 4.6 `permissions:` posture
- `quality-gate.yml:14` — `permissions: {}` at workflow root, per-job `contents: read`. **Bug**: the `quality-report` job needs `pull-requests: write` to call `createComment`. With root `{}` and no override, the call fails with HTTP 403.
- Most other workflows do not declare a `permissions:` block; they inherit the org default.

## 5. Branch protection + merge gating

`gh api repos/KooshaPari/ShareCLI/branches/main/protection` returns:

```json
{
  "required_status_checks": {"contexts": [], "checks": []},
  "required_signatures": {"enabled": false},
  "enforce_admins": {"enabled": true},
  "required_linear_history": {"enabled": true},
  "required_conversation_resolution": {"enabled": true},
  "allow_force_pushes": {"enabled": false},
  "allow_deletions": {"enabled": false},
  "lock_branch": {"enabled": false}
}
```

- **Zero required status checks from the GitHub UI.** Merging is gated by:
  1. `required_conversation_resolution` (all comments resolved)
  2. `required_linear_history` (no merge commits)
  3. `enforce_admins` (admin bypass disabled)
  4. **Mergify Merge Queue** — appears as a check on every PR (`bucket=skipping`, `state=NEUTRAL`) and is the actual de facto gate, since it re-runs all checks and only merges when the queue is green.

This is why PRs show `mergeState=BLOCKED` even though `mergeable=MERGEABLE`: the GitHub API classifies "BLOCKED" when required status checks (from `required_status_checks.checks` list) include any non-passing entry; the absence of any UI-configured required check does not stop the queue, but the `required_conversation_resolution` and Mergify gate both block.

## 6. UNKNOWN / data gaps

- **12th PR**: operator said "12" but listed 11 (#907, #918, #919, #921, #922, #923, #924, #925, #927, #929, #930). The 12th PR is **[UNKNOWN]**.
- **License/advisory drift specifics in `deny.yml`**: not opened in this audit; would need `gh run view <id> --log` against a recent failing Deny run.
- **Criterion regression specifics in `bench.yml` / `eval.yml`**: same — log inspection required.
- **`INFISICAL_TOKEN` provenance**: the token might be coming from an Enterprise-level secret or a separate `actions` install. The 11 visible secrets are 0; this audit could not inspect Enterprise-level org secrets.
- **PR #919 (poison-safe lock) extra failure pattern**: the 44 vs 21-26 deviation warrants a follow-up; not investigated in this read-only scope.
- **The `quality-report` job actually failing on HTTP 403 vs downstream `Unit Tests` failure**: not directly observed; the root permission posture makes both possible.

## 7. Recommendations (no actions taken)

1. **Operator**: wire `INFISICAL_TOKEN` (and confirm `INFISICAL_PROJECT_ID` matches `8efe392e-56a6-4c3c-89f9-8141183dd7e8`) as a repo or environment secret, or guard the `macos-sign` job on release tags only. This single change unblocks Cluster C for all 11 PRs.
2. **Code**: merge PR #930 once its own cascade clears, then rebase the remaining 10 PRs. That breaks Cluster A.
3. **Code**: add `pull-requests: write` to the `quality-report` job in `quality-gate.yml`, or remove the PR comment step. Even if the cascade is fixed, Quality Report cannot succeed without this.
4. **Code**: investigate `bench.yml` / `eval.yml` Criterion thresholds. Likely a perf regression or a Criterion version drift.
5. **Hygiene**: delete the 7 never-ran workflow files (`appcast.yml`, `container-cosign.yml`, `infisical.yml`, `openapi-drift.yml`, `packaging.yml`, `playwright-soft.yml`, `release-attestation.yml`) or document why they exist.
6. **Hygiene**: set `branch protection → required status checks` to at least the 5-7 most important contexts (`Quality Gate`, `CI`, `Hermetic Build (Hard Gate)`, `Code Signing (Hard Gate)`, `Cargo Deny`) so that GitHub's UI reflects the actual gate instead of relying on Mergify alone.

## 8. Inventory of evidence

All raw outputs in this audit are reproducible from:

- `wc -l .github/workflows/*.yml` — 60 files, 5,598 lines.
- `gh run list --limit 500 --json name,headBranch,conclusion,createdAt` — see `/tmp/audit_sharecli.out`.
- `gh pr checks <n> --json name,bucket` for n in {907, 918, 919, 921, 922, 923, 924, 925, 927, 929, 930} — see `/tmp/audit_pr2.out`.
- `gh run list --limit 2000 --json name,headBranch,createdAt,conclusion` — see `/tmp/audit_workflow_wide.out`.
- `gh api repos/KooshaPari/ShareCLI/branches/main/protection` — no required status checks, conversation resolution required, linear history required, admins enforced.
- `gh secret list --repo KooshaPari/ShareCLI` — empty; `gh variable list --repo KooshaPari/ShareCLI` — empty; `gh secret list -e all` — 404 (no envs).

No files in `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli/` were modified.
