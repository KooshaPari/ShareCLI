# sharecli architecture

This directory holds the canonical architecture diagrams for
sharecli. The diagrams are Mermaid; the source files can be
edited as plain Markdown and render natively in GitHub, Obsidian,
JetBrains IDEs, and the `mmdc` CLI from `@mermaid-js/mermaid-cli`.

| # | Diagram | File | Type | Purpose |
|---|---------|------|------|---------|
| 01 | System overview (crate topology) | [01-system-overview.md](01-system-overview.md) | `graph TD` | Which crate depends on which, and the role of each |
| 02 | Process lifecycle data flow | [02-process-lifecycle.md](02-process-lifecycle.md) | `sequenceDiagram` | How `sharecli start` flows from shell to a managed process |
| 03 | FUSE write-provenance intercept | [03-fuse-write-provenance.md](03-fuse-write-provenance.md) | `flowchart` | How the FUSE layer tags every write with `user.sharecli.session` and `user.sharecli.written_at` |
| 04 | Session state machine | [04-session-state-machine.md](04-session-state-machine.md) | `stateDiagram-v2` | The four `SessionState` variants and which crate function drives each transition |
| 05 | IPC Lock-Wait-Cache protocol | [05-ipc-cache-transaction-flow.md](05-ipc-cache-transaction-flow.md) | `sequenceDiagram` | The canonical Lock-Wait-Cache protocol used by every FR-008 command path |
| 06 | Tray backend platform matrix | [06-tray-backend-platform-matrix.md](06-tray-backend-platform-matrix.md) | `flowchart` | The asymmetric tray stack (Linux single crate, Windows two components, macOS Swift-only) |
| 07 | Clap-derive command dispatch | [07-clap-derive-command-dispatch.md](07-clap-derive-command-dispatch.md) | `flowchart` | The `Cli::parse → match &cli.command` pipeline and every nested `#[command(subcommand)]` |

Every diagram cites the source lines and crates that justify the
picture, so a diagram can be checked against the code rather than
trusted.

## Reading order

1. **01-system-overview** — start here. Read once to learn the crate
   names and what each one does. Re-visit when you need to know
   "where does this functionality live?".
2. **02-process-lifecycle** — read when working on `start`, `ps`,
   `stop`, `status`, or the ProcessPool itself. The data-flow
   diagram is the source of truth for "who calls who" during a
   process-management operation.
3. **03-fuse-write-provenance** — read when working on the FUSE
   layer, the mount/unmount flow, or the xattr-based auditing.
   The diagram pins the order of operations in the FUSE
   write callback.
4. **04-session-state-machine** — read when working on
   `sharecli-session`, the durable-session ledger, or recovery.
5. **05-ipc-cache-transaction-flow** — read when debugging "why did
   this command re-run when I expected it to share?" or when adding
   a new command-keyed coalesce path.
6. **06-tray-backend-platform-matrix** — read when porting the
   tray, debugging "tray icon missing on platform X", or auditing
   the cargo-dist release matrix.
7. **07-clap-derive-command-dispatch** — read when adding a new
   subcommand, debugging "my arg is parsed wrong", or reviewing the
   top-level dispatch table.

## Cross-references

- `docs/adr/0007-ports-adapters-hexagonal.md` — explains the
  `sharecli-core` ↔ `sharecli-fleet` ↔ `sharecli-thermal-tui`
  hexagonal split that 01-system-overview diagrams.
- `docs/adr/0009-global-nested-ledger-local-admission.md` —
  explains why `sharecli-session` keeps its own ledger and does
  not depend on `sharecli-fleet`.
- `docs/adr/0008-airlock-convergence-contract-capture.md` —
  explains the contract-capture surface between the CLI binary
  and the sharecli-fleet fleet-aggregator.
- `FUNCTIONAL_REQUIREMENTS.md` — pins the FR-001..FR-011 IDs
  that map to the runtime behaviour shown here.
- `Cargo.toml` lines 244-260 — the cargo-dist release matrix that
  pins which tray binary ships on which platform, covered by
  06-tray-backend-platform-matrix.
