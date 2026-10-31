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
- `fuse_guard` — FUSE write-provenance session ids keyed off the command key.
- `hypervisor` — `Hypervisor` itself: `run` / `run_queued`, thermal-gate
  check, coalesce-via-lock.
- `proc_scan` — `/proc`-style process and FD accounting on Unix.
- `speculation` — speculative pre-spawn / coalesce-ahead primitives.
- `thermal_gate` — `ThermalGate` decisions (Allow / Warn / Refuse) derived
  from hardware pressure plus host agent contention.
- `transaction_contract` — pure, actor-neutral admission predicates for the
  proposed global transaction contract. Not an OS effect broker and not a
  durable ledger.
- `types` — `HypervisorConfig`, `SpawnRequest`, `SpawnOutcome`, and the
  cache-result conversions.

## Quick start

`Hypervisor::new` is **synchronous** and takes a cache root directory.
`SpawnRequest::new` takes the full argv vector, the working directory, and
the child's env overrides. `SpawnOutcome` carries no command key — recompute
it with `command_key` from the same argv/cwd/env you passed in.

```rust
use std::path::PathBuf;

use sharecli_core::{command_key, Hypervisor, SpawnRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Cache root; `Hypervisor::new` is not async and defaults the queue to
    // `{cache_root}/queue`.
    let hv = Hypervisor::new("/tmp/sharecli-cache");

    let argv = vec!["ruff".to_owned(), "check".to_owned(), ".".to_owned()];
    let cwd = PathBuf::from(".");
    let env = Vec::new();

    let request = SpawnRequest::new(argv.clone(), cwd.clone(), env.clone());
    let outcome = hv.run(request).await?;

    // `SpawnOutcome` has no `command_key` field; derive it from the request.
    let key = command_key(&argv, &cwd, &env);
    println!("cache key: {}", key.0);

    if outcome.from_cache {
        println!("cache hit");
    } else {
        println!("spawned, exit={}", outcome.exit_code);
    }
    println!(
        "stdout: {} bytes, stderr: {} bytes",
        outcome.stdout.len(),
        outcome.stderr.len()
    );
    Ok(())
}
```

## Build

```bash
cargo build -p sharecli-core --release
```

## License

Apache-2.0 OR MIT (dual).
