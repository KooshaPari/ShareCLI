# 02 — Intent, mature contract, CVP and MVP

## 2.1 Identity and mature finish line

**sharecli** is a single-binary Rust **agent runtime / hypervisor** for multi-agent hosts. It detects coding agents via process-tree and pattern scan (it does not wrap vendor binaries as the primary path), watches CPU / MEM / Net / FD / IO activity, and under contention coalesces, debounces, and queues redundant tool work (Lock-Wait-Cache) while an optional FUSE layer intercepts IO and a thermal governor gates speculative work. An agent mesh / shared substrate (membership, coordination, fleet registry) sits above; declarative `supervise` / `serve` / `tray` is operator UX.

**Beneficiary:** the multi-agent host operator — a single human or a small team running 1-50 heterogeneous coding agents (claude, codex, thegent, ghostty shells, custom research tools) on one workstation.

**Durable outcome:** the operator can install sharecli once, supervise every coding agent on the host, observe aggregate resource pressure, and let sharecli coalesce redundant agent work without per-tool integration.

## 2.2 Recovered intent (chronological archaeology)

| Date | Source | Stated intent |
|---|---|---|
| 2026-04 (pre-history) | `thegent-sharecli` fork-line (now defunct) | "agent CLI for shared orchestration"; absorbed into `thegent` later |
| 2026-06-30 | `PRD.md` (current canonical) | "OS/kernel-adjacent agent runtime / hypervisor for multi-agent hosts" |
| 2026-08-31 | `CHANGELOG.md:27` (now corrected) | `1.0.0` (release tag) |
| 2026-09-01 | `docs/changes` | 1.0.0 release notes |
| 2026-09-12 | `VERSION:1` (now corrected) | `0.8.0` |
| 2026-09-16 | ADR-0007 (ports-adapters-hexagonal) | "sharecli-core must NEVER depend on sharecli-* infra" |
| 2026-09-20 | `COMPREHENSIVE_AUDIT_SCORECARD.md` | "sharecli-core depends on sharecli-ipc, sharecli-fleet, sharecli-fuse, scored 3/3" (contradicts ADR-0007) |
| 2026-10-12 | `docs/intent/2026-10-12-global-nested-ledger.md` | ledger admission contract |
| 2026-10-16 | `audit-sessions/2026-10-17-deep-state-audit` | 8-lane evidence dump |
| 2026-10-17 | `PRD.md` (re-read) | "no Harbor / SWE-bench / Terminal-Bench in product core" |

**Unresolved contradiction (operator decision required):** ADR-0007 forbids `sharecli-core → sharecli-ipc/fleet/fuse`; the scorecard celebrates it. The code follows the scorecard. **Recommended:** scorecard wins, ADR-0007 §1 is updated to match (PR #935 already drafted this).

## 2.3 Goal tree (mature contract)

| Goal ID | Accepted outcome | Source / authority | Mature acceptance | Current evidence | CVP / MVP projection |
|---|---|---|---|---|---|
| **G1** Detect agents via proc scan + pattern match (no vendor wrap) | `sharecli-core::detect` and `proc_scan` modules return a stable `AgentId` for known agent patterns from `/proc`-equivalent walk | `PRD.md` §Epics E1.1, E1.3 | FR-001 AC; surviving test `c01_climb2_core.rs`; agent registry has 8+ patterns | `E-detect: PASS` in CI on main, PASS on `chore/cvp-1h-queue-2026-10-17` | **IN CVP** |
| **G2** Watch CPU/MEM/Net/FD/IO across managed and detected agents | `sharecli-fleet::resource_watch` exposes per-agent stream; `proc_scan` rolls up | `PRD.md` §E2.1 | FR-002 AC | `E-resource_watch: PASS` CI on main | **IN CVP** |
| **G3** Speculatively coalesce concurrent agent work; debounce/queue mutating paths | `sharecli-ipc::CoalesceCache` + `SpeculativeCoalescer` (FR-008) | `PRD.md` §E3 | FR-008 AC (SpeculativeCoalescer) | CoalesceCache: PASS main; SpeculativeCoalescer: PASS off-main (PR #927) | **IN CVP** (after #927 merges) |
| **G4** Optional FUSE attach for meter/forward/shared-read | `sharecli-fuse::InterceptFs`; `mount_smoke` binary | `PRD.md` §E4 | FR-003 AC; FUSE mount smoke test | `E-fuse-mount: PASS` CI on main | **IN CVP** |
| **G5** Coordinate agents through mesh/substrate; thermal gate under pressure | `sharecli-fleet::ThermalGovernor` + `sharecli-mesh::{membership, worktree_pool}` | `PRD.md` §E5 | FR-004 AC | `E-thermal-gate: PASS` CI on main | **IN CVP** |
| **G6** Operator supervise/serve/tray surfaces for lifecycle and observability | `sharecli-tray-{linux,windows}` + `serve` subcommand + `supervise` subcommand | `PRD.md` §E6 | FR-005..FR-007 AC | tray: implemented; libappindicator backend; windows: ksni backend | **IN CVP** (after #938 lands) |
| **G7** FFI C-ABI for embedders (third-party hosts) | `sharecli-ffi::lib.rs` 6 `pub extern "C" fn` | `docs/adr/0009-airlock-convergence-contract-capture.md` | FR-010 AC (planned) | `E-ffi-impl: PASS` CI on main; docs: PASS on `chore/cvp-1h-queue-2026-10-17` | **DEFERRED from CVP** (FFI is a sub-product, FR-010 not in CVP-1.0) |
| **G8** Atomic txn admission across the global nested ledger | `crates/sharecli-session/src/ledger.rs` | `docs/intent/2026-10-12-global-nested-ledger.md`, ADR-0009 | FR-011 AC | `E-ledger: PASS` main (PR #892 merged); `E-admission-guard: PASS` | **IN CVP** (already accepted) |
| **G9** Configurable runtime caps with validation | `src/config.rs` + `cb37decc` | ADR-0007 §2 | FR-012 AC | `E-runtime-caps: PASS` CI on main | **IN CVP** |
| **G10** Plugin/extensibility surface | (not in PRD v1.0) | — | — | absent | **OUT OF SCOPE** for v1.0 |
| **G11** Mobile companion app | (not in PRD v1.0) | ADR-0001 | not in scope | absent | **EXCLUDED** by ADR-0001 |
| **G12** MCP server mode | (not in PRD v1.0) | ADR-0004 | not in scope | absent | **EXCLUDED** by ADR-0004 |
| **G13** Harbor / SWE-bench / Terminal-Bench integration | (deferred) | `PRD.md` §Out of product scope | not in scope | absent; lives in `phenotype-tooling/crates/benchora` | **EXCLUDED** by `PRD.md` §Out of product scope |

## 2.4 CVP (Closest Viable Product) projection

**CVP-1.0** = G1 ∪ G2 ∪ G3 ∪ G4 ∪ G5 ∪ G6 ∪ G8 ∪ G9 = 8 of 8 CVP goals; engineering leaves = 19/20 = 95% (audit baseline 18/20 + is_agent + tray-icon fix). **CVP journey: 1 host, 1 human, install sharecli, supervise 3+ heterogeneous agents, observe thermal gate under pressure, no agent ever silently consumes more than its declared cap.** Verified end-to-end on `chore/cvp-1h-queue-2026-10-17` and through CI on main, modulo the tray-icon fix.

## 2.5 MVP projection

**MVP-1.0** = CVP + G7 (FFI) + real-user validation = 9 goals. Real-user validation requires a recorded `docs/eval/<date>-<human>-<journey>/` session entry — **no such entry exists today**. The MVP gate cannot be claimed PASS until at least one such entry exists, the entry is dated, and the human confirms the journey succeeded.

## 2.6 Authoritative exclusions (preserved)

| Excluded | Authority | Reason |
|---|---|---|
| Mobile app | ADR-0001 | not product |
| MCP server | ADR-0004 | not product |
| Harbor / SWE-bench integration | `PRD.md` | out of product scope |
| Agent-eval harness | ADR-0005 | eval surface is separate (benchora) |
| Per-vendor agent wrapping | `PRD.md` §Origin thesis | detection not wrapping |
| File-size decomposition of all 14 over-1000L files | `BOUNDARY.md` | accepted as "post-MVP" with explicit rationale |
| `monitoring.rs` and `pyroscope_stub.rs` extraction | audit §3 Theme B | honest stubs with module-level docs |
| `crates/spawn-core` Zig build wiring | audit §4.4 F2 | on disk but not in workspace |
| `crates/gateway-tools` workspace membership | PR #923 in flight | integration pending |

**Do not silently re-add any of these as scope.** Doing so would be a fraud on the mature contract.
