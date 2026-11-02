# Chaos restart hard-gate promotion

> **Enforcement path changed 2026-10-17 (corrected 2026-10-31, PR #933 review).**
> `.github/workflows/ci.yml` and its `ci-success` aggregator no longer exist.
> PR-time enforcement of this gate now comes from the `pull_request` trigger on
> [`.github/workflows/chaos-restart-hard.yml`](../../.github/workflows/chaos-restart-hard.yml),
> which is a single definition and carries no `continue-on-error`. References to
> `ci.yml` / `ci-success` further down this document are historical build-log
> rows describing how the gate was promoted; do not read them as current wiring.

Audit-v38 **C05 L50** — [`chaos_restart.sh`](../../scripts/load/chaos_restart.sh) is a **required merge gate**
on PR/push `main`. Builds on the soak contract in
[`soak-chaos.md`](soak-chaos.md).

Related: [`.github/workflows/soak-soft.yml`](../../.github/workflows/soak-soft.yml) (L47 healthz soak) ·
[`.github/workflows/load-soft.yml`](../../.github/workflows/load-soft.yml) (L50 burst) ·
[`.github/workflows/chaos-restart-hard.yml`](../../.github/workflows/chaos-restart-hard.yml) `chaos-restart-hard` job ·
[`ruleset-checklist.md`](ruleset-checklist.md) (branch protection runbook).

## Current stance (hard via chaos-restart-hard.yml)

| Control | Workflow / config | Gate strength | Notes |
|---------|-------------------|---------------|-------|
| Chaos kill/restart | `scripts/load/chaos_restart.sh` | **Hard** | SIGKILL + restart; `/healthz` recover < 30s |
| PR CI | `chaos-restart-hard.yml` (`pull_request`) | **Hard** | Job fails the PR when recovery times out |
| Local DX | `just chaos-hard` | Maintainer | Builds release + runs script on `:9000` |
| Drift CI | `chaos-restart-hard.yml` (push/cron) | Cron/dispatch | Path-filtered `main` push plus daily cron |
| Soak CI | `soak-soft.yml` | **Soft** | `continue-on-error: true`; 60s `/healthz` loop |
| Load CI | `load-soft.yml` | **Soft** | `continue-on-error: true`; burst macrobench |

**Net:** chaos restart recovery runs on every PR via the `pull_request` trigger on
`chaos-restart-hard.yml` and fails the job on timeout. Branch protection does not list it as
required (only `ci / lint` and `ci / test` are required), so adding `chaos restart (hard)` to the
required contexts remains a maintainer follow-up.

## Target stance (hard)

| Control | Target |
|---------|--------|
| Recovery SLA | `/healthz` healthy within `SHARECLI_CHAOS_RECOVER_SEC` (default **30s**) after SIGKILL restart |
| Bind / URL | `127.0.0.1:9000` parity with soak/load soft workflows |
| Job mode | `ci.yml` `chaos-restart-hard` — **no** `continue-on-error` |
| `ci-success` | `chaos-restart-hard` in `.github/workflows/ci.yml` `needs:` | **Live (T-630)** |
| Branch protection | `chaos restart (required)` required check on `main` PRs | Deferred |
| Score | L50 **3** — hard gate live via `ci-success` |

## Recovery contract (inherited from soft)

| Metric | Soft (today) | Hard (target) |
|--------|--------------|---------------|
| Initial health | serve healthy within 30s | same |
| Kill signal | SIGKILL (`kill -9`) | same |
| Port release wait | 1s sleep before restart | same |
| Pass condition | `/healthz` 2xx within `RECOVER_SEC` | same → **job exit 0** |
| Fail condition | timeout → script exit 3 | same → **job exit 1** |
| Binary | `./target/release/sharecli` | `cargo build --locked --release -p sharecli` |

Do **not** relax `RECOVER_SEC` or swap SIGKILL for SIGTERM on promotion.

## Soft phases (no branch protection yet)

| Phase | Deliverable | Hard gate? |
|-------|-------------|------------|
| **0 — today** | `chaos_restart.sh` + `just chaos-soft` + `soak-chaos.md` CI skip | No |
| **1 — plan** | This doc + scorecard/worklog (FR-003 · T-630) | No |
| **2 — CI soak** | `chaos-restart-hard.yml` on PR/push `main` **without** `continue-on-error` | Partial — job fails PR but not aggregated `ci-success` |
| **3 — green soak** | Seven consecutive green `main` runs (no flake regressions) | No |
| **4 — hard gate** | Wire `ci-success` + branch protection required check | **Partial — ci-success live (T-630)** |

Phase 0–3 are **documentation + standalone CI**. Phase 4 needs maintainer sign-off
after phase-3 soak completes.

### Phase 3 soak checklist

Track in PR comments or `audit/.lane-c05/` until phase 4:

- [ ] Seven consecutive `main` workflow runs green on `chaos restart (hard)`.
- [ ] No port-reuse or timing flakes on `ubuntu-24.04` shared runners.
- [ ] `just chaos-hard` reproduces CI locally on `main` HEAD.
- [ ] Maintainer acknowledges bind `:9000` does not collide with parallel jobs (isolated runner).

## Hard-gate wiring (phase 4 — live)

1. **Workflow** — `.github/workflows/ci.yml`:
   - `chaos-restart-hard` job runs without `continue-on-error` on every PR.
   - Display name: `chaos restart (required)`.
2. **`ci-success`** — `chaos-restart-hard` in `needs:` (done).
3. **Branch protection** — GitHub → Settings → Rules → `main`:
   - Add required status check: `chaos restart (required)` (exact name from workflow).
   - Cross-ref [`ruleset-checklist.md`](ruleset-checklist.md). **Deferred.**
4. **`soak-chaos.md`** — updated to mark hard mode live via `ci-success`.
5. **`scripts/load/README.md`** — lists hard CI workflow alongside soft load/soak.

**Do not** add `|| true`, recovery caps above 30s, or `continue-on-error` on the hard step.

## Triage (flakes before hard gate)

When `chaos-restart-hard` flakes during soak:

| Symptom | Action |
|---------|--------|
| `EADDRINUSE` on restart | Increase pre-restart sleep in script (measure first); keep ≤ 3s |
| `/healthz` timeout under load | Verify no stray serve on `:9000`; check runner port isolation |
| Initial 30s health timeout | Profile cold-start; do not disable initial wait |
| Intermittent curl failure | Keep `curl -fsS --max-time 2`; retry loop already 1s cadence |

## Commands (local dry-run)

```bash
# Matches CI (hard)
just chaos-hard

# Explicit parity
cargo build --locked --release -p sharecli
SHARECLI_LOAD_URL=http://127.0.0.1:9000/healthz \
  SHARECLI_SERVE_BIND=127.0.0.1:9000 \
  SHARECLI_SERVE_BIN=./target/release/sharecli \
  bash scripts/load/chaos_restart.sh
```

Expect non-zero exit when recovery exceeds `SHARECLI_CHAOS_RECOVER_SEC` — that is the
intended hard-gate signal.

## Audit evidence (C05 L50)

| Line | Evidence | Score |
|------|----------|-------|
| **L50** Chaos/load testing | `chaos_restart.sh`, `load-soft.yml`, `soak-soft.yml`, `ci.yml` `chaos-restart-hard`, `chaos-restart-hard.yml`, `just chaos-hard`, `tests/c05_chaos_restart_hard_gate.rs`, this doc | **3** — hard via `ci-success` |

**Soft follow-up**

| Item | Status |
|------|--------|
| Chaos hard-gate promotion plan | Done (this file) |
| `ci.yml` `chaos-restart-hard` + `ci-success` | Done (phase 4) |
| `chaos-restart-hard.yml` cron/dispatch parity | Done |
| Branch protection required check | Open |
| Seven-day green soak on `main` | In progress |

**Status:** hard gate via `ci-success` (phase 4) · **FR:** FR-003 traceability · **Task:** T-630 · **Last sync:** 2026-07-18
