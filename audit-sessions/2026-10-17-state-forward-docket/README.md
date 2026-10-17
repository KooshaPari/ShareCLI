# State & Forward Docket — sharecli (snapshot 2026-10-17)

> **SUBJECT EVIDENCE BOUND.** This docket is a snapshot of `KooshaPari/ShareCLI` taken at 2026-10-17 08:52 UTC (2026-10-17 01:52 Pacific). All claims are bound to a pinned source, a worktree, an audit, or an explicit UNKNOWN. It is not a product acceptance. The PhenoRegistry index pointer is at `audits/sharecli/2026-10-17-sfd-001/`. The sharecli repo copy lives at `audit-sessions/2026-10-17-state-forward-docket/`.

| Field | Value |
|---|---|
| Subject | `sharecli` (v0.8.0, working toward v1.0) |
| Repository | `KooshaPari/ShareCLI` |
| Clone path | `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli` |
| Branch under audit | `chore/cvp-1h-queue-2026-10-17` @ `5f2425ca` |
| Origin main | `ec7bcbd9` (2026-10-17 00:10 UTC, #918 squash-merged) |
| Working-tree state | dirty: `Cargo.lock`, `crates/sharecli-tray-windows/Cargo.toml` (tray-icon fix) |
| Audit baseline | `134888ede` (2026-10-16 21:53 PDT — 8-lane deep audit commit) |
| Audit artifacts | `audit-sessions/2026-10-17-deep-state-audit/{MASTER-STATE-AUDIT.md, subagent-reports/, deliverables/}` |
| SFD commit (this docket) | `5f2425ca0d2d14a40e0f7c96029fdc854bbc7792` |
| Observation window | 2026-10-17 01:08 UTC (audit commit) to 2026-10-17 08:52 UTC (this docket) |
| Timezone | Pacific (UTC-7) for human dates, UTC for machine timestamps |
| Beneficiary | Multi-agent host operator running heterogeneous coding agents (claude, codex, thegent, etc.) on macOS, Linux, or Windows |
| Stage | CVP-complete on declared scope; MVP validation pending first real use session |
| Confidence | bounded — engineering state high, real-user validation absent, multiple branches unmerged |

## Two-minute decision brief

**What this is:** sharecli is a single-binary Rust agent-runtime/hypervisor that detects, watches, coalesces, and orchestrates coding-agent processes on a single host. It does not wrap vendor agent binaries; it supervises them.

**Usable now:** the 7 base FRs (FR-001..FR-007) are implemented and pass CI; the 9 additional CVP leaves (FR-008, 010, 011, 012 plus FFI/test coverage) have working PRs (#919, #921, #922, #923, #924, #925, #927, #929) that are MERGEABLE but currently blocked behind 5 audit remediation PRs (#932, #933, #934, #935, #936) which themselves block on a pre-existing tray-icon 0.25.0 cargo-feature failure on main. The repo is in better shape than the 2026-09-20 scorecard suggested.

**Not usable now:** anything depending on `crates/gateway-tools` (not in workspace), `crates/spawn-core` (Zig build, not wired), `crates/sharecli-runtime/serve/config` (planned but not extracted), the documented `monitoring.rs` and `pyroscope_stub.rs` stub modules (honest, not wired), and the `crates/harness-native/src/is_agent.rs` detection (now implemented on the `chore/cvp-1h-queue-2026-10-17` branch, not yet on main). Real user validation absent.

**Change since the prior snapshot:** 9 commits have landed on `chore/cvp-1h-queue-2026-10-17` since the audit. The HIGH item from the audit (`is_agent.rs` stub) is now implemented and the implementation sits on a non-default branch. Five forward-path PRs (#932-#937) are MERGEABLE in branch sense; their `ci / test` checks fail because of a pre-existing tray-icon 0.25.0 backend-feature failure on main, not because of the PR contents. The fix is itself uncommitted on `fix/tray-icon-0.25.0-bump`.

**Next best action:** commit the tray-icon 0.25.0 fix (`features = ["libappindicator"]`), push, and watch `ci / test` re-run. After green, the merge train from PR #937 backwards through #932 is low-conflict. The HIGH is_agent remediation is already on `chore/cvp-1h-queue-2026-10-17` (PR #936) and goes in through that PR.

**Operator decision required:** (1) push `chore/cvp-1h-queue-2026-10-17` to origin now or fold is_agent into the next PR? (2) is the tray-icon fix to land as its own PR #938 or fold into #936? (3) ADR-0007 vs `COMPREHENSIVE_AUDIT_SCORECARD.md` contradiction — pick the scorecard as canonical (matches code) and update ADR-0007 §1 (Phase 3a already drafted as PR #935, MERGEABLE).

The full template set follows. Templates 00..18 each replace the placeholder line; UNKNOWNs are explicit.
