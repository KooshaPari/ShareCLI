# sharecli architecture

This directory holds the canonical architecture diagrams for
sharecli. The diagrams are Mermaid; the source files are plain
Markdown and render natively in GitHub, Obsidian, JetBrains IDEs,
and the `mmdc` CLI from `@mermaid-js/mermaid-cli`.

## This lane: 1d-queue `docs/architecture-diagrams-2026-10-17`

This branch (`docs/architecture-diagrams-2026-10-17`) adds four
focused architecture diagrams that complement the 4h-queue lane
contribution (01–03). Each diagram in this branch is a single
Markdown file with one fenced `mermaid` block; the source code
references cited inside each diagram are the lines and crates that
justify the picture.

| # | Diagram | File | Type | Source pins |
|---|---------|------|------|-------------|
| 04 | Session state machine | [04-session-state-machine.md](04-session-state-machine.md) | `stateDiagram-v2` | `crates/sharecli-session/src/lib.rs:200`, `ledger.rs:102`, `recovery.rs:82` |
| 05 | IPC Lock-Wait-Cache protocol | [05-ipc-cache-transaction-flow.md](05-ipc-cache-transaction-flow.md) | `sequenceDiagram` | `crates/sharecli-ipc/src/lib.rs:286-366`, `cache_key.rs` |
| 06 | Tray backend platform matrix | [06-tray-backend-platform-matrix.md](06-tray-backend-platform-matrix.md) | `flowchart` | `crates/sharecli-tray-linux/Cargo.toml:22`, `crates/sharecli-tray-windows/Cargo.toml:14`, `Cargo.toml:244-260` |
| 07 | Clap-derive command dispatch | [07-clap-derive-command-dispatch.md](07-clap-derive-command-dispatch.md) | `flowchart` | `src/main.rs:59, 87-88, 856, 881, 978` |

## Reading order

1. **04-session-state-machine** — read when working on
   `sharecli-session`, the durable-session ledger, or recovery.
   The state diagram distinguishes the four `SessionState`
   variants and pins which crate function drives each transition.
2. **05-ipc-cache-transaction-flow** — read when debugging "why did
   this command re-run when I expected it to share?" or when adding
   a new command-keyed coalesce path. This is the canonical
   Lock-Wait-Cache protocol used by every FR-008 command path.
3. **06-tray-backend-platform-matrix** — read when porting the
   tray, debugging "tray icon missing on platform X", or auditing
   the cargo-dist release matrix. Pins the asymmetric tray stack
   (Linux single crate, Windows two components, macOS Swift-only).
4. **07-clap-derive-command-dispatch** — read when adding a new
   subcommand, debugging "my arg is parsed wrong", or reviewing the
   top-level dispatch table. Pins the `Cli::parse → match
   &cli.command` pipeline and every nested `#[command(subcommand)]`.

## Related diagrams (added by the 4h-queue lane)

The 4h-queue lane (`docs/4h-queue-lane-2026-10-17`) adds three
sibling diagrams that should be read alongside these:

- `01-system-overview.md` — crate topology (graph TD, 16 crates
  coloured by role).
- `02-process-lifecycle.md` — sequence diagram for the four core
  process-lifecycle operations (start / ps / stop / status).
- `03-fuse-write-provenance.md` — how the FUSE layer tags every
  write with `user.sharecli.session` and `user.sharecli.written_at`.

Those three diagrams live on the 4h-queue branch and will land on
`main` via that lane's PR; this README indexes them for the reader's
convenience but does not own them.

## Cross-references

- `docs/adr/0007-ports-adapters-hexagonal.md` — explains the
  `sharecli-core` ↔ `sharecli-fleet` ↔ `sharecli-thermal-tui`
  hexagonal split.
- `docs/adr/0009-global-nested-ledger-local-admission.md` —
  explains why `sharecli-session` keeps its own ledger and does
  not depend on `sharecli-fleet`.
- `docs/adr/0008-airlock-convergence-contract-capture.md` —
  explains the contract-capture surface between the CLI binary
  and the sharecli-fleet fleet-aggregator.
- `FUNCTIONAL_REQUIREMENTS.md` — pins the FR-001..FR-011 IDs
  that map to the runtime behaviour shown here.
- `Cargo.toml` lines 244-260 — cargo-dist release matrix that
  pins which tray binary ships on which platform (covered in
  detail by 06-tray-backend-platform-matrix).