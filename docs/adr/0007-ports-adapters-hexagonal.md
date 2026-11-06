# ADR 0007 — Ports & Adapters (Hexagonal) Architecture

**Status:** Accepted
**Date:** 2026-08-28
**Deciders:** sharecli maintainers
**Supersedes:** —
**Related:** [lib-sprawl-plan.md](../ops/lib-sprawl-plan.md) (Phases 2–4 crate extraction)

---

## Context

The C00 L0 audit gap (score 2, effort M) calls for a formal ports/adapters ADR
to lock the hexagonal architecture boundaries between domain core and
infrastructure I/O. Without a written contract, crate extraction Phases 2–4
risk ad-hoc dependency edges that leak infrastructure concerns into the domain
core.

sharecli's architecture already follows an implicit hexagonal layout:

| Layer | Owner | I/O direction |
|-------|-------|---------------|
| **Domain core** | `sharecli-core` (Hypervisor, ThermalGate, CoalesceCache) | None — pure business rules |
| **Application services** | `sharecli-runtime` (ProcessPool, SharedRuntime, scheduler) | Inbound from CLI commands |
| **Driving adapters** | `sharecli` binary, `sharecli-tray-*`, `sharecli-thermal-tui` | CLI + desktop GUI |
| **Driven adapters** | `sharecli-fuse` (FUSE), `sharecli-fleet` (device registry), `sharecli-ipc` (CoalesceCache), `spawn-core-sys` (Zig hot core) | OS, filesystem, network |
| **Infrastructure** | `sharecli-mesh` (Maildir queue), `sharecli-session` (persistence) | Disk, network |

The `lib-sprawl-plan.md` proposes Target crate boundaries with explicit
dependency arrows. This ADR formalizes those as architectural invariants.

## Decision

### 1. Domain core is dependency-free

`sharecli-core` (Hypervisor, ThermalGate) must **never** depend on:
- Tokio async runtime
- Axum HTTP framework
- Filesystem or network I/O
- Any `sharecli-*` infrastructure crate

Domain types use plain Rust enums and structs. Async wrappers live in
application services.

### 2. Dependency arrows are one-directional

```text
Driving adapters (CLI, tray, TUI)
    → Application services (runtime, config, serve)
        → Domain core (sharecli-core)
        → Driven adapters (fuse, fleet, ipc, session, spawn-core-sys)
            → OS / hardware
```

**Rule:** No upward or lateral arrows. Driven adapters never import from
driving adapters. Domain core never imports from any adapter.

### 3. Port traits for driven adapters

Each driven adapter exposes a trait (port) that the application service
depends on, not the concrete implementation:

```rust
// In sharecli-core or a shared types crate
pub trait ThermalSensor {
    fn read_pressure(&self) -> Result<u8, SensorError>;
}

pub trait ProcessSpawner {
    fn spawn(&self, spec: &ProcessSpec) -> Result<ManagedProcess, SpawnError>;
}
```

Adapters implement these traits. Runtime wires concrete adapters at startup.

### 4. Crate boundary enforcement

- `cargo-deny` bans cross-boundary dependency edges (Tier C → Tier A/B)
- CI `cargo-machete` catches unused transitive deps across crate boundaries
- Each crate's `Cargo.toml` documents its layer in a header comment

### 5. Error isolation

Each adapter owns its error type. Application services map adapter errors
to domain-level error variants. The `ErrorEnvelope` (serve HTTP) maps
domain errors to JSON at the boundary, not inside handlers.

## Consequences

- Adapters can be swapped (e.g., FUSE → native intercept) without changing
  domain core or application services.
- Unit testing domain core requires no async runtime or I/O mocks.
- New adapters (e.g., a different process spawner) only implement a trait
  and register at startup.
- Phases 2–4 of `lib-sprawl-plan.md` can be executed against these rules
  with CI enforcement.

## Reconciliation (2026-10-17)

The 2026-10-17 deep-state audit (see
`audit-sessions/2026-10-17-deep-state-audit/MASTER-STATE-AUDIT.md`) found
that the current implementation diverges from the strict dependency-free
domain-core rule in §1 above:

- `crates/sharecli-core/Cargo.toml:9-13` declares runtime dependencies on
  `sharecli-ipc`, `sharecli-fleet`, `sharecli-fuse`, and `tokio = "1"`.
- The same `Cargo.toml` table appears in the comprehensive audit
  scorecard as row L2 (Layered Crate Topology) and is rated 3/3
  ("excellent") with the prose summary "Core -> IPC -> Fleet -> Fuse
  layering is consistent".

The scorecard's positive evaluation and ADR §1's prohibition directly
contradict each other. There are two honest resolutions; this section
adopts **(b) and documents the current state as a deliberately relaxed
variant** pending a future commit to (a).

### (a) Migrate `sharecli-core` to be dependency-free [rejected, multi-day]

The original §1 invariant. Requires:

- Extracting the IPC bridge into a `sharecli-ports` crate (or port
  traits in `sharecli-core` with a stub `sharecli-ipc` adapter).
- Moving the Fleet registry calls behind a port trait.
- Removing `tokio` from `sharecli-core` (move all async wrappers into
  the `sharecli-runtime` application service).
- Reworking the FUSE trigger path that today flows through
  `sharecli-core` (likely needs a callback or message-bus port).

This is a multi-day refactor that the audit flagged as out of scope for
the 2026-10-17 forward path. It is recorded as a follow-up in
`docs/FOLLOWUPS-2026-10-17.md` (TBD).

### (b) Revise ADR §1 to match the current state [adopted for now]

Replace the strict dependency-free invariant with a deliberately
relaxed "core-with-allowed-facade-deps" pattern, and update §2's
dependency-arrow diagram accordingly. The current `sharecli-core`
Cargo.toml becomes the canonical reference, not a violation.

**Revised §1 (effective 2026-10-17):**

> `sharecli-core` may depend on a closed set of in-tree facade crates
> (`sharecli-ipc`, `sharecli-fleet`, `sharecli-fuse`) and on `tokio`
> for the async runtime. `sharecli-core` must not depend on driving
> adapters (CLI, tray, TUI), on `axum` or other HTTP frameworks, or on
> third-party I/O libraries. New facade dependencies require an ADR
> amendment.

**Revised §2 diagram (effective 2026-10-17):**

```text
Driving adapters (CLI, tray, TUI)
    → Application services (runtime, config, serve)
        → Domain core (sharecli-core)
            → In-tree facades (sharecli-ipc, sharecli-fleet, sharecli-fuse)
            → Tokio async runtime
        → Driven adapters (sharecli-session, sharecli-mesh, spawn-core-sys)
            → OS / hardware
```

The horizontal `Domain core -> In-tree facades` arrow is the deliberate
relaxation. It is the smallest delta that lets the audit's scorecard
rating and the source-of-truth `Cargo.toml` agree with the ADR.

### Audit cross-references

- The original contradiction is logged in
  `audit-sessions/2026-10-17-deep-state-audit/subagent-reports/08-cross-cutting.md`
  under "ADR-0007 vs scorecard".
- The scorecard file
  `audit-sessions/2026-10-17-deep-state-audit/COMPREHENSIVE_AUDIT_SCORECARD.md`
  row L2 should be re-read as a positive evaluation of the relaxed
  pattern, not a false confirmation of the strict rule.

### Date and ownership

- **Reconciliation date:** 2026-10-17
- **Adopted variant:** (b) above
- **Owner:** sharecli maintainers
- **Follow-up:** TBD — see (a) above if/when the strict variant becomes
  feasible (e.g., after v1.0 ships and a port-trait refactor can land
  without breaking downstream forks).

## References

- lib-sprawl-plan.md: [`../ops/lib-sprawl-plan.md`](../ops/lib-sprawl-plan.md)
- Error envelope: [`../ops/error-envelope.md`](../ops/error-envelope.md)
- Concurrency docs: [`../ops/concurrency.md`](../ops/concurrency.md)
- ADR 0006 (Feb harness lineage): [`0006-feb-harness-recovery-lineage.md`](0006-feb-harness-recovery-lineage.md)
