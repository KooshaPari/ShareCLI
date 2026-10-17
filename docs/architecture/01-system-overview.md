# 01 — System overview (crate topology)

This diagram shows the runtime crate topology of sharecli. The
goal is to answer "where does this functionality live?" in one
glance.

```mermaid
graph TD
  classDef bin fill:#1e293b,stroke:#94a3b8,color:#f1f5f9
  classDef core fill:#0c4a6e,stroke:#7dd3fc,color:#e0f2fe
  classDef adapter fill:#14532d,stroke:#86efac,color:#dcfce7
  classDef ui fill:#581c87,stroke:#d8b4fe,color:#f3e8ff
  classDef vendor fill:#7c2d12,stroke:#fdba74,color:#ffedd5

  CLI[sharecli<br/>CLI binary<br/>src/main.rs]:::bin
  SYNC[sharecli-sync]:::ui
  TRAY_L[sharecli-tray-linux]:::ui
  TRAY_W[sharecli-tray-windows]:::ui
  THERM[sharecli-thermal-tui]:::ui
  FSR[fuse-smoke-runner]:::ui
  SC[sharecli-core]:::core
  HN[harness-native]:::core
  SFFI[sharecli-ffi]:::adapter
  SF[sharecli-fleet]:::adapter
  SIPC[sharecli-ipc]:::adapter
  SESS[sharecli-session]:::adapter
  SFUSE[sharecli-fuse]:::adapter
  SMESH[sharecli-mesh]:::adapter
  SCS[spawn-core-sys<br/>Zig/POSIX fork+waitpid]:::vendor

  CLI --> SC
  CLI --> SF
  CLI --> SIPC
  CLI --> SESS
  CLI --> THERM
  CLI --> SFUSE
  CLI --> SFFI
  CLI --> SCS

  SC --> SIPC
  SC --> SF
  SC --> SFUSE
  SIPC --> SF
  SIPC --> SESS
  HN --> SC
  HN --> SIPC
  THERM --> SF
  THERM --> SFUSE
  THERM --> SMESH
  FSR --> SFUSE
  SYNC --> SC

  class SCS vendor
```

## Reading the diagram

- **Blue (core)**: `sharecli-core` is the central state machine,
  resource manager, and process-pool. The CLI binary depends on
  it for every state-mutating operation. `harness-native` is
  the per-OS harness detection lib and is a thin wrapper around
  `sharecli-core`.
- **Green (adapter)**: adapters wrap I/O and external systems.
  - `sharecli-ffi` — C-ABI surface for the sharecli-fleet
    aggregator and third-party languages.
  - `sharecli-fleet` — multi-device fleet registry, thermal
    aggregation, and the contract-capture surface.
  - `sharecli-ipc` — NDJSON-RPC over Unix socket / Windows
    named pipe, plus the zmx session transport.
  - `sharecli-session` — durable terminal layout snapshots and
    session ledger (FR-005).
  - `sharecli-fuse` — FUSE mount + CoW + write-provenance xattr
    (FR-009).
  - `sharecli-mesh` — gossip / shared substrate protocol
    (FR-010).
- **Purple (ui)**: presentation-layer crates. None of them are
  required by the CLI binary; they import it instead.
  - `sharecli-sync`, `sharecli-tray-linux`, `sharecli-tray-windows`
    — desktop tray icons and the sync surface.
  - `sharecli-thermal-tui` — terminal UI for the thermal gate
    (FR-011).
  - `fuse-smoke-runner` — minimal test harness for the FUSE
    layer.
- **Orange (vendor)**: `spawn-core-sys` wraps a small Zig
  library that does POSIX `fork`+`waitpid` on Unix and a
  Rust stub on Windows. The CLI binary depends on it directly
  because `sharecli-core` is meant to be testable on Windows
  without a real spawn primitive.

## Why this shape

`sharecli-core` is the only crate that depends on more than
two other sharecli-* crates. It is the **port layer** in
ports-and-adapters terms. The adapter crates are independent
of `sharecli-core` and depend only on each other when there is
a hard technical reason (e.g. `sharecli-ipc` depends on
`sharecli-fleet` because the IPC transport needs the fleet
registry to resolve a peer by name).

The UI crates are leaves: they pull in core+adapters but no
adapter pulls them in. This keeps the CLI binary's compile
graph shallow on the inbound side (it only depends on the
ones it needs at runtime) and the adapter crates free of
UI-framework dependencies.

See [docs/adr/0007-ports-adapters-hexagonal.md](../../adr/0007-ports-adapters-hexagonal.md)
for the rationale behind this split.
