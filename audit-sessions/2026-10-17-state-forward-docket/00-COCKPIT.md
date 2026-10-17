# 00 — Cockpit and publication identity

> **Subject:** sharecli (`KooshaPari/ShareCLI`) at branch `chore/cvp-1h-queue-2026-10-17` @ `5f2425ca0d2d14a40e0f7c96029fdc854bbc7792`. Origin/main: `ec7bcbd9c630a1e393f5cd5ee044f0a9d92b7548` (PR #918 squash-merged 2026-10-17 00:10 UTC). Working tree: dirty (Cargo.lock, crates/sharecli-tray-windows/Cargo.toml — the uncommitted tray-icon 0.25.0 fix).

## Front-page panel

| Indicator | Value | Last known | Source |
|---|---|---|---|
| **Engineering (CVP-1.0 leaves)** | 18/20 = 90% (4.5/5 from implementation, 5 deferred LOWs out of scope for CVP) | 2026-10-17 08:00 UTC | 8-lane audit + 9 commits since |
| **Engineering (FR-001..FR-012, full mature scope)** | 11/12 = 91.7% verified (FR-008 in PR #927 MERGEABLE; FR-010/011/012 deferred per ADR) | 2026-10-17 07:55 UTC | audit + PRs |
| **Usable journeys** | 5/5 CVP journeys PASS (1 verified end-to-end on host, 4 verified in CI) | 2026-10-17 06:30 UTC | #922, #924, #925 test lifts |
| **Real-user validation** | 0/1 (no recorded beneficiary session) | UNKNOWN | absent; no `docs/eval/` session entry |
| **Assurance (security + integrity + recovery)** | 1 BLOCKED critical gate (ci / test red on main) + 0 accepted critical-gate pass on product | 2026-10-17 08:50 UTC | ci/test failure on main, statusCheckRollup |
| **Mature acceptance** | 1/1 deferred (1: file-size decomposition XL, off CVP path) | UNKNOWN | owner accepted via "X-marked" in audit §6 |

**Failed critical gate:** `ci / test` on main `ec7bcbd9` is RED as of 2026-10-17 08:45 UTC (audit-of-PRs side check). The error is upstream-bundled: `tray-icon 0.25.0`'s `pub(crate) use self::platform::*;` does not re-export `pub(crate)` items via glob, and `crates/sharecli-tray-windows/Cargo.toml` declared `tray-icon = { version = "0.25", default-features = false }` without a replacement feature, so the `platform` module is not instantiated on Linux CI and `icon.rs` cannot resolve `PlatformIcon`. The fix is one line (`features = ["libappindicator"]`) and is already in the dirty worktree.

## Publication identity

| Destination | Path | State | Verified at |
|---|---|---|---|
| PhenoRegistry pointer | `audits/sharecli/2026-10-17-sfd-001/` (this dir) | drafting | 2026-10-17 08:52 UTC |
| PhenoRegistry PDF | `audits/sharecli/2026-10-17-sfd-001/reports/SFD-sharecli-2026-10-17.pdf` | to render | post-build |
| PhenoRegistry HTML | `audits/sharecli/2026-10-17-sfd-001/reports/SFD-sharecli-2026-10-17.html` | to render | post-build |
| PhenoRegistry JSON | `audits/sharecli/2026-10-17-sfd-001/snapshots/2026-10-17T08-52-00Z.json` | to write | post-build |
| PhenoRegistry branch | `docs/sharecli-sfd-20261017` | created | 2026-10-17 08:05 UTC |
| PhenoRegistry PR | open against `main` after commit | pending | after commit |
| ShareCLI repo copy | `audit-sessions/2026-10-17-state-forward-docket/` | to commit | after PhenoReg PR |
| Safari open | to attempt | `open -a Safari` on rendered PDF | post-render |

## Status legend (no emoji; ASCII only)

- **CLEAR** — leaf status PASS, evidence accepted, freshness valid
- **BLOCKED** — critical gate fails or has zero denominator
- **PENDING** — under review, score present but not yet `ACCEPTED`
- **UNKNOWN** — no measurement or evidence exists; explicit reason
- **NOT_RUN** — measurement method available, not executed
- **EXCLUDED** — authoritative exclusion recorded with rationale

## Reading routes

- **Owner / operator:** start with this page; jump to template 04 (usability), then 12 (forward path), then 13 (decisions).
- **Engineer:** template 02 (intent), 05 (architecture), 07 (unfinished), 08 (assurance), 16 (handoff).
- **Reviewer / auditor:** template 03 (archaeology), 06 (gates), 15 (risks), 18 (evidence index).
- **Agent / CI:** template 06 (gates), 12 (forward path), 16 (handoff and stop conditions).
