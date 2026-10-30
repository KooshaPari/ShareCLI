---
title: sharecli session state machine
topic: sharecli-session SessionState transitions
diagram_type: stateDiagram-v2
source: crates/sharecli-session/src/{lib.rs,ledger.rs,recovery.rs}
audience: contributors working on recovery / surface observation / session persistence
---

# sharecli session state machine

The `sharecli-session` crate models one durable agent-session record as a
four-state `SessionState` enum. This diagram captures the transitions the
crate actually performs; it is the source of truth for "what can change
about a session's state and when" — `recovery_plan`, `recovery_executor`,
`ledger::recover_stale`, and the `SessionService` use it as the filter and
the write target.

The four states are exactly:

```text
pub enum SessionState { Pending, Active, Exited, Unknown }
```

(`crates/sharecli-session/src/lib.rs:200`).

Transitions observed in the source:

- `AgentSession::new` / `with_recipe` initialise **Active** (`lib.rs:263`,
  `lib.rs:307`). The harness wrapper is expected to surface-map a real
  terminal before writing the ledger, but the in-memory default is Active.
- `RecoveryExecutor::execute → launch` (`recovery.rs:82`) only proceeds
  when state ∈ {`Active`, `Exited`, `Pending`}. A session in `Unknown`
  is reported as `UnsupportedSurface` and never re-launched — this is the
  reason `Unknown` is terminal-with-respect-to-recovery.
- `ledger::recover_stale` (`ledger.rs:102`) walks every persisted
  `AgentSession` and sets `state = SessionState::Unknown` for any record
  older than `DEFAULT_RECOVERY_MAX_AGE_SECONDS` without a live surface
  observation. This is the only write to `Unknown` in the crate.

```mermaid
stateDiagram-v2
    direction LR

    [*] --> Active : AgentSession::new / with_recipe\n(lib.rs:263, 307)
    [*] --> Pending : SidecarStateProvider append\n(state.rs SidecarRecord)

    Pending --> Active : SurfaceObservationScanner\ncorroborates launch-time mapping
    Pending --> Unknown : ledger::recover_stale\n(max_age exceeded)

    Active --> Exited : SurfaceObservationScanner\nmarks terminal / process gone
    Active --> Unknown : ledger::recover_stale\n(no live surface, max_age exceeded)

    Exited --> Active : SessionService::resume\nre-uses argv + cwd recipe
    Exited --> Unknown : ledger::recover_stale\n(no surface ping within max_age)

    Unknown --> [*] : SessionService::prune\nor operator `session forget`
    Unknown --> Pending : SidecarStateProvider re-records\nthe same surface id

    note right of Active
        Default state for any new AgentSession.
        Eligible for RecoveryExecutor::launch
        (recovery.rs:82).
    end note

    note right of Unknown
        RecoveryExecutor refuses to launch
        an Unknown session — only Active,
        Exited, Pending are eligible. The
        ledger writes this state once when
        max_age is exceeded; the only
        transitions out are prune or a new
        SidecarRecord append.
    end note
```

## How to read this

- **Active** is the steady state — the surface is observable and the
  harness wrapper holds a live PID. Almost all reads treat Active as the
  "this is real, go look" signal.
- **Pending** is the launch-time evidence window: a wrapper has
  written a `SidecarRecord` but the surface scanner has not yet
  corroborated it. Once it does, the session moves to Active.
- **Exited** is the recovery-friendly terminal state — the surface is
  gone but the recipe (argv + cwd) is still useful, so a fresh
  `RecoveryExecutor::execute` can re-spawn the harness from the recipe.
- **Unknown** is the "don't trust this" sink. `recovery_plan` filters it
  out and the only legitimate way out is a fresh SidecarRecord append
  (which moves it back to Pending) or an explicit operator prune.

## Where this shows up in code

| File | Lines | What it does |
|------|-------|--------------|
| `crates/sharecli-session/src/lib.rs` | 200 | `enum SessionState { Pending, Active, Exited, Unknown }` |
| `crates/sharecli-session/src/lib.rs` | 263, 307 | `Active` is the default for `new` / `with_recipe` |
| `crates/sharecli-session/src/state.rs` | 12-… | `SidecarRecord` appends that create Pending entries |
| `crates/sharecli-session/src/ledger.rs` | 102 | The single write to `Unknown` (`recover_stale`) |
| `crates/sharecli-session/src/recovery.rs` | 82 | `launch` filter — refuses `Unknown` |
| `crates/sharecli-session/src/discovery.rs` | – | `SurfaceObservationScanner` drives Pending → Active and Active → Exited |

## Cross-references

- [`01-system-overview.md`](01-system-overview.md) — where the
  `sharecli-session` crate sits in the 16-crate topology.
- `docs/adr/0009-global-nested-ledger-local-admission.md` — why this
  crate owns its own ledger rather than sharing one with
  `sharecli-fleet`.
- `FUNCTIONAL_REQUIREMENTS.md` — FR-009 pins the durable-session
  behaviour this state machine backs.