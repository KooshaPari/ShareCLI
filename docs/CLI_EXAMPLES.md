# sharecli CLI examples

This page complements the inline `EXAMPLES` block rendered by
`sharecli --help`. The `--help` block is a quick copy-paste surface;
this page explains *when* to reach for each command, what the
canonical argument shape is, and which acceptance-criteria or
functional-requirement ID the command is part of.

> **Source of truth for the inline block:** the `#[command(examples = "...")]`
> attribute on `Cli` in `src/main.rs` (commit `16d05da2`).
> Keep this page in sync when the inline block changes.

## Quick-reference

| Goal                                              | Command                                                       | FR / AC               |
|---------------------------------------------------|---------------------------------------------------------------|-----------------------|
| List managed processes (human)                    | `sharecli ps`                                                 | FR-007                |
| List managed processes (JSON, full inventory)     | `sharecli ps --all --json`                                    | AC-007.43             |
| Live watch, JSON, agent parity                    | `sharecli ps --all --json --watch 2`                          | AC-007.49             |
| Operator snapshot as CSV                          | `sharecli status --csv`                                       | AC-007.82             |
| Start a harness                                   | `sharecli start myproject --harness claude`                   | FR-001                |
| Stop a single process by PID                      | `sharecli stop --pid 4242`                                    | FR-001                |
| Stop all processes for a project (force)          | `sharecli stop --project myproject --all --force --yes`       | FR-001                |
| Inspect durable agent sessions                    | `sharecli session list`                                       | —                     |
| Show one session                                  | `sharecli session show <session-id>`                          | —                     |
| Send text to a Ghostty surface                    | `sharecli surface send <surface-id> 'echo hi'`                | —                     |
| Read bounded output from a surface                | `sharecli surface read <surface-id> --max-bytes 4096`         | —                     |
| Set project resource limits                       | `sharecli limits set myproject --cpu 2.0 --memory 4096`       | FR-005                |
| Get project resource limits                       | `sharecli limits get myproject`                               | FR-005                |
| Live thermal-gate TUI                             | `sharecli thermal`                                            | FR-011                |
| Dashboard server (HTTP + WebSocket)               | `sharecli serve --bind 127.0.0.1:3939`                        | —                     |
| FUSE mount intercept layer                        | `sharecli fuse mount --src ~/work --mountpoint ~/mnt/agent`   | FR-009                |
| FUSE commit staged CoW                            | `sharecli fuse commit --agent myagent`                        | FR-009                |
| FUSE unmount                                      | `sharecli fuse unmount ~/mnt/agent`                           | FR-009                |
| Cast pane register                                | `sharecli cast register alpha 127.0.0.1:7100`                 | —                     |
| Cast pane send                                    | `sharecli cast send alpha 'hello'`                            | —                     |
| List process-compose services                     | `sharecli service list`                                       | —                     |

> Rows marked `—` are operational commands that pre-date the
> FR-001..FR-011 set (see `FUNCTIONAL_REQUIREMENTS.md`); they
> are still part of the public CLI surface and belong in the
> `--help` examples block.

## Argument-shape conventions

- **Project names** are free-form strings passed as the first positional
  argument to project-scoped commands (`start`, `stop`, `limits`,
  etc.). They are case-sensitive.
- **Harness types** for `start` and `stop` are: `claude`, `forge`, `node`, `bun`.
  The default for `start` is `claude` if `--harness` is omitted.
- **`--all`** expands the visible set beyond processes that sharecli
  itself started. It is required for the JSON and CSV outputs that
  feed dashboards (AC-007.43, AC-007.83).
- **`--watch <seconds>`** re-renders every N seconds until Ctrl-C.
  Combine with `--all --json` for the canonical live-inventory feed.
- **`--yes`** suppresses the interactive force-kill confirmation
  prompt on `stop --all --force`. Required for unattended
  operator scripts.
- **`<surface-id>`** is the durable surface identifier from
  `sharecli session list` (or from a captured `LayoutSnapshot`).
  Surface IDs are stable across restarts as long as the underlying
  Ghostty instance is the same.

## Cross-references

- `docs/adr/0001-no-mobile-app.md` — why there is no `sharecli app` command
- `docs/adr/0003-cli-english-primary.md` — why all flags are `--kebab-case`
- `docs/adr/0004-no-mcp-server.md` — why sharecli is a CLI first, not an MCP server
- `docs/adr/0007-ports-adapters-hexagonal.md` — how subcommands map to ports
- `docs/faq.md` — top troubleshooting answers
- `docs/a11y/README.md` — `--theme` and `NO_COLOR` interaction notes
