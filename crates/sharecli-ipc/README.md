# sharecli-ipc

> Coalesce / debounce / queue tier of the sharecli OS-process hypervisor.

Implements the **Lock-Wait-Cache** pattern: when N isolated agent
processes issue the same command concurrently (e.g. 8 agents all run
`ruff check .`), only one execution actually runs. The other 7 block
on an advisory `flock` and then read the result written by the winner.

## Building blocks

1. **`command_key`** — SHA-256 of `(argv + cwd + relevant env)` into
   a deterministic hex key.
2. **`CoalesceCache`** — atomic JSON store: `lookup` / `store` /
   `with_lock`.
3. **`CachedResult`** — the serialisable `exit_code + stdout + stderr`
   bundle.
4. **`SlotQueue` / `PriorityQueue`** — N-slot concurrency limiter for
   mutating paths.
5. **`has_nocache_arg`** — Feb `nocache_args` detection (coalesce ->
   queue fallback).

## TTL + debounce

- **TTL** — `lookup` treats entries whose mtime age is >= configured
  TTL as a miss (default `CoalesceCache::DEFAULT_TTL` = 300s). `store`
  sweeps stale `*.json` entries under the cache root.
- **Debounce** — on a miss, `with_lock` waits `debounce` then
  re-checks the cache so a concurrent store completed in-window is
  shared instead of re-running (default off).

## Queue + nocache (FR-008)

Mutating argv (`--fix`, `--force`, `--write`, ...) MUST NOT hit
`CoalesceCache`. Use `should_bypass_coalesce` then
`SlotQueue::with_slot`. Hypervisor callers: see
`sharecli_core::Hypervisor::{queue, run}`.

## Quick start

```rust
use sharecli_ipc::{CoalesceCache, command_key, CachedResult};

let key = command_key(&argv, cwd, &env_subset);
let cache = CoalesceCache::new("/var/cache/sharecli");
match cache.lookup(&key)? {
    Some(hit) => println!("cache hit, exit={}", hit.exit_code),
    None => {
        // ... run the actual command ...
        cache.store(&key, &CachedResult::from(&output))?;
    }
}
```

## Build

```bash
cargo build -p sharecli-ipc --release
```

## Configuration

There are **no environment variables** for TTL, debounce, or the cache
root — all three are set per instance at construction time:

| Setting | Default | How to change it |
|---------|---------|------------------|
| Cache root | none (caller supplies it) | first argument to `CoalesceCache::new` / `with_ttl` / `with_options` |
| TTL | `CoalesceCache::DEFAULT_TTL` = 300s | `CoalesceCache::with_ttl(root, ttl)` or `with_options` |
| Debounce | `Duration::ZERO` (off) | `CoalesceCache::with_options(root, ttl, debounce)` |

Environment variables this crate *does* read (unrelated to coalesce
config): `SHARECLI_QUEUE_PRIORITY` (queue tier), `SHARECLI_IPC_SOCK` /
`SHARECLI_IPC_ADDR` (daemon transport), `SHARECLI_SESSION_DB` (durable
session store path), `XDG_RUNTIME_DIR` (serve-lock directory).

## License

MIT — matches `license = "MIT"` in this crate's `Cargo.toml` and the
repository `LICENSE` file.
