# 02 — Process lifecycle data flow

This diagram traces the data flow for the four core process-lifecycle
operations: **start**, **ps**, **stop**, **status**. Together they
make up the FR-001 (managed process lifecycle) and FR-007 (resource
and syscall-relevant watch) functional requirements.

```mermaid
sequenceDiagram
  autonumber
  actor User
  participant Shell
  participant CLI as sharecli binary<br/>(src/main.rs)
  participant Core as sharecli-core<br/>ProcessPool
  participant Spawn as spawn-core-sys<br/>(Zig fork+exec)
  participant Kernel
  participant IPC as sharecli-ipc<br/>(NDJSON-RPC)
  participant Fleet as sharecli-fleet<br/>Registry
  participant Sess as sharecli-session<br/>Ledger
  participant TUI as sharecli-thermal-tui

  rect rgb(15, 23, 42)
    note over User,Shell: sharecli start myproject --harness claude
    User->>Shell: $ sharecli start myproject --harness claude
    Shell->>CLI: execve("sharecli", [...])
    CLI->>CLI: clap parses args into Cli { command: Start { ... } }
    CLI->>Core: ProcessPool::start(project, harness, cwd, args)
    Core->>Core: validate project, check resource limits (FR-005)
    Core->>Spawn: zig_spawn(argv, envp, cwd)
    Spawn->>Kernel: fork()
    Kernel-->>Spawn: child pid
    Spawn->>Kernel: execve(argv[0], ...)
    Kernel-->>Spawn: (running)
    Spawn-->>Core: ChildHandle { pid, stdout, stderr }
    Core->>Core: register ChildHandle in process table
    Core->>IPC: publish ProcessEvent::Started { pid, project, harness }
    IPC->>Fleet: NDJSON-RPC over Unix socket
    Fleet->>Fleet: update operator_pool_status, thermal aggregation
    Core-->>CLI: Ok(ProcessInfo { pid, ... })
    CLI-->>Shell: "started myproject (claude) pid=4242"
    Shell-->>User: stdout
  end

  rect rgb(15, 23, 42)
    note over User,TUI: sharecli ps --all --json
    User->>Shell: $ sharecli ps --all --json
    Shell->>CLI: execve("sharecli", ["ps","--all","--json"])
    CLI->>Core: ProcessPool::list(Filter::all())
    Core->>Core: read managed map (empty in a fresh CLI invocation)
    Core->>Fleet: GET /operator_pool_status (over IPC)
    Fleet-->>Core: PoolOperatorPanel { panels: [...] }
    Core->>Core: merge host proc-agent scan + fleet pool/status panels
    Core-->>CLI: Vec<ProcessInfo>
    CLI->>CLI: serde_json::to_string_pretty
    CLI-->>Shell: JSON to stdout
    Shell-->>User: pretty-printed JSON
  end

  rect rgb(15, 23, 42)
    note over User,Kernel: sharecli stop --pid 4242
    User->>Shell: $ sharecli stop --pid 4242
    Shell->>CLI: execve("sharecli", ["stop","--pid","4242"])
    CLI->>Core: ProcessPool::stop(Pid(4242), force=false)
    Core->>Spawn: zig_kill(pid, SIGTERM)
    Spawn->>Kernel: kill(4242, SIGTERM)
    Kernel-->>Spawn: (signal sent)
    Spawn->>Kernel: waitpid(4242, ...)
    Kernel-->>Spawn: exit status
    Spawn-->>Core: ChildHandle::Exited { code }
    Core->>Core: remove from process table
    Core->>IPC: publish ProcessEvent::Exited { pid, code }
    IPC->>Fleet: NDJSON-RPC
    IPC->>Sess: append to session ledger (FR-005)
    Core-->>CLI: Ok(ExitInfo { code, duration })
    CLI-->>Shell: "stopped pid=4242 exit=0"
  end

  rect rgb(15, 23, 42)
    note over User,TUI: sharecli status
    User->>Shell: $ sharecli status
    Shell->>CLI: execve("sharecli", ["status"])
    CLI->>Core: ProcessPool::health_snapshot()
    Core->>Fleet: GET /health (over IPC)
    Fleet-->>Core: HealthSnapshot { thermal: Yellow, ... }
    Core-->>CLI: HealthSnapshot
    CLI-->>Shell: pretty-printed table
    Shell-->>User: terminal output
    note right of TUI: TUI subscribes via sharecli-fleet<br/>websocket; the CLI is not in this path
  end
```

## Key invariants

1. **The CLI binary never holds process state across invocations.**
   `sharecli-core::ProcessPool` is purely in-memory: `ProcessPool::new()`
   starts with an empty managed-process map, and `list()` / `find()` read
   only that map. Nothing reloads a previous invocation's managed
   processes, so a fresh `ps` reports zero *managed* rows; host-wide
   inventory comes from the independent host proc scan
   (`proc::AgentProcSnapshot::capture()`, the `sharecli proc` command),
   and the pool/status panels in `ps --all --json` are fetched over IPC
   from sharecli-fleet. The CLI process is just a parser + formatter.
2. **`spawn-core-sys` is the only crate that talks to the kernel
   for process spawn.** All fork/exec/waitpid/kill calls flow
   through it. The Rust stub on Windows returns
   `Unsupported` for `zig_spawn`/`zig_waitpid` (see
   `crates/spawn-core-sys/README.md`).
3. **sharecli-fleet is informed via IPC, not by sharing memory.**
   `sharecli-core` publishes `ProcessEvent::*` over the
   `sharecli-ipc` NDJSON-RPC transport. Fleet receives the
   events, updates the `operator_pool_status` registry, and
   exposes them to the TUI over WebSocket.
4. **The TUI is a pure consumer.** It subscribes to fleet
   over WebSocket and never calls into the CLI binary. A user
   can run `sharecli thermal` in one terminal while starting
   and stopping harnesses from another.
5. **FUSE writes carry provenance on the backing file, not in a
   ledger** — `sharecli-fuse` stamps `user.sharecli.*` xattrs (or NTFS
   ADS on Windows) and never appends to `sharecli-session`; see
   [03-fuse-write-provenance.md](03-fuse-write-provenance.md).

## Failure-mode annotations

- If `sharecli-ipc` is unreachable when `start` runs, the
  local spawn still succeeds. Fleet simply never sees the
  event. `sharecli ps` will still return the local process
  table; it just won't include fleet panels. (Degraded mode,
  not a failure.)
- If the user runs `sharecli stop --all --force` on a project
  with no processes, the command returns Ok with an empty
  result. The CLI does not error.
- If the TUI's WebSocket disconnects, the TUI re-renders with
  the last-known snapshot and a "stale" badge. It does not
  re-query the CLI.

See [docs/adr/0008-airlock-convergence-contract-capture.md](../../adr/0008-airlock-convergence-contract-capture.md)
for the contract surface between CLI and fleet.
