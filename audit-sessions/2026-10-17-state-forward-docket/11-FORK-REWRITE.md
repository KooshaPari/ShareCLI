# 11 — Fork, rewrite and re-platform analysis

## 11.1 What is sharecli today (1-paragraph description)

A single-binary Rust **agent runtime / hypervisor** for multi-agent hosts. It detects coding agents via process-tree and pattern scan (not vendor-wrap), watches CPU / MEM / Net / FD / IO, coalesces redundant tool work via Lock-Wait-Cache, optionally attaches a FUSE layer for meter/forward/shared-read, coordinates agents through a mesh/substrate with a thermal governor, and exposes `supervise` / `serve` / `tray` operator surfaces plus a 6-`pub extern "C" fn` FFI C-ABI for embedders. Live in CVP-1.0, deferred to MVP-1.0 for FFI embedder use. 486 Rust files, 101,757 LoC, 19 critical-engineering leaves verified, 5 journeys verified, 1 critical engineering leaf pending (= tray-icon fix), 1 critical assurance leaf blocked (= `ci / test` on main), 1 MVP gate blocked (= no recorded beneficiary session).

## 11.2 Fork vs rewrite vs re-platform matrix

| Decision axis | Fork | Rewrite | Re-platform | Status |
|---|---|---|---|---|
| **Re-implement from scratch in a different language** | — | rewrite in Zig, Go, Nim? | — | **REJECTED**: every metric (LoC, perf, maturity, IPC UDS, FUSE) is better in Rust today; rewrite is at least 6-12 engineer-months and would re-build every weakness |
| **Re-platform to a different crate layout** | — | — | re-platform | **DEFERRED to v1.1**: extract `sharecli-runtime` and `sharecli-config` from the `src/` root into workspace crates; `lib-sprawl-plan.md` Phase 4; 2-3 hours; not for v1.0 |
| **Fork into a separate product** | fork | — | — | **REJECTED**: `thegent-sharecli` was this fork, now defunct; `BOUNDARY.md` records the absorption into `thegent` instead |
| **Fork the FFI C-ABI into a separate product** | fork | — | — | **CONSIDERED** but **DEFERRED**: FFI is a sub-product, FR-010 in v1.1; sharecli-ffi is a self-contained crate today |
| **Fork `crates/gateway-tools` into a separate product** | fork | — | — | **DEFERRED**: PR #923 ships it inside sharecli first; can be split post-MVP |
| **Fork `crates/spawn-core` (Zig) into a separate product** | fork | — | — | **REJECTED for v1.0**: ADR-0006 says `portage-temp`; on disk in sharecli but not in workspace; document and freeze |
| **Replace sharecli with `systemd --user` + cgroups** | — | — | re-platform | **REJECTED**: systemd does not detect agents, does not coalesce, does not FUSE-intercept, has no agent-mesh, no thermal governor, and is OS-service-management, not user-agent-supervision |
| **Replace sharecli with Kubernetes** | — | — | re-platform | **REJECTED**: K8s is multi-host, not single-host; sharecli is single-host per `PRD.md` |
| **Replace sharecli with `tmux` / `dtach` / `abduco`** | — | — | re-platform | **REJECTED**: text-only UX; sharecli supervises processes + mesh + thermal |
| **Replace sharecli with `HyperDX` / `Phoenix LiveView`** | — | — | re-platform | **REJECTED for the operator UI**: those are observability UIs; sharecli has its own dashboard; not a competitor |
| **Replace sharecli with Harbor / SWE-bench** | — | — | re-platform | **REJECTED**: those are eval harnesses; out of scope per `PRD.md` |
| **Replace sharecli with a per-vendor wrapper (claude-cli, codex-cli)** | — | — | re-platform | **REJECTED**: the entire sharecli thesis is **detection not wrapping**; wrapping is a per-tool integration nightmare |

## 11.3 What we have learned (cost-of-replacement estimate)

If we throw away sharecli and rebuild from scratch, the cost is approximately:

| Component | LoC today | Engineer-months to re-build | Reuse path |
|---|---|---|---|
| CLI (clap derive) | ~1,808 | 0.5 | reuse clap |
| Configuration | ~1,166 | 0.5 | reuse serde + toml |
| Process pool + hypervisor | ~1,358 | 1.0 | partial reuse of `nix`/`tokio` |
| IPC UDS + peer creds | ~1,200 | 1.0 | reuse `tokio`/`nix` |
| FUSE + WinFSP | ~1,709 | 1.5 | reuse `fuser` |
| Fleet registry + resource watch + thermal | ~1,100 | 1.0 | reuse `sysinfo` |
| Mesh + worktree pool | ~900 | 1.0 | reuse `git2` |
| Harness native + is_agent | ~500 | 0.5 | rewrite in any language |
| Tray (libappindicator + ksni) | ~400 | 0.5 | reuse `tray-icon` |
| FFI C-ABI | ~300 | 0.25 | reuse `cbindgen` |
| Session ledger | ~600 | 0.5 | reuse `redb` |
| Teamcomm CLI | ~600 | 0.5 | reuse `clap` |
| 88-pillar scorecard | n/a | n/a | sharecli is unique |
| SpeculativeCoalescer | ~500 | 0.5 | sharecli is unique |
| Tests (CI 64% → 80%) | n/a | 2.0 | re-write |
| CI workflows (50+ workflows) | n/a | 1.0 | re-write |
| Documentation (PRD, SPEC, FR, BOUNDARY, ADRs) | n/a | 1.0 | carry over |
| **Total rebuild estimate** | **~12,000 LoC** | **~12 engineer-months** | n/a |

**Recommendation: do not rewrite.** Spend the 12 engineer-months on hardening, MVP-1.1, and the deferred items in §07.1.

## 11.4 What the audit recommended

| Recommendation | Source | Status |
|---|---|---|
| Accept `monitoring.rs` and `pyroscope_stub.rs` as honest stubs | audit §3 Theme B | accepted (v1.0) |
| Defer `sharecli-runtime` extraction to v1.1 | audit §4.8 MEDIUM | accepted (v1.1) |
| Accept 14 files > 1000 LoC with `BOUNDARY.md` rationale | audit §4.4 F1 | accepted (v1.0) |
| Document `crates/spawn-core` placement in ADR-0006 | audit §4.4 F2 | not done; recommend as v1.1 ADR |
| Add `crates/gateway-tools` to workspace | audit §4.4 F3 | PR #923 MERGEABLE |
| Keep 8 `unreachable!()` sites as defensive | audit §4.1 | accepted; PR #937 reduces 5 to 1 |
| Do not add new ADRs until contradictions resolved | audit §6 | accepted |
| Reconcile ADR-0007 §1 with current code | audit §6 | PR #935 DRAFTED |
| Replace `process::exit(1)` with detection in is_agent | audit §4.2 | done in `878212aa` on `chore/cvp-1h-queue-2026-10-17` |
| Lift `sharecli-fleet` 110 pub items via facade reorg | audit §4.8 MEDIUM | v1.1 |
| Add `cargo-deny` allowlist for tray-icon advisory | audit §6.4 | PRs #896, #909 in flight |

## 11.5 What the audit did NOT recommend (and we should not silently add)

| Item | Why it was not in audit scope | Operator stance |
|---|---|---|
| File-size decomposition of all 14 over-1000L files | `BOUNDARY.md` rationale | accept as-is |
| Mobile app | ADR-0001 | out of scope |
| MCP server | ADR-0004 | out of scope |
| Harbor / SWE-bench | `PRD.md` | out of scope |
| Per-vendor agent wrapping | `PRD.md` | thesis; do not add |
| Plugin system | not in PRD v1.0 | out of scope |
| `crates/spawn-core` migration to portage-temp | ADR-0006 says; no `move` PR | v1.1 |
| `monitoring.rs` and `pyroscope_stub.rs` deletion | honest stubs are accepted | accept as-is |
| Prebuilt binary | operator-side | not for v1.0 |
| Homebrew tap | operator-side | not for v1.0 |
| Operator manual | v1.1 | not for v1.0 |

## 11.6 Decision: do not fork, rewrite, or re-platform

**Recommendation: stay on Rust; stay on the current architecture; freeze the layout; v1.0 ships when the 5 critical engineering leaves (including the tray-icon fix) and the 4 critical assurance leaves go green; v1.1 then does the deferred extractions.** This is the lowest-cost path with the highest fidelity to operator experience. The audit recommends exactly this; the SFD agrees.
