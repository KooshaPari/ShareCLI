# ADR-0007 — PURE vs INFRA classification for `sharecli-core` items

**Source**: `crates/sharecli-core/src/lib.rs` (HEAD `b4644757`, branch
`refactor/adr-0007-hybrid-clean-2026-10-17`).
**Purpose**: input for the `sharecli-core-pure` crate split (ADR-0007
amendment, see `docs/adr/0010-core-pure-hybrid.md`).

## Criteria

A `pub` item in `sharecli-core` is **PURE** if and only if its entire
type-level closure (fields, variants, trait bounds, function signatures)
involves only:
- `std::*` (core + alloc + collections + path + time + sync)
- `serde::{Serialize, Deserialize}` and `serde_json` (not used by `sharecli-core` itself)
- other PURE items within the same crate

A `pub` item is **INFRA** if its type closure transitively touches any of:

| Layer | Crate | Used by `sharecli-core`? |
|-------|-------|--------------------------|
| TIER-A→TIER-C | `sharecli-ipc` | YES (`pub use sharecli_ipc::*`) |
| TIER-A→TIER-C | `sharecli-fleet` | YES (`pub use sharecli_fleet::*`) |
| TIER-A→TIER-C | `sharecli-fuse` | YES (`Cargo.toml:11`) |
| TIER-A→TIER-C | `sharecli-sync` | NO |
| TIER-A→TIER-C | `sharecli-mesh` | NO |
| TIER-A→TIER-C | `sharecli-session` | NO |
| TIER-A→TIER-C | `sharecli-ffi` | NO |

`tokio::*`, `anyhow::*`, `tracing::*`, `tempfile::*` are utility/infra
runtime crates. They are NOT classified as infra-for-split purposes
(because the boundary is about "domain model" vs "concrete infrastructure
adapters"); however, any item whose type signature depends on them is
INFRA because the pure crate cannot pull them in without forcing the same
runtime constraint on embedders.

## Classification table

| Module / source | Item | Class | Reason |
|-----------------|------|-------|--------|
| `crate::detect::*` (re-exports) | `match_known_agent`, `KNOWN_AGENT_FAMILIES` | INFRA | re-exports from `sharecli_fleet::detect` |
| `crate::fuse_guard::*` | `fuse_session_id_for_command_key` | INFRA | depends on `sharecli_fuse` (FUSE IO-intercept) |
| `crate::hypervisor` | `Hypervisor` | INFRA | engine wires tokio + sharecli_ipc + sharecli_fuse + sharecli_fleet |
| `crate::proc_scan::*` | all agents (`DetectedAgent`, `HostProcSource`, …) | INFRA | re-exports from `sharecli_fleet::proc_scan` |
| `crate::sharecli_fleet` re-exports | `ResourceWatchSample`, `sample_*` | INFRA | direct cross-crate re-export |
| `crate::sharecli_ipc` re-exports | `command_key`, `CoalesceCache`, `CachedResult`, `SlotQueue`, … | INFRA | direct cross-crate re-export |
| `crate::speculation` | `SpeculationTracker`, `SpeculationCandidate`, constants | INFRA | `CommandKey`, `CachedResult`, `CoalesceCache` from `sharecli_ipc` |
| `crate::thermal_gate` | `ThermalDecision`, `ThermalGate`, `SystemThermalGate`, `AgentAwareThermalGate`, `FakeThermalGate`, `combine_thermal_agent_decision`, `THERMAL_MAX_RETRIES`, `THERMAL_RETRY_SLEEP` | INFRA | depends on `sharecli_fleet` / platform probes (tokio) |
| `crate::types` | `HypervisorConfig`, `SpawnRequest`, `SpawnOutcome` | INFRA | fields use `sharecli_ipc::CacheKeyMode`, `QueuePriority`, `sharecli_fleet::ResourceWatchSample`, `sharecli_fuse` calls |
| **`crate::transaction_contract`** | `Actor`, `ActorKind`, `Grant`, `validate_delegation`, `GuardError`, `ReservationPool`, `ProducerState`, `SharedProducer`, `EffectOutcome`, `can_retry_effect`, `ParentMembership` | **PURE** | uses only `std::collections::{BTreeMap, BTreeSet}` — no infra, no tokio |

## Result

**Total PURE items**: 11 unique items / 12 type-level items (1 enum + 1 fn in
`Actor/Grant/etc.`, plus the `validate_delegation` and `can_retry_effect`
free functions, plus `ReservationPool` / `SharedProducer` /
`ParentMembership` impl blocks).

**Total INFRA items**: ~70 (every re-export from `sharecli_ipc`,
`sharecli_fleet`, every `pub` in `fuse_guard`, `hypervisor`, `proc_scan`,
`speculation`, `thermal_gate`, `types`).

The single pure surface is `sharecli_core::transaction_contract`. That is
the SSOT foundation for the `sharecli-core-pure` crate re-exports.

## Followup

- Future churn: if `sharecli-core` adds a new pure item, add a re-export to
  `sharecli-core-pure` and bump the version. CI gate to enforce sync is
  recommended but out of scope for this amendment.
