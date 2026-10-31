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
use sharecli_session::SessionStore;

let store = SessionStore::open("/var/lib/sharecli/sessions.db")?;
let plan = store.recovery_plan(chrono::Duration::hours(1))?;
// -> Vec<AgentSession>: the newest FRESH observation per surface, i.e. one
//    observed within the last hour. Observations older than `max_age` are
//    discarded (as are future-dated ones and ones with malformed timestamps).
//    Pass `Duration::zero()` or a negative duration and the call errors.
```

`max_age` bounds how stale an observation may be to still count as
recoverable. It is **not** a "sessions older than N" selector: raising
it widens the window of sessions considered live/recoverable, and
lowering it narrows it. The CLI exposes the same knob as
`sharecli session recovery-plan --max-age-seconds <N>` (default four
hours).

## Build

```bash
cargo build -p sharecli-session --release
```

## License

MIT — matches `license = "MIT"` in this crate's `Cargo.toml` and the
repository `LICENSE` file.
