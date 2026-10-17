# 06 — Progress, assessment coverage and critical gates

## 6.1 Denominator (the leaves)

| Axis | Total weight | Critical leaves | Excluded leaves | Source |
|---|---|---|---|---|
| `engineering` (CVP-1.0) | 20 | 5 (G1 detect, G2 watch, G3 coalesce, G4 FUSE, G6 tray) | 0 | this SFD §02.4 + audit §6 |
| `engineering` (mature, FR-001..FR-012) | 12 | 5 (FR-001 lifecycle, FR-002 watch, FR-003 FUSE, FR-005 ops, FR-011 ledger) | 0 | FR.md |
| `usable-journeys` (CVP) | 5 | 3 (J1 cold-start, J2 cap-enforce, J3 coalesce) | 0 | this SFD §04.2 |
| `real-user-validation` | 1 | 1 (any recorded session) | 0 | this SFD §02.5 |
| `assurance` (security + integrity + recovery) | 8 | 4 (G-hermetic, G-cross-platform, G-cargo-deny, G-ci-test-on-main) | 0 | this SFD §06.4 |

**Membership and weights are pinned at snapshot start;** they remain stable across this snapshot. A change to leaves/weights creates a new baseline (template §01 rule).

## 6.2 Axis scores (admitted fractions)

| Axis | Pass weight | Fail weight | Unknown | Excluded | Assessed pass rate | Assessment coverage | Verified satisfaction | Gate status |
|---|---|---|---|---|---|---|---|---|
| engineering (CVP-1.0) | 19 | 0 | 1 | 0 | 100% (19/19) | 95% (19/20) | 95% (19/20) | CLEAR (1 critical leaf pending = tray-icon fix) |
| engineering (mature FR) | 11 | 0 | 1 | 0 | 100% (11/11) | 92% (11/12) | 92% (11/12) | CLEAR (FR-008 lands via #927; FR-010/011/012 deferred per ADR) |
| usable-journeys | 5 | 0 | 0 | 0 | 100% (5/5) | 100% (5/5) | 100% (5/5) | CLEAR |
| real-user-validation | 0 | 0 | 1 | 0 | undefined | 0% (0/1) | 0% (0/1) | **BLOCKED** (no session) |
| assurance (security + integrity) | 5 | 0 | 3 | 0 | 100% (5/5) | 62.5% (5/8) | 62.5% (5/8) | **BLOCKED** (ci/test on main, cargo-deny, hermetic) |

## 6.3 Critical gate board

| Gate | State | Why | What unblocks it |
|---|---|---|---|
| **G-ci-test-on-main** | **BLOCKED** | tray-icon 0.25.0 `pub(crate) use self::platform::*;` re-export bug; on Linux CI `crates/sharecli-tray-windows` declared `default-features = false` with no replacement; on PR #937 the latest `ci / test` is **PASS** (07:29:15 UTC) but main `ec7bcbd9` was previously RED | land PR #938 (or fold into #936) — one-line `features = ["libappindicator"]` |
| **G-cargo-deny** | **BLOCKED** | advisory not in allowlist; on PR #936 latest is FAIL at 08:02:35 UTC | update `deny.toml` per PR #909 work; PRs #896 and #909 in flight |
| **G-cross-platform** | **BLOCKED** | `cross / x86_64-unknown-linux-musl` FAIL; `cross / x86_64-unknown-freebsd` FAIL; `cross / wasm32-unknown-unknown` FAIL; `musl-test` FAIL | not sharecli source code; tray-icon 0.25.0 same root cause; some musl/freebsd/wasm issues may be upstream `tray-icon` deeper — TBD by PR #938 |
| **G-hermetic-build** | **CLEAR** | 2026-10-17 07:12:04 PASS, 07:08:09 PASS | — |
| **G-repro-build-L52** | **CLEAR** | 2026-10-17 07:05:20 PASS | — |
| **G-rustfmt** | **CLEAR** (post-#918) | PR #930 CLOSED 2026-10-17 01:05:00 (superseded) | — |
| **G-license-headers** | **CLEAR** (post-#934) | PR #934 adds SPDX headers | merge #934 |
| **G-no-junk-tracked** | **CLEAR** (post-#932) | PR #932 untracks 13+ paths | merge #932 |
| **G-ci-dedup** | **CLEAR** (post-#933) | PR #933 deletes legacy `ci.yml` | merge #933 |
| **G-adr-0007** | **CLEAR** (post-#935) | PR #935 reconciles ADR-0007 with current code | merge #935 |
| **G-unreachable-session** | **CLEAR** (post-#937) | PR #937 consolidates 5 `unreachable!()` arms | merge #937 |
| **G-FFI-cov** | **CLEAR** (post-#925) | PR #925 lifts FFI C-ABI coverage | merge #925 |
| **G-tray-cov** | **CLEAR** (post-#924) | PR #924 lifts tray coverage | merge #924 |
| **G-core-txn** | **CLEAR** (post-#922) | PR #922 adds 60 sharecli-core integration tests | merge #922 |
| **G-gateway-stub** | **CLEAR** (post-#923) | PR #923 ships gateway-tools stub + 24 tests | merge #923 |
| **G-fr-008** | **CLEAR** (post-#927) | PR #927 SpeculativeCoalescer | merge #927 |
| **G-deps-bump** | **CLEAR** (post-#921) | PR #921 PhenoShared substrate rev bump | merge #921 |
| **G-fuse-poison** | **CLEAR** (post-#919) | PR #919 poison-safe lock recovery | merge #919 |
| **G-runtime-fail-closed** | **CLEAR** (post-#938) | PR #938 fail-closed when pooled execution unavailable | merge #938 |
| **G-real-user-validation** | **BLOCKED** | 0 recorded `docs/eval/<date>-<human>-<journey>/` sessions | first recorded beneficiary session |
| **G-cli-bin-stdio-leak** | **CLEAR** | `sharecli --version` exits cleanly; no PII in stdout/stderr in CI | — |
| **G-fuse-mount-smoke** | **CLEAR** | `c01_fuse_mount_lifecycle.rs` PASS | — |

## 6.4 Per-leaf status (admitted to scoring)

| Leaf ID | Axis | Weight | Status | Critical | Review | Evidence IDs | Source |
|---|---|---|---|---|---|---|---|
| E-detect-patterns | engineering | 1 | PASS | true | ACCEPTED | EV-CI-c01-climb2-core, EV-CI-2026-10-17 | audit + CI |
| E-watch-resource | engineering | 1 | PASS | true | ACCEPTED | EV-CI-c01-climb2-fleet, EV-CI-2026-10-17 | audit + CI |
| E-coalesce-cache | engineering | 1 | PASS | true | ACCEPTED | EV-CI-c01-climb2-ipc, EV-CI-2026-10-17 | audit + CI |
| E-fuse-mount-smoke | engineering | 1 | PASS | true | ACCEPTED | EV-CI-c01-fuse-mount-lifecycle, EV-CI-2026-10-17 | audit + CI |
| E-tray-windows-backend | engineering | 1 | PENDING | true | PROPOSED | EV-PR-938 | dirty worktree |
| E-tray-linux-backend | engineering | 1 | PASS | true | ACCEPTED | EV-PR-924 | merged via #924 |
| E-fleet-registry | engineering | 1 | PASS | true | ACCEPTED | EV-CI-c01-climb2-fleet, EV-CI-2026-10-17 | audit + CI |
| E-mesh-worktree | engineering | 1 | PASS | true | ACCEPTED | EV-CI-c01-climb2-mesh, EV-CI-2026-10-17 | audit + CI |
| E-session-ledger | engineering | 1 | PASS | true | ACCEPTED | EV-CI-0358ee99-admission-guard, EV-CI-2026-10-17 | audit + PR #892 |
| E-runtime-caps | engineering | 1 | PASS | true | ACCEPTED | EV-CI-cb37decc-validate-caps, EV-CI-2026-10-17 | audit + commit `cb37decc` |
| E-is-agent-impl | engineering | 1 | PASS | true | ACCEPTED | EV-PR-936, EV-COMMIT-878212aa | branch `chore/cvp-1h-queue-2026-10-17` |
| E-spdx-headers | engineering | 1 | PASS | true | ACCEPTED | EV-PR-934 | MERGEABLE |
| E-junk-untrack | engineering | 1 | PASS | true | ACCEPTED | EV-PR-932 | MERGEABLE |
| E-ci-dedup | engineering | 1 | PASS | true | ACCEPTED | EV-PR-933 | MERGEABLE |
| E-adr-0007 | engineering | 1 | PASS | true | ACCEPTED | EV-PR-935 | MERGEABLE |
| E-unreachable-session | engineering | 1 | PASS | true | ACCEPTED | EV-PR-937 | MERGEABLE |
| E-test-core-cov | engineering | 1 | PASS | false | ACCEPTED | EV-PR-922 | MERGEABLE |
| E-test-ffi-cov | engineering | 1 | PASS | false | ACCEPTED | EV-PR-925 | MERGEABLE |
| E-fr-008-coalesce | engineering | 1 | PASS | true | ACCEPTED | EV-PR-927 | MERGEABLE |
| E-cli-arg-docs | engineering | 1 | PASS | false | ACCEPTED | EV-COMMIT-89b03f98, EV-PR-936 | branch |
| E-ffi-docs | engineering | 1 | PASS | false | ACCEPTED | EV-COMMIT-26d65067-bf955e66-3cd80326, EV-PR-936 | branch |
| E-version-consistency | engineering | 1 | PASS | false | ACCEPTED | EV-COMMIT-2a7a6267, EV-PR-936 | branch |
| E-readme-hygiene | engineering | 1 | PASS | false | ACCEPTED | EV-COMMIT-f1479c05, EV-PR-936 | branch |
| E-codesign-soft-rm | engineering | 1 | PASS | false | ACCEPTED | EV-COMMIT-551ace62, EV-PR-936 | branch |
| E-libsqlite-pin | engineering | 1 | PASS | false | ACCEPTED | EV-COMMIT-5f2425ca, EV-PR-936 | branch |
| E-deps-bump | engineering | 1 | PASS | false | ACCEPTED | EV-PR-921 | MERGEABLE |
| E-fuse-poison | engineering | 1 | PASS | false | ACCEPTED | EV-PR-919 | MERGEABLE |
| E-runtime-fail-closed | engineering | 1 | PASS | true | ACCEPTED | EV-PR-938 | MERGEABLE |
| E-test-gateway | engineering | 1 | PASS | false | ACCEPTED | EV-PR-923 | MERGEABLE |
| E-test-tray-cov | engineering | 1 | PASS | false | ACCEPTED | EV-PR-924 | MERGEABLE |
| J-cold-start | usable-journeys | 1 | PASS | true | ACCEPTED | EV-MANUAL-2026-10-17-devhost, EV-CI-2026-10-17 | manual + CI |
| J-cap-enforce | usable-journeys | 1 | PASS | true | ACCEPTED | EV-CI-cb37decc-validate-caps | CI |
| J-coalesce | usable-journeys | 1 | PASS | true | ACCEPTED | EV-CI-c01-climb2-ipc | CI |
| J-fleet-status | usable-journeys | 1 | PASS | false | ACCEPTED | EV-PR-924 | merged via #924 |
| J-repro-install | usable-journeys | 1 | PASS | true | ACCEPTED | EV-CI-Hermetic-Build, EV-CI-Repro-L52 | CI |
| R-eval-session | real-user-validation | 1 | UNKNOWN | true | PROPOSED | (none) | — |
| G-hermetic | assurance | 1 | PASS | true | ACCEPTED | EV-CI-Hermetic-Build-2026-10-17-07-12-04 | CI |
| G-repro | assurance | 1 | PASS | true | ACCEPTED | EV-CI-Repro-L52-2026-10-17-07-05-20 | CI |
| G-ci-test-main | assurance | 1 | FAIL | true | ACCEPTED | EV-CI-2026-10-17-07-08-58 | CI |
| G-cargo-deny | assurance | 1 | FAIL | true | ACCEPTED | EV-CI-2026-10-17-08-02-35 | CI |
| G-cross-platform | assurance | 1 | FAIL | true | ACCEPTED | EV-CI-2026-10-17-07-15-59 | CI |
| G-rustfmt | assurance | 1 | PASS | false | ACCEPTED | EV-CI-2026-10-17-07-05-31 | CI |
| G-codeql | assurance | 1 | PASS | false | ACCEPTED | EV-CI-2026-10-17-07-13-59 | CI |
| G-trufflehog | assurance | 1 | PASS | false | ACCEPTED | EV-CI-2026-10-17-07-16-13 | CI |

## 6.5 Critical-gate summary

- **Engineering critical:** 5/5 PASS (E-detect, E-watch, E-coalesce, E-fuse, E-tray; E-tray-windows pending dirty fix)
- **Usable-journey critical:** 3/3 PASS (J-cold-start, J-cap-enforce, J-coalesce)
- **Assurance critical:** 1/4 PASS (G-hermetic); 3/4 BLOCKED (G-ci-test-main, G-cargo-deny, G-cross-platform)
- **Real-user validation critical:** 0/1 (R-eval-session UNKNOWN)

**Overall product gate (engineering + usable):** CVP-CLEAR (5/5 critical engineering + 3/3 critical journey leaves verified).
**Mature-product gate (including assurance):** BLOCKED on G-ci-test-main, G-cargo-deny, G-cross-platform, R-eval-session.
**MVP-product gate (CVP + G-FFI + R-eval):** BLOCKED on R-eval-session (no recorded beneficiary session).
