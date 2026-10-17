# sharecli-session

> Agent session and recovery state model.

`sharecli-session` owns the durable record of an agent session: which
harness, which conversation id, which working directory, the resume
argv, the recovery plan, and the typed RPC surface that the
`sharecli session` subcommand exposes to operator tooling.

## Modules

- `state` — `AgentSession` row, `SessionStore` trait, in-memory and
  sqlite-backed implementations.
- `adapter` — harness-specific session adapters (forge, codex, jcode,
  ghostty) and the `forge()` / `codex()` constructors that bake the
  right `--conversation-id` argv into the resume.
- `rpc` / `rpc_transport` / `rpc_tests` — typed JSON-RPC service
  (`inspect`, `recovery_plan`, ...) over the IPC transport.
- `recovery` — recovery-plan generation given a stale-session age.
- `discovery` / `resolver` — finding live sessions on disk or via the
  IPC daemon.
- `events` / `ledger` / `layout` / `migration` — on-disk layout,
  schema migrations, and the operator-visible event ledger.

## Quick start

```rust
use sharecli_session::{AgentSession, SessionStore};

let store = SessionStore::open("/var/lib/sharecli/sessions.db")?;
let session = AgentSession::forge("conv-1234", "/home/agent/proj");
store.upsert(&session)?;
let rows = store.list()?;
assert_eq!(rows[0].id, session.id);
```

## Recovery plan

```rust
use sharecli_session::SessionService;

let svc = SessionService::new(store);
let plan = svc.recovery_plan(chrono::Duration::hours(1))?;
// -> Vec<RecoveryAction>: nothing to do if no sessions older than 1h
```

## Build

```bash
cargo build -p sharecli-session --release
```

## License

Apache-2.0 OR MIT (dual).
