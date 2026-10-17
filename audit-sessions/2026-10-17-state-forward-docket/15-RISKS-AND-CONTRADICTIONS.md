# 15 — Risks, contradictions, and reconciliation

## 15.1 Risks (ranked by severity)

| # | Risk | Severity | Probability | Mitigation | Owner | Notes |
|---|---|---|---|---|---|---|
| 1 | **Operator has no recorded beneficiary session** | HIGH (for MVP) | HIGH | operator runs a 1-hour session and writes `docs/eval/<date>-<human>-<journey>/` | operator | this is the MVP gate |
| 2 | **tray-icon 0.25.0 `pub(crate)` re-export bug** | HIGH (for v1.0 ci/test on main) | ACTIVE | PR #938; or `features = ["libappindicator"]` on tray-icon 0.25 | sharecli engineer | already drafted |
| 3 | **cargo-deny advisory not allowlisted** | MEDIUM | ACTIVE | update `deny.toml` or upgrade tray-icon | sharecli engineer | PRs #896, #909 in flight |
| 4 | **Cross-platform (musl/freebsd/wasm) failures** | MEDIUM | HIGH | likely upstream `tray-icon`; bump to 0.27.x in v1.1 | sharecli engineer | not for v1.0 |
| 5 | **ADR-0007 contradicts `COMPREHENSIVE_AUDIT_SCORECARD.md`** | MEDIUM | ACTIVE | PR #935 reconciles | sharecli architect | already drafted |
| 6 | **`cargo bench (gate)` perf regression on `healthz`** | MEDIUM | ACTIVE | investigate; gate is BLOCKING | sharecli engineer | not yet investigated |
| 7 | **`coverage` workflow FAIL** | MEDIUM | ACTIVE | lift from 64% to 80% via PRs #922, #924, #925 (MERGEABLE) | sharecli engineer | in progress |
| 8 | **`phenoAI` Hermes imports in PhenoRegistry** | MEDIUM | LOW | re-run importer if drift appears | PhenoRegistry ops | not blocking sharecli |
| 9 | **DCO / GPG signed commits** | LOW (soft gate) | ACTIVE | operator provisions GPG key; or remain soft-fail | operator | soft gate; not blocking |
| 10 | **`crates/spawn-core` ownership undecided** | LOW | ACTIVE | document as frozen in v1.0; move in v1.1 | sharecli architect | not blocking v1.0 |
| 11 | **`crates/gateway-tools` not in workspace** | LOW | ACTIVE | PR #923 lands | sharecli engineer | MERGEABLE |
| 12 | **14 files > 1000 LoC** | LOW (acknowledged) | ACTIVE | `BOUNDARY.md` rationale; not for v1.0 | sharecli architect | accepted |
| 13 | **`monitoring.rs` / `pyroscope_stub.rs` are stubs** | LOW (acknowledged) | ACTIVE | honest stubs; not for v1.0 | sharecli architect | accepted |
| 14 | **No prebuilt binary** | LOW (acknowledged) | ACTIVE | source build only; not for v1.0 | sharecli engineer | not blocking v1.0 |
| 15 | **Visual regression transient FAIL** | LOW | ACTIVE | re-run; transient Zig download | sharecli CI | not blocking |
| 16 | **`cliff.toml` `commit.link` template** | LOW | ACTIVE | patch to use `{{ commit.id \| truncate(length=7, end="") }}` + manual URL | sharecli engineer | 30 min |
| 17 | **`sharecli-fleet` 110 pub items facade bloat** | LOW | ACTIVE | facade reorg in v1.1 | sharecli engineer | v1.1 |
| 18 | **`fs_ext.rs` openat vs openat2 syscalls** | LOW | ACTIVE | v1.1 | sharecli engineer | v1.1 |
| 19 | **`C7 hypervisor.rs` single-syscall optimisation** | LOW (1.6% perf headroom) | ACTIVE | v1.1 | sharecli engineer | v1.1 |
| 20 | **Tracera org chart stale for sharecli** | LOW | ACTIVE | operator adds | operator | not blocking v1.0 |

## 15.2 Contradictions (where two sources disagree)

| ID | Source A | Source B | Authority wins | Evidence |
|---|---|---|---|---|
| C1 | ADR-0007 §1: `sharecli-core` Tier A pure, no infra deps | `COMPREHENSIVE_AUDIT_SCORECARD.md` C00 row 3: `sharecli-core` depends on ipc/fleet/fuse | **Code wins** (PR #935 reconciles ADR) | code, audit, `Cargo.toml` |
| C2 | `VERSION:1` (was 0.1.0, then 0.8.0 after `2a7a6267`) | `CHANGELOG.md:27` (1.0.0 release tag) | **Both are correct** (reconciled at `2a7a6267`: 0.8.0 in VERSION, 1.0.0 in CHANGELOG) | commit `2a7a6267` |
| C3 | `BOUNDARY.md`: "shared CLI process manager" | `PRD.md`: "agent runtime / hypervisor" | **Both correct at different abstraction levels** (BOUNDARY is the cross-project statement; PRD is the mature contract) | `BOUNDARY.md`, `PRD.md` |
| C4 | `crates/spawn-core` on disk in sharecli | ADR-0006: lives in `portage-temp` | **No decision** (defer to v1.1) | audit §4.4 F2 |
| C5 | `crates/gateway-tools` on disk in sharecli | `Cargo.toml [workspace]`: not a member | **PR #923** adds to workspace | PR #923 |
| C6 | `monitoring.rs` exists with dead code | operator intuition: should be deleted | **Honest stub accepted** (audit §3 Theme B) | audit |
| C7 | `sharecli-tray-windows` declared `default-features = false` | tray-icon 0.25.0 Linux requires a feature | **Add `features = ["libappindicator"]`** (PR #938) | PR #938 |
| C8 | audit §4.1: 5 redundant `unreachable!()` arms in `src/main.rs` | operator: defensive is fine | **Consolidate to 1** (PR #937) | PR #937 |
| C9 | audit §4.8 MEDIUM: `sharecli-fleet` 110 pub items is facade bloat | the same 110 items are used by FFI, IPC, mesh | **Accept for v1.0; facade reorg in v1.1** | audit |
| C10 | audit §4.4 F1: 14 files > 1000 LoC | AGENTS.md 500L cap | **`BOUNDARY.md` rationale accepts** | `BOUNDARY.md` |
| C11 | `crates/spawn-core` (Zig) is on disk but `zig build` not exercised | ADR-0006 says `portage-temp` | **No decision** (defer to v1.1) | audit §4.4 F2 |
| C12 | `phenoAI` Hermes imports (PhenoRegistry) | sharecli has no hermes workflows | **n/a** | n/a |
| C13 | `crane.yml` operator-decisions | (not in this SFD scope) | **Out of agent scope** | n/a |
| C14 | `cliff.toml` `commit.link` is non-standard | `git-cliff` would silently emit error | **Patch to standard var** | sharecli engineer; 30 min |
| C15 | `sharecli-dashboard` 12 widgets | `axe dashboard (WCAG 2.x Level A)` PASS | **Both correct** (UI is accessible; not the same as comprehensive) | CI |

## 15.3 Failure modes (where the system can fail in a way that surprises the operator)

| Mode | How it fails | How the system handles it | Operator action |
|---|---|---|---|
| Tray icon lib doesn't load | `sharecli-tray` exits with clear error | CLI keeps running | `sharecli status` for headless visibility |
| FUSE mount fails | `mount_smoke` exits with sysexits code | CLI keeps running | `modprobe fuse`; re-attempt |
| IPC socket in use | `sharecli-ipc` returns 76 (EX_PROTOCOL) | clear error | `rm /tmp/sharecli.sock` |
| Config file invalid | `sharecli config validate` returns 64 (EX_USAGE) | clear error, line number | edit `~/.config/sharecli/config.toml` |
| Resource cap exceeded | process is SIGSTOPped (not SIGKILLed) | pause, not kill | adjust cap; `sharecli resume` |
| Cross-process worktree conflict | `sharecli-mesh` returns "retry with backoff" | deterministic contract | retry |
| `is_agent` cannot determine parent process | (post-#929) falls back to env + exec-name heuristic | logs warning | check logs |
| Pooled execution unavailable | (post-#938) fail-closed with actionable error message | deterministic contract | follow error message |
| Ledger admission guard rejects | `sharecli-session` returns rejection reason | deterministic contract | follow reason |
| `cliff.toml` `commit.link` var | `git-cliff` emits an error or empty string | manual workaround: edit CHANGELOG.md | patch template |
| Cross-platform build (musl/freebsd/wasm) | upstream `tray-icon` limitation | build fails | build from source on target platform |
| 14 files > 1000 LoC | `cargo fmt` works; clippy passes | no failure | not blocking |
| Stale `Cargo.lock` | `cargo build` fails with version conflict | standard | `cargo update -p <crate>` |
| `unreachable!()` site triggers | (post-#937) clear "unhandled SessionCmd variant" message | the message is the documentation | add the missing variant |
| Tray-icon 0.25.0 backend fails | Linux build fails | (pre-#938) silent | PR #938 fixes |

## 15.4 Reconciliation pattern (when an ADR contradicts code)

The canonical pattern for reconciling a contradicting ADR is:

1. Identify the contradiction (this SFD's audit)
2. Run `cargo metadata` to confirm the code's behavior
3. Write a PR that updates the ADR to match the code (e.g., PR #935)
4. The PR is the single source of truth for the new ADR text
5. Merge; the ADR is now consistent

This pattern was applied for ADR-0007 (PR #935). It is the recommended pattern for any future ADR contradictions.

## 15.5 What "risk" means in this SFD

- **Risk** = a future event with a probability and a cost
- **Blocker** = a current event that prevents a gate from passing
- **Debt** = a current state that will cost more in the future if not addressed
- **Contradiction** = a current state where two sources disagree

These four categories are separate. A risk can become a blocker; a debt can become a contradiction. The 20 risks in §15.1 are risk; the 7 blockers in §13.2 are blockers; the 25+ debts in §07.1 are debts; the 15 contradictions in §15.2 are contradictions.

## 15.6 What we are NOT doing to reduce risk

- We are NOT adding a plugin system (PRD v1.0: no)
- We are NOT adding a mobile app (ADR-0001: no)
- We are NOT adding an MCP server (ADR-0004: no)
- We are NOT adding Harbor / SWE-bench (PRD: no)
- We are NOT silently re-scoping any excluded item (would be fraud on the mature contract)

These are explicit no-go decisions. They reduce risk *by refusing to expand scope*; not by adding new code.
