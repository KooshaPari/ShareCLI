# sharecli-fleet

> Fleet registry and thermal governor for sharecli's multi-host runtime.

`sharecli-fleet` provides the device registry (NATS-backed) and
thermal-aware scheduling primitives that decide when it is safe to
spawn on a given host. It is the layer above `sharecli-core` that
turns per-process spawning into fleet-wide resource accounting.

## Modules

- `registry` — NATS-backed device registry: host enroll, heartbeat,
  capability advertisement, drain.
- `thermal` — thermal gate and tier classification (Green / Yellow /
  Red), polled by `sharecli-core::ThermalGate`.
- `resource_watch` — per-process RSS, FD, and net watch samplers
  used by the contention tier.
- `agent_contention` — combines per-agent and per-host resource
  contention into a single tier and `GateStatusSnapshot` for the
  CLI's `sharecli status` view.
- `coalesce_meters`, `slot_queue_meters`, `operator_pool_status` —
  global meters exposed to the operator pool status endpoint.
- `detect`, `proc_scan` — runtime pattern matching and `/proc`
  accounting shared with `sharecli-core`.

## Quick start

```rust
use sharecli_fleet::{AgentContentionTier, effective_gate_decision_for_tier};

let tier = AgentContentionTier::Yellow;
let decision = effective_gate_decision_for_tier(tier);
// -> "proceed-with-loud-retry"
```

## Build

```bash
cargo build -p sharecli-fleet --release
```

## Configuration

- **NATS URL** — passed explicitly to `FleetRegistry::connect(url)`.
  This crate reads no environment variable for it. The exported
  default constant is
  `sharecli_fleet::DEFAULT_COORDINATOR = "nats://localhost:4222"`.
- `RUST_LOG` — tracing filter (defaults to `sharecli_fleet=info`).

## License

MIT — matches `license = "MIT"` in this crate's `Cargo.toml` and the
repository `LICENSE` file.
