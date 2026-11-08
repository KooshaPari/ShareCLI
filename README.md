# phenotype-teamcomm

`phenotype-teamcomm` is the inter-agent coordination infrastructure for the
Phenotype ecosystem. It provides the shared substrate that lets multiple
AI-driven coding agents (and the humans steering them) cooperate on the same
repository without stomping on each other: durable sessions, file-level
reservations, an in-process inbox, a live state surface, and a hook event
stream that downstream agents can subscribe to.

[![AI slop inside](https://sladge.net/badge.svg)](https://sladge.net) [![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/<REDACTED>/phenotype-teamcomm/total)](https://github.com/KooshaPari/phenotype-teamcomm/releases)

The project is a Cargo workspace with five crates:

- `teamcomm-protocol` — wire types and serde-friendly message definitions.
- `teamcomm-client` — embeddable client library other agents use to talk to
  the daemon.
- `teamcomm-daemon` — the long-running coordinator process.
- `teamcomm-cli` — the human-facing command line (`teamcomm …`).
- `teamcomm-mcp` — Model Context Protocol server that exposes teamcomm
  primitives to MCP-aware agents (Codex, Claude, etc.).

## Core concepts

- **Sessions** — a registered agent identity with a heartbeat, a role, and
  a lifetime bounded by an explicit release.
- **File reservations** — exclusive locks over a glob/expiry pair so two
  agents never edit the same hunk concurrently.
- **Inbox** — durable, addressed messages for offline peers.
- **Live state** — a queryable view of all active sessions and reservations
  (the "who is doing what, where" snapshot).
- **Hook events** — append-only events emitted by forge/codex/claude/copilot
  hooks so the daemon can react to lifecycle signals from the host agents.

## Quickstart

```bash
# 1. Start the coordinator (background-friendly)
teamcomm daemon start

# 2. Inspect active sessions
teamcomm sessions list

# 3. Reserve files before editing
teamcomm reservations acquire "crates/teamcomm-*/src/**/*.rs" --ttl 30m
teamcomm reservations release <reservation-id>

# 4. Tail hook events from another terminal
teamcomm events tail --follow
```

Status: **pre-alpha**. The workspace, manifests, and stub surfaces are
landed; the wire protocol, daemon persistence, and CLI commands are filled
in by parallel implementation agents. See `docs/` for design notes and
`crates/teamcomm-protocol` for the eventual message schema.
