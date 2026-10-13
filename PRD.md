# PRD — sharecli

## Overview

`sharecli` is an OS/kernel-adjacent **agent runtime / hypervisor** for
multi-agent hosts. It **detects** heterogeneous coding agents by process-tree
and pattern scan (it does **not** wrap vendor agent binaries as the primary
path), **watches** CPU / MEM / Net / FD and IO-relevant activity, and under
contention **coalesces / debounces / queues** redundant tool work
(Lock-Wait-Cache) while an optional **FUSE** layer intercepts IO and a
**thermal** governor gates speculative work. An **agent mesh / shared
substrate** (membership, coordination, fleet registry) sits above that;
declarative supervise / serve / tray is operator UX.

Core crates: `sharecli-core` (Hypervisor, detect, proc_scan), `sharecli-fuse`
(InterceptFs), `sharecli-ipc` (CoalesceCache), `sharecli-fleet`
(ThermalGovernor, FleetRegistry).

**Origin thesis** (authoritative lineage, not reconstruct-from-crates alone):

- Process-tree-aware command proxy with coalesce / debounce / queue strategies
  (human callers bypass); Tier-3 FUSE for shared-read / write-serialize
- Agent-mesh WBS: IPC primitives + process discovery & registry
- Hypervisor evolution: pattern detect + `/proc` walk; no per-bin wrap as the
  real product path; dual-git audit deferred out of product core

**Out of product scope:** Harbor / SWE-bench / Terminal-Bench agent-eval
harnesses. Suite-facing soft eval lives in `phenotype-tooling/crates/benchora`
(`harbor-soft/`). Harbor fork/env lives in `KooshaPari/portage-temp`.

## Goals

- Detect agents via proc scan + pattern match (no vendor-bin wrap)
- Watch CPU, memory, network, FDs, and IO/syscall-relevant activity
- Speculatively coalesce concurrent agent work; debounce / queue mutating paths
- Optional FUSE attach for meter / forward / shared-read coalesce hooks
- Coordinate agents through mesh / substrate; thermal gate under pressure
- Expose operator supervise / serve / tray surfaces for lifecycle and observability

## Epics

### E1 — Agent detection & mesh

- E1.1 Proc-scan discovery of known agent patterns (`detect`, `proc_scan`)
- E1.2 Mesh membership / substrate coordination across detected agents
- E1.3 No wrapping of Claude / other vendor agent binaries

### E2 — Watch & thermal gate

- E2.1 CPU / MEM / Net / FD observation for managed and detected agents
- E2.2 Thermal / contention gate (Green / Yellow / Red → Allow / Warn / Refuse)
- E2.3 Operator status / health surfaces (CLI + HTTP)

### E3 — Coalesce, debounce, queue & FUSE

- E3.1 Lock-wait / read coalesce (`sharecli-ipc` CoalesceCache)
- E3.2 Debounce window / queue for mutating or `nocache` args (documented path)
- E3.3 Optional FUSE cwd intercept (`InterceptFs`) for shared build-cache reads
- E3.4 Speculative merge of redundant concurrent checkout / IO work

### E4 — Operator supervise surface (supporting)

- E4.1 Declarative TOML lifecycle (start / stop / restart)
- E4.2 Config hot-reload, health checks, metrics, JWT serve AuthN
- E4.3 Tray / dashboard / plugins

## Acceptance Criteria

- Agents appear in discovery without wrapping their binaries
- Resource watch reports CPU / MEM / Net / FD (and IO path where FUSE enabled)
- Under multi-agent contention, coalesce paths reduce redundant IO / work
- FUSE attach point exists on Linux / macOS; coalesce hooks are the extension path
- Mesh / substrate state is observable (registry subject / status / serve)
- Thermal gate refuses speculative work under Red / Refuse
- Harbor soft CI is not required in-repo for product A+

## FR map (runtime thesis)

| FR | Title |
|----|-------|
| FR-006 | Agent Detection (proc scan, no bin wrap) |
| FR-007 | Resource & Syscall-Relevant Watch |
| FR-008 | Speculative Coalesce / Debounce / Queue |
| FR-009 | FUSE IO Intercept |
| FR-010 | Agent Mesh / Shared Substrate |
| FR-011 | Thermal Contention Gate |
| FR-012 | Serve HTTP Federated AuthN (operator surface) |

FR-001..FR-005 remain the supervise-surface acceptance spine.

---

## Proposed E5 — Actor-neutral global nested transaction participation (2026-10-12)

**Human intent:** [verbatim source](docs/intent/2026-10-12-global-nested-ledger.md). This is an extension of the existing OS-adjacent optimizer, not a new product or a claim of deployed capability.

When local human and software-agent work is split, coalesced, cancelled or retried, the system must preserve originating commitments in **one logical global nested transaction tree** with separate consumer subscriptions and cross-branch shared resources. Actor is common to Human and SoftwareAgent, while permissions are independently delegated and enforced. Standalone ShareCLI remains useful; PhenoFabric joins distributed placement and epochs via an explicit authority adoption procedure.

**User-facing outcomes:** no duplicate charges for one shared physical execution; cancelling one subscriber does not cancel another; a child actor never gains a duplicated parent budget; an unknown remote effect cannot silently run twice; recorded history distinguishes planned, attempted, observed and accepted.

**Scope and acceptance:** proposed FR-013, [architecture ADR 0009](docs/adr/0009-global-nested-ledger-local-admission.md), [verification](docs/verification/global-nested-ledger.md). The present code comprises pure Rust guard predicates with tests; integration into process dispatch, distributed authority, and native effect fencing remains pending.
