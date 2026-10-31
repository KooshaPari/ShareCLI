# sharecli-core

> Hypervisor engine tier — the central entry point for spawning managed
> processes in the sharecli stack.

`sharecli-core` wires the coalescing cache from `sharecli-ipc`
(Lock-Wait-Cache deduplication) with real OS process spawning via
`tokio::process::Command`. It is the `Hypervisor::run(SpawnRequest)` caller
path that every sharecli CLI subcommand ultimately funnels into.

## Architecture

```text
caller -> Hypervisor::run(SpawnRequest)
   |
   +-- ThermalGate::poll()          -> Green/Yellow: proceed
   |                                 Red: sleep-retry (loud) or Err
   |
   +-- ResourceWatchSample::capture() -> FD / net watch on every run
   |
   +-- nocache_args match?          -> SlotQueue::with_slot -> spawn (no cache)
   |
   +-- compute command_key (sharecli-ipc)
   |
   +-- CoalesceCache::with_lock
          |
          +-- [cache hit]  -> SpawnOutcome { from_cache: true }
          |
          +-- [cache miss] -> spawn + store -> from_cache: false
```

## Modules

- `detect` — runtime pattern matching (known agent families, system tools).
- `proc_scan` — `/proc`-style process and FD accounting on Unix.
- `speculation` — speculative pre-spawn / coalesce-ahead primitives.
- `transaction_contract` — typed SpawnRequest / SpawnOutcome contracts.

## Quick start

```rust
use sharecli_core::Hypervisor;

let hv = Hypervisor::new().await?;
let outcome = hv.run(SpawnRequest::new("ruff", &["check", "."])).await?;
if outcome.from_cache {
    println!("cache hit: {}", outcome.command_key);
} else {
    println!("spawned pid={}, exit={}", outcome.pid, outcome.exit_code);
}
```

## Build

```bash
cargo build -p sharecli-core --release
```

## License

Apache-2.0 OR MIT (dual).
