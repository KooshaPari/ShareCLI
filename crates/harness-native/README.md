# harness-native

> Helios Shield native strategies, cache, queue, breaker — Unix-only.

`harness-native` provides the Linux / macOS process-ancestry
detection used by sharecli to tell "the agent is the foreground
process" from "the agent's child shell called out to a sibling tool".
It is the nix counterpart to the host-level `is_agent` heuristics in
`sharecli-fleet::detect`.

## Modules

- `find_real` — walk `/proc/<pid>/status` + `ppid` chain to find the
  topmost non-shell ancestor; used to label a process as
  agent-attributable for thermal accounting.
- `strategies` — small strategies for cache, queue, and breaker
  patterns tailored to native (non-WASM) hosts.
- `is_agent` — public predicate combining the ancestry walk with the
  runtime pattern table from `sharecli-fleet::detect`.

The crate re-exports `resolve_operator_queue_priority`,
`QueuePriority`, and `QUEUE_PRIORITY_ENV` from `sharecli-ipc` so
callers can import operator-queue configuration from a single crate
on Unix hosts.

## Platform

| Target | Status |
|--------|--------|
| Linux | Full support (`/proc`-based ancestry). |
| macOS | Full support (sysctl + `proc_pidinfo` ancestry). |
| Windows | Not built — Windows tray uses `sharecli-tray-windows` + `sharecli-ipc` directly. |
| WASM | Not applicable. |

## Quick start

```rust
use harness_native::find_real::topmost_ancestor;

let pid = std::process::id();
let real = topmost_ancestor(pid)?;
println!("topmost non-shell ancestor: pid={} cmd={:?}", real.pid, real.cmd);
```

## Build

```bash
cargo build -p harness-native --release
```

## License

Apache-2.0 OR MIT (dual).
