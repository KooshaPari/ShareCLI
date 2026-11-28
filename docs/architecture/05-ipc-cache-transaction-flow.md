---
title: sharecli-ipc Lock-Wait-Cache protocol
topic: CoalesceCache::with_lock — how concurrent agent commands share one execution
diagram_type: sequenceDiagram
source: crates/sharecli-ipc/src/{lib.rs,cache_key.rs}
audience: contributors debugging flakiness, designing new coalesce paths, reviewing FR-008 (queue + nocache) interaction
---

# sharecli-ipc Lock-Wait-Cache protocol

The `sharecli-ipc` crate is the coalesce / debounce / queue tier of the
sharecli OS-process hypervisor. When N isolated agent processes issue
the same command concurrently — say, eight `forge` agents all running
`ruff check .` in the same project — only **one** execution actually
runs; the other seven block on an advisory `flock` and then read the
result that the winner wrote.

This diagram captures the protocol that `CoalesceCache::with_lock`
implements (`crates/sharecli-ipc/src/lib.rs:286-366`). It is the source
of truth for "why doesn't this command re-run when I expect it to
share?" and "why is there a 30-second wait the first time a command
acquires after a -9-killed winner?".

The four roles are:

- **Agent A / Agent B** — two OS processes (typically `forge`, `codex`,
  `claude`, …) that called `Hypervisor::run` with the same argv + cwd
  + relevant env.
- **command_key** — `crates/sharecli-ipc/src/cache_key.rs`: SHA-256
  over `(argv + cwd + relevant_env)` → 64-char hex. The cache root is
  keyed off this hex.
- **CoalesceCache** — `crates/sharecli-ipc/src/lib.rs::CoalesceCache`.
  Owns the `<root>/<key>.lock` and `<root>/<key>.json` pair.
- **flock** — Linux/POSIX `fcntl(F_SETLKW)` advisory exclusive lock.
  The 30-second deadline (`acquire_lock_with_deadline`) is a Lane-7
  blocker (tiger) fix for hung `-9` winners (`lib.rs:330`).

```mermaid
sequenceDiagram
    autonumber
    participant A as Agent A (forge turn 1)
    participant B as Agent B (forge turn 2)
    participant K as command_key (SHA-256)
    participant Cache as CoalesceCache
    participant FS as <root>/<key>.lock + .json
    participant f as miss-closure (the actual run)

    Note over A,B: argv = ["ruff","check","."], cwd = /p, env = {RUSTFLAGS}
    A->>K: command_key(argv, cwd, env)
    B->>K: command_key(argv, cwd, env)
    K-->>A: key = "a3f9…c0e1"
    K-->>B: key = "a3f9…c0e1"

    par Agent A arrives first
        A->>Cache: with_lock(key, f)
        Cache->>FS: open(<root>/<key>.lock, O_CREAT|O_WRONLY)
        Cache->>FS: flock LOCK_EX (block, deadline 30s)
        Note right of Cache: acquire_lock_with_deadline<br/>(lib.rs:330) — bounds the wait
        Cache->>Cache: lookup(key) → None (miss)
        alt debounce != 0
            Cache->>Cache: sleep(debounce); lookup(key) → None
        end
        Cache->>f: invoke miss-closure
        f-->>Cache: T = CachedResult { exit_code = 0, stdout, stderr }
        alt cached.exit_code == 0
            Cache->>FS: write <root>/<key>.json (atomic temp + rename)
        else cached.exit_code != 0
            Note right of Cache: Lane-7 BLOCKER (tiger):<br/>failures are NOT persisted so<br/>siblings re-run their own.
        end
        Cache->>FS: drop lock_file → flock LOCK_UN
        A-->>A: return T (CoalesceHitKind::Miss)
    and Agent B arrives ~3 ms later
        B->>Cache: with_lock(key, f)
        Cache->>FS: open(<root>/<key>.lock, O_CREAT|O_WRONLY)
        Cache->>FS: flock LOCK_EX (block, deadline 30s)
        Note right of FS: Agent B blocks here<br/>until Agent A drops the file handle.
        Cache->>Cache: lookup(key) → Some(T) (HIT)
        Cache->>FS: drop lock_file → flock LOCK_UN
        B-->>B: return T (CoalesceHitKind::LockRecheck)
    end
```

## How to read this

1. **Both agents hash the same key.** That is the only signal the cache
   uses to recognise "same command"; the cache does **not** consult
   argv or cwd after the key has been computed.
2. **The flock is the gate, the lookup is the share.** Agent B does
   not get a hit because it scanned the JSON — it gets a hit because,
   after waking from the flock, the **re-check** at
   `crates/sharecli-ipc/src/lib.rs:334` sees a file that did not exist
   when B's `with_lock` started.
3. **30-second deadline is a hard cap.** A `-9`-killed holder leaks the
   fd but the kernel releases the flock; `acquire_lock_with_deadline`
   means a real blocker (NFS stall, wedged reader) cannot wedge siblings
   silently for the full TTL.
4. **Debounce is a second re-check window.** With `debounce_ms`
   configured, even the **first** arrival sleeps after a miss so that a
   store from a sibling that completed during the flock-wait is shared
   (`lib.rs:341-347`). Default is off (`Duration::ZERO`).
5. **Failures are not persisted.** Lane-7 BLOCKER (tiger): any
   `CachedResult` with `exit_code != 0` is returned to the caller but
   never written to `<root>/<key>.json`, so siblings will re-run their
   own copy rather than replay someone else's failure for the full TTL.

## CoalesceHitKind values

The `with_lock_detailed` companion returns a `CoalesceHitKind` alongside
the value so callers (and metrics) can distinguish:

| HitKind | When | Meaning for siblings |
|---------|------|----------------------|
| `LockRecheck` | Sibling stored during our flock-wait | Winner was already done |
| `DebounceRecheck` | Sibling stored during our debounce sleep | Race resolved inside the debounce window |
| `Miss` | We were first to acquire the flock | Pre-call ran and (if exit_code == 0) wrote the cache |

## Cross-references

- [`02-process-lifecycle.md`](02-process-lifecycle.md) — the broader
  CLI data flow that ends in this coalesce tier.
- `crates/sharecli-ipc/src/cache_key.rs` — exact hash function and the
  `CacheKeyMode` enum (argv + cwd + env vs. argv + cwd only).
- `crates/sharecli-ipc/src/handler.rs` — where `with_lock` is wired
  into `Hypervisor::run` for the cold path.
- `crates/sharecli-ipc/src/nocache.rs` — the `should_bypass_coalesce`
  filter that sends `--fix`, `--force`, `--write` to
  `SlotQueue::with_slot` instead of the cache.
- `docs/adr/0008-airlock-convergence-contract-capture.md` — how the
  CLI binary captures the result before the hypervisor touches it.
- `FUNCTIONAL_REQUIREMENTS.md` — FR-008 (queue + nocache) is the
  sibling feature that MUST bypass this path.