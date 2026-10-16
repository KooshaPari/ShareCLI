# ARCHITECTURE.md — phenotype-teamcomm

**Version:** 1.0
**Date:** 2026-06-14
**Status:** Active (pre-alpha)
**Scope:** Inter-agent coordination infrastructure for the Phenotype ecosystem

---

## 1. Overview

`phenotype-teamcomm` is the shared coordination substrate for the Phenotype engineering ecosystem. It enables multiple AI-driven coding agents (and the humans steering them) to work on the same repository without conflicting edits, stale context, or lost messages.

The system is built as a **Rust Cargo workspace** with five crates. At its core is a **hub-and-spoke** architecture: a single long-running `teamcomm-daemon` process acts as the central hub, while agent clients (forge, codex, claude, copilot, etc.) connect as lightweight spokes over a local Unix-domain socket. All coordination state — sessions, file reservations, inbox messages, live agent state, and hook events — is managed by the daemon and exposed through a JSON-RPC 2.0 wire protocol.

The project is currently at **M0** (minimum viable surface). In-memory state, basic session lifecycle, and stub surfaces for reservations, inbox, state, and discovery are implemented. Persistence (SQLite), first-class threads, structured message types, and cross-machine networking are planned for M1–M3.

---

## 2. Components

### 2.1 Workspace Layout

```
phenotype-teamcomm/
├── Cargo.toml                 # workspace root (resolver = 2, MSRV 1.75)
├── crates/
│   ├── teamcomm-protocol/     # wire types and serde schemas
│   ├── teamcomm-client/       # embeddable async client library
│   ├── teamcomm-daemon/       # long-running coordinator (lib + bin)
│   ├── teamcomm-cli/          # human-facing CLI (`teamcomm` binary)
│   └── teamcomm-mcp/          # MCP server (bin + mcp/manifest.json)
├── schemas/                   # protocol schema sources (planned)
├── docs/                      # design notes (SDD.md, PROTOCOL.md)
├── tests/                     # cross-crate integration tests (planned)
├── configs/                   # hook snippets for forge/codex/claude/copilot
└── .github/workflows/ci.yml   # quality gates (check, test, fmt, clippy)
```

### 2.2 Crate Responsibilities

#### `teamcomm-protocol` — The Contract Layer

A pure data crate with zero async dependencies. Defines the canonical wire types for every domain:

| Module | Key Types | Purpose |
|--------|-----------|---------|
| `session.rs` | `Session`, `SessionRegistration`, `SessionSummary`, `AgentType` | Agent identity, registration payload, lightweight summaries |
| `reservation.rs` | `Reservation`, `ClaimRequest`, `ClaimResult`, `ReservationMode` | Advisory file locking with TTL |
| `inbox.rs` | `InboxMessage`, `InboxQuery`, `Priority` | Durable, addressed messaging |
| `state.rs` | `LiveState`, `AgentStatus` | Focus file, branch, worktree, status snapshot |
| `discovery.rs` | `DiscoveryQuery`, `DiscoveryResult` | "Who else is working on what" filters |
| `hook_event.rs` | `HookEvent`, `HookEventType` | Append-only lifecycle event stream |
| `rpc.rs` | `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcErrorResponse`, `RpcId` | JSON-RPC 2.0 envelope types |
| `error.rs` | `TeamcommError`, `ErrorCode`, `error_code()` | Error enum and JSON-RPC code mapping |

All types are `serde`-friendly with stable wire shapes. The protocol crate intentionally stays agnostic of the concrete method set — `params` and `result` are `serde_json::Value` in the RPC envelope so the crate never needs to know about daemon-specific methods.

#### `teamcomm-daemon` — The Hub

The central coordinator. Exposes a `lib.rs` surface (for embedding and testing) and a `main.rs` binary (`teamcomm-daemon`) with `start`/`stop`/`status` CLI commands.

**Internal modules:**

| Module | Responsibility |
|--------|--------------|
| `listener.rs` | Unix-domain socket accept loop, per-connection JSON-RPC dispatch, signal handling (SIGINT/SIGTERM), graceful shutdown |
| `handlers.rs` | JSON-RPC method implementations: `session.*`, `reservation.*`, `inbox.*`, `state.*`, `discover.agents` |
| `state.rs` | In-memory shared state (`AppStateInner` behind `tokio::sync::RwLock`) with id minters (`sess_<uuid>`, `resv_<uuid>`, `msg_<uuid>`) |
| `config.rs` | `DaemonConfig` — socket/pid paths, heartbeat timeouts, log level |
| `pid.rs` | PID file write/read/remove with stale-PID recovery and `kill -0` liveness probe |
| `error.rs` | Daemon-specific `TeamcommError` enum with `thiserror` and JSON-RPC code mapping |

The daemon is wrapped in a `Daemon` struct that holds configuration, shared state, and a `tokio::sync::watch` shutdown channel. `DaemonHandle` provides cheap, cloneable shutdown triggers.

**Key runtime constants:**
- `HEARTBEAT_INTERVAL_SEC = 30` — recommended client heartbeat cadence
- `LEASE_TTL_SEC = 90` — maximum silence before a session may be reaped
- `MAX_LINE_BYTES = 1 MiB` — per-request line size limit
- `IDLE_TIMEOUT = 5 min` — connection idle timeout

#### `teamcomm-client` — The Spoke Library

An embeddable async client that other agents link into their own process. Connects over a Unix-domain socket and dispatches JSON-RPC 2.0 requests.

**Design:**
- `Client::connect(path)` or `Client::connect_default()` to establish a stream
- `Client::call(method, params)` sends a request, reads one response line, and returns the raw `result` payload
- Convenience methods for every domain: `session_register`, `reservation_claim`, `inbox_post`, `state_set`, `discover_agents`, etc.
- The stream is split for read/write, then reunited after each call so the same `Client` handle can be reused

The client uses `anyhow` for error propagation and `serde_json::Value` for results, keeping the API flexible for callers that may not want to import the full protocol crate.

#### `teamcomm-cli` — Human Operator Interface

The `teamcomm` binary is the human-facing command line. It connects to a running daemon and dispatches subcommands as JSON-RPC requests.

**Subcommand groups:**
- `daemon` — local process management (`start`, `stop`, `status`); does not talk to the daemon over RPC
- `sessions` — `list` (with `--watch` mode), `show`
- `reservations` — `ls`, `claim`, `release`
- `inbox` — `list`, `read`, `post`
- `state` — `show`, `set-focus`, `set-status`
- `discover` — `agents` (filter by path, branch, capability)

**Internal modules:**
- `connect.rs` — Unix socket connection helpers
- `rpc.rs` — JSON-RPC request/response framing with `RpcCallError` (distinguishes `MethodNotFound`, `Transport`, and `Server` errors)
- `output.rs` — Pretty-printing helpers using `comfy-table` for tabular output and `serde_json` for JSON mode

The CLI uses an M0 placeholder pattern: if a daemon method returns `MethodNotFound` or the daemon is unreachable, the CLI prints a friendly placeholder message and exits 0 rather than failing hard.

#### `teamcomm-mcp` — Model Context Protocol Bridge

An MCP server that exposes teamcomm primitives as MCP tools to MCP-aware agents (Codex, Claude, etc.). Communicates over stdio (one JSON-RPC request per line on stdin, one response per line on stdout).

**M0 status:** Every tool returns a mocked successful response without touching real daemon state. The manifest in `mcp/manifest.json` declares 11 tools: `register_session`, `deregister_session`, `list_sessions`, `claim_file`, `release_file`, `list_claims`, `post_message`, `read_inbox`, `announce_focus`, `set_status`, `discover_agents_for_path`.

**M1 plan:** Replace stub handlers with calls into the `teamcomm-client` library (or direct daemon IPC) so the MCP server becomes a real bridge.

---

## 3. Data Flow

### 3.1 Session Lifecycle

```
Agent starts
  │
  ▼
session.register ──► Daemon assigns session_id + lease_ttl_sec
  │                    │
  │                    ▼
  └──────────────► Heartbeat loop (every 30s)
                          │
                          ▼
                    Daemon refreshes last_heartbeat
                          │
              ┌───────────┘
              │
              ▼
   No heartbeat within 90s ──► Daemon reaps session (M1)
              │
              ▼
session.deregister ──► Daemon removes session + releases reservations
```

**Idempotency:** `session.register` is idempotent on `pid`. If the same pid re-registers, the existing session is reused and its `last_heartbeat` is bumped. This makes agent restarts safe.

**Deregister idempotency:** `session.deregister` on an unknown session returns `{"ok": true}` so shutdown cleanup scripts are safe to run unconditionally.

### 3.2 Reservation Flow

```
Agent calls reservation.claim
  │
  ▼
Daemon checks session exists
  │
  ▼
Daemon scans existing reservations for conflicts:
  - Same path?
  - Different session?
  - Not expired?
  - Mode conflicts? (Read < Write < Exclusive)
  │
  ├─► Conflicts found ──► Return reservation + conflicts list
  │
  └─► No conflicts ──► Insert reservation, return reservation + []
```

Reservations are **advisory**, not enforced by the OS kernel. Agents are cooperative peers and are expected to respect conflict reports voluntarily.

### 3.3 Inbox Flow

```
Agent A calls inbox.post ──► Message stored in recipient's queue
                                   │
                                   ▼
Agent B calls inbox.list ──► Daemon returns unread messages
                                   │
                                   ▼
                              Messages marked as read upon delivery
```

The inbox is **pull-based**, not push-based. Agents poll for unread messages; the daemon marks them as read upon delivery. This aligns with the MCP tool-call model where the agent initiates all communication.

### 3.4 Live State Flow

```
Agent calls state.set ──► Daemon stores LiveState snapshot
                                   │
                                   ▼
Other agents call discover.agents ──► Daemon filters sessions
                                      by path, branch, capability
```

`LiveState` includes `focus_file`, `focus_branch`, `worktree`, `status`, and `last_heartbeat`. Discovery queries return `SessionSummary` objects with matching filters.

### 3.5 Hook Event Flow

```
Agent lifecycle events (file read, file written, plan announced, etc.)
  │
  ▼
Agent emits HookEvent via JSON-RPC or hook script
  │
  ▼
Daemon appends to event stream
  │
  ▼
Subscribers (CLI tail, MCP surface, other agents) filter and consume
```

Hook events are **append-only** and typed (`HookEventType` enum). The payload is a free-form `serde_json::Value` so the protocol stays flexible as new event variants are added.

### 3.6 Wire Protocol

**Transport:** JSON-RPC 2.0 over newline-delimited lines on a Unix-domain socket (daemon) or stdio (MCP server).

**Request envelope:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "session.register",
  "params": { "pid": 1234, "agent_type": "forge" }
}
```

**Success response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "session_id": "sess_...", "lease_ttl_sec": 90 }
}
```

**Error response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": { "code": -32004, "message": "not found: session xyz" }
}
```

**Notifications:** `id == None` denotes a notification; the server must not reply.

---

## 4. Key Invariants

### 4.1 Session Uniqueness
- A session is uniquely identified by `session_id` (UUID v4, prefixed `sess_`)
- The `sessions_by_pid` reverse index guarantees at most one session per OS process id at any time
- Re-registration of the same pid reuses the existing session id

### 4.2 Reservation Integrity
- A reservation is uniquely identified by `reservation_id` (UUID v4, prefixed `resv_`)
- Reservations are bound to a valid session; the daemon verifies session existence before granting a claim
- Conflicts are computed at claim time using mode ordering: `Read < Write < Exclusive`
- Expired reservations are filtered out in `reservation.list` but are not proactively reaped in M0

### 4.3 Inbox Durability (M0 Best-Effort)
- Messages are keyed by `message_id` (UUID v4, prefixed `msg_`)
- Messages are stored per-recipient in a `HashMap<String, Vec<InboxMessage>>`
- `inbox.list` with `unread_only: true` returns only unread messages and marks them as read
- `inbox.read` by `message_id` marks the specific message as read

### 4.4 State Consistency
- `LiveState` is always associated with a known session; `state.set` fails with `NotFound` if the session does not exist
- `SessionSummary` objects returned by `discover.agents` combine base `Session` data with optional `LiveState` data
- If no live state has been published for a session, discovery falls back to `AgentStatus::Idle`

### 4.5 Error Code Stability
- JSON-RPC standard codes: `-32700` (parse), `-32600` (invalid request), `-32601` (method not found), `-32602` (invalid params), `-32603` (internal error)
- Teamcomm application codes: `-32001` (unauthorized), `-32003` (already exists), `-32004` (not found), `-32005` (conflict)
- These mappings are stable and tested; clients may rely on them for branching logic

### 4.6 Unsafe Code Prohibition
- The entire workspace declares `unsafe_code = "forbid"` in `[workspace.lints.rust]`
- All crates are `forbid(unsafe_code)`

---

## 5. Cross-Cutting Concerns

### 5.1 Observability

**Tracing:** All crates use `tracing` + `tracing-subscriber` with `EnvFilter` driven by `RUST_LOG`. The default level is `info`. Daemon logs include structured fields (`session_id`, `reservation_id`, `pid`, etc.) for correlation.

**Error Logging:** Parse errors and malformed requests are logged at `warn` level with the peer address and error details. Connection-level failures are logged at `debug` level.

### 5.2 Concurrency Model

- The daemon uses **tokio** multi-thread runtime
- Shared state is protected by a single `tokio::sync::RwLock<AppStateInner>` behind an `Arc`
- Handlers acquire a write lock or read lock as needed; lock contention is acceptable for M0 because all state is in-memory and operations are sub-millisecond
- The listener spawns one task per connection; in-flight tasks are tracked in an `Arc<Mutex<Vec<JoinHandle>>>` and aborted on shutdown

### 5.3 Shutdown and Cleanup

- Graceful shutdown is triggered by:
  - External `DaemonHandle::shutdown()` call (e.g., from a CLI `daemon.stop` command in future)
  - OS signal (SIGINT or SIGTERM) caught by a dedicated signal handler task
- On shutdown:
  1. The listener stops accepting new connections
  2. In-flight connection tasks are aborted
  3. The Unix socket file is removed
  4. The PID file is removed (by the daemon on exit, or by the CLI if stale)

### 5.4 Configuration and Paths

- Default socket: `$XDG_RUNTIME_DIR/teamcomm/daemon.sock` (or `/tmp/teamcomm/daemon.sock`)
- Default pid file: `$XDG_RUNTIME_DIR/teamcomm/daemon.pid` (or `/tmp/teamcomm/daemon.pid`)
- All paths are resolved at `DaemonConfig` construction time; the listener does no runtime path manipulation
- The CLI and client library share the same default path logic via `dirs::runtime_dir()`

### 5.5 Testing Strategy

- **Unit tests:** Every protocol type has roundtrip serde tests. Every daemon handler has async tests using `new_state()`. Every utility module (config, pid, error) has isolated tests.
- **Integration tests:** `crates/teamcomm-daemon/tests/integration.rs` and `crates/teamcomm-mcp/tests/` exercise cross-module behavior. The listener test in `listener.rs` spawns a real daemon, connects over a Unix socket, sends a request, and verifies the response.
- **Quality gates:** `cargo check --workspace`, `cargo test --workspace`, `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` must all pass in CI.

### 5.6 Agent Hook Integration

The `configs/` directory contains hook snippets for lifecycle event bridging:
- `forge/`, `codex/`, `claude/`, `copilot/`, `droid/` — one script per lifecycle event
- Convention: `<agent>_<event>.sh` (or `.ps1` on Windows)
- Scripts are **idempotent** — the daemon is the source of truth; hooks are advisory notifications

---

## 6. Future Considerations

### 6.1 M1: Persistence and State Expansion

- **SQLite-backed state:** Replace `AppStateInner` (in-memory `HashMap`s) with a SQLite store. Enables durability across daemon restarts and reduces memory footprint for large agent fleets.
- **Session reaper:** A background task that periodically scans sessions and removes those whose `last_heartbeat` exceeds `LEASE_TTL_SEC`.
- **5-state status enum:** Migrate `AgentStatus` from 4-state (`Idle`, `Working`, `Blocked`, `Done`) to 5-state (`online`, `idle`, `busy`, `blocked`, `offline`) per the canonical AGSLAG model.
- **Structured message types:** Add `message_type` field to `InboxMessage` (e.g., `status_update`, `task_assignment`, `query`, `response`, `blocker_report`, `knowledge_share`, `feedback`, `request_assistance`, `decision_log`).
- **First-class threads:** Add `Thread` struct with CRUD operations (`thread.create`, `thread.join`, `thread.add_participant`, `thread.remove_participant`, `thread.get_details`, `thread.list`). Messages carry an optional `thread_id`.
- **MCP real bridge:** Replace stub handlers with actual `teamcomm-client` calls so the MCP server becomes a real daemon proxy.

### 6.2 M2: MCP Tool-Call Surface

- Full MCP server integration with the running daemon
- Tool-call schemas synchronized with `mcp/manifest.json`
- Support for MCP `initialize` handshake and capability negotiation

### 6.3 M3: Cross-Machine Networking

- TCP transport option alongside Unix-domain sockets
- Optional TLS for inter-machine authentication
- Federation: multiple daemons can gossip session/reservation state

### 6.4 Scalability and Performance

- **Lock granularity:** The single `RwLock` on `AppStateInner` will become a bottleneck under high concurrency. M1+ should shard by domain (sessions, reservations, inbox) or use an async channel-based actor model.
- **Connection pooling:** The client currently splits and reunites the stream on every call. A persistent connection pool would reduce syscall overhead.
- **Event streaming:** Hook events are currently stored in memory. A ring buffer or persistent log (e.g., SQLite WAL or a dedicated event store) would support replay and long-term audit.

### 6.5 Security and Hardening

- **Authentication:** M0 trusts any local process. M1+ should add token-based or capability-based auth for the Unix socket.
- **Rate limiting:** Heartbeat flood protection, reservation claim rate limits, and inbox spam prevention.
- **Path validation:** Reservations should validate that paths are within the declared `working_dir` to prevent cross-repo locking.

### 6.6 Observability Enhancements

- **Metrics:** Export counters (sessions registered, reservations claimed, messages posted) via OpenTelemetry or Prometheus.
- **Structured logging:** Add `trace_id` / `request_id` propagation across RPC calls.
- **Health checks:** A `daemon.health` JSON-RPC method for load balancer and supervisor integration.

---

## 7. References

| Document | Path | Purpose |
|----------|------|---------|
| README | `README.md` | Project overview, quickstart, five functional domains |
| SPEC | `SPEC.md` | Scope, wire protocol, core methods, quality gates |
| SDD | `docs/SDD.md` | Software design document: patterns, gaps, glossary |
| AGENTS | `AGENTS.md` | Agent working guidelines, stack, layout, quality rules |
| Protocol crate | `crates/teamcomm-protocol/src/` | Wire type definitions |
| Daemon handlers | `crates/teamcomm-daemon/src/handlers.rs` | JSON-RPC method implementations |
| Daemon listener | `crates/teamcomm-daemon/src/listener.rs` | Socket accept loop and dispatch |
| MCP manifest | `crates/teamcomm-mcp/mcp/manifest.json` | MCP tool catalogue |

---

*This document is a living artifact. Update it when the architecture changes, and validate against the source of truth in the crate source files.*
