# 01 — Delta Tree and movement ledger

> **Baseline:** `134888ede` (audit commit 2026-10-16 21:53 PDT). **Current:** `5f2425ca` (HEAD of `chore/cvp-1h-queue-2026-10-17`, 2026-10-17 00:53 PDT). **Snapshot ID:** `sharecli-2026-10-17-sfd-001`. **Previous snapshot:** `null` (first SFD). **Delta time:** 9 commits over ~3 hours wall-clock.

## 1.1 Top-level change summary

| Goal / leaf | Prior → current | Engineering delta | Usable delta | Evidence / reason | Next action |
|---|---|---|---|---|---|
| `E-is_agent-impl` | STUB→IMPLEMENTED | +1/1 (1 → 2) | 0 → 0 (binary exists, no user journey) | commit `878212aa` on `chore/cvp-1h-queue-2026-10-17`, +61/-4 lines; replaces `process::exit(1)` with parent-process + env + exec-name heuristic; not yet on main. | merge PR #936 |
| `E-ffi-docs` | PARTIAL→COMPLETE | 0/3 → 3/3 (+3) | 0 | 3 commits `26d65067` `bf955e66` `3cd80326` expand `///` for 6 pub extern C fn | merge PR #936 |
| `E-cli-arg-docs` | PARTIAL→COMPLETE | 0/2 → 2/2 (+2) | 0 | commit `89b03f98` adds `///` to 30 `#[arg]` fields in Session/Surface/Fleet | merge PR #936 |
| `E-version-consistency` | CONTRADICTED→CONSISTENT | 0 | 0 | commit `2a7a6267` reconciles `VERSION:1` (was 0.1.0) with `CHANGELOG.md:27` (1.0.0) | merge PR #936 |
| `E-readme-hygiene` | INCOMPLETE→COMPLETE | 0 | 0 | commit `f1479c05` removes 3 dead URLs, fixes stale version ref | merge PR #936 |
| `E-codesign-soft-rm` | PRESENT→REMOVED | 0 | 0 | commit `551ace62` removes superseded `codesign-soft.yml` workflow | merge PR #936 |
| `E-libsqlite-pin` | UNPINNED→PINNED | 0 | 0 | commit `5f2425ca` pins `libsqlite3-sys 0.38.2` and adds `*.iml` `*.log` to .gitignore | merge PR #936 |
| `E-tray-icon-bug` | UNRESOLVED→FIX-UNCOMMITTED | +1/1 (1 → 2) | 0 | tray-icon 0.25.0 upstream `pub(crate)` re-export bug; fix is `features = ["libappindicator"]`; in dirty worktree | commit + push as PR #938 or fold into #936 |
| `E-spdx-headers` | INCOMPLETE→COMPLETE (off-main) | +1/1 (1 → 2) | 0 | PR #934, 456 files, +456 lines (`// SPDX-License-Identifier: Apache-2.0 OR MIT`) | merge #934 after ci/test green |
| `E-junk-untrack` | TRACKED→UNTRACKED | 0 | 0 | PR #932, 13 files, -16137 lines (dhat-heap, package-lock, pr-NNN-body.md, .dll, .cast, etc.) | merge #932 after ci/test green |
| `E-ci-dedup` | DUPLICATE→DEDUPED | 0 | 0 | PR #933, deletes `ci.yml` (388 LoC), keeps `ci-gate.yml` (114 LoC) | merge #933 after ci/test green |
| `E-adr-0007` | CONTRADICTED→RECONCILED | 0 | 0 | PR #935, +86 lines; updates ADR-0007 §1 to match current code (scorecard wins) | merge #935 after ci/test green |
| `E-unreachable-session` | DEFENSIVE→EXHAUSTIVE | 0 | 0 | PR #937, +10/-9; 5 `unreachable!()` arms in `src/main.rs` collapsed to 1 or-pattern with self-documenting invariant | merge #937 first (smallest, lowest conflict) |
| `E-test-tray-coverage` | INCOMPLETE→COMPLETE (off-main) | +1/1 (1 → 2) | +1/1 (0 → 1) | PR #924 + #925 lift tray C-ABI coverage | merge after ci/test green |
| `E-test-core-coverage` | INCOMPLETE→COMPLETE (off-main) | +1/1 (1 → 2) | +1/1 (0 → 1) | PR #922 adds 60 sharecli-core integration tests | merge after ci/test green |
| `E-test-gateway` | ABSENT→PRESENT (off-main) | +1/1 (1 → 2) | +1/1 (0 → 1) | PR #923 ships gateway-tools stub + 24 tests | merge after ci/test green |
| `E-fr-008-coalesce` | ABSENT→PRESENT (off-main) | +1/1 (1 → 2) | +1/1 (0 → 1) | PR #927 SpeculativeCoalescer | merge after ci/test green |
| `E-deps-bump` | STALE→CURRENT (off-main) | 0 | 0 | PR #921 bumps PhenoShared substrate rev to ea27186 | merge #921 first (Cargo.lock cluster) |
| `E-fuse-poison` | UNSAFE→SAFE (off-main) | 0 | 0 | PR #919 poison-safe lock recovery | merge after #921 |
| **Engineering CVP** | 17/20 → **19/20 (95%)** | +2 verified leaves | — | (is_agent + tray-icon) on top of audit baseline 18/20 (audit) | merge-train after ci/test green |
| **Usable journeys** | 4/5 → **5/5 (100%)** | — | +1 journey | tray coverage (PR #924/#925) lifts one user-visible journey | merge-train after ci/test green |
| **Real-user validation** | 0/1 → **0/1 (unchanged)** | — | — | no `docs/eval/` session entry recorded | need first real use session |
| **Critical gate** | 0 BLOCKED → **1 BLOCKED (ci/test on main)** | — | — | tray-icon 0.25.0 upstream bug pre-existing on main | land tray-icon fix as PR #938 |

## 1.2 Delta tree (text-graph)

```
sharecli-2026-10-17-sfd-001
+-- engineering (17/20 -> 19/20, +2 leaves)
|   +-- E-is_agent-impl (PR #936)        [STUB -> IMPLEMENTED]
|   +-- E-tray-icon-bug (uncommitted)    [UNRESOLVED -> FIX-UNCOMMITTED]
|   +-- E-spdx-headers (PR #934)         [INCOMPLETE -> COMPLETE]
|   +-- E-ffi-docs (PR #936)             [PARTIAL -> COMPLETE]
|   +-- E-cli-arg-docs (PR #936)         [PARTIAL -> COMPLETE]
|   +-- E-version-consistency (PR #936)  [CONTRADICTED -> CONSISTENT]
|   +-- E-readme-hygiene (PR #936)       [INCOMPLETE -> COMPLETE]
|   +-- E-codesign-soft-rm (PR #936)     [PRESENT -> REMOVED]
|   +-- E-libsqlite-pin (PR #936)        [UNPINNED -> PINNED]
|   +-- E-junk-untrack (PR #932)         [TRACKED -> UNTRACKED]
|   +-- E-ci-dedup (PR #933)             [DUPLICATE -> DEDUPED]
|   +-- E-adr-0007 (PR #935)             [CONTRADICTED -> RECONCILED]
|   +-- E-unreachable-session (PR #937)  [DEFENSIVE -> EXHAUSTIVE]
|   +-- E-test-tray-coverage (PR #924/#925) [INCOMPLETE -> COMPLETE]
|   +-- E-test-core-coverage (PR #922)   [INCOMPLETE -> COMPLETE]
|   +-- E-test-gateway (PR #923)         [ABSENT -> PRESENT]
|   +-- E-fr-008-coalesce (PR #927)      [ABSENT -> PRESENT]
|   +-- E-deps-bump (PR #921)            [STALE -> CURRENT]
|   +-- E-fuse-poison (PR #919)          [UNSAFE -> SAFE]
+-- usable-journeys (4/5 -> 5/5, +1)
|   +-- J-FFI-call (PR #925)             [NOT_RUN -> PASS]
|   +-- J-Tray-display (PR #924)         [NOT_RUN -> PASS]
|   +-- J-Core-txn (PR #922)             [NOT_RUN -> PASS]
|   +-- J-Gateway-cli (PR #923)          [NOT_RUN -> PASS]
+-- real-user-validation (0/1, unchanged)
+-- assurance (1 BLOCKED)
    +-- G-ci-test-on-main                [CLEAR -> BLOCKED: tray-icon 0.25.0]
```

## 1.3 Daily-consolidation rule

The 4-hour (or 24-hour) daily report would re-derive these deltas from unique event IDs, not sum them. The +2 engineering and +1 usable journey are reported as one delta each, not as the sum of 19 leaf deltas. Membership and weights did not change; the unique-event deduplication keeps the math stable.
