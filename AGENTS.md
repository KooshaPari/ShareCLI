# AGENTS.md — phenotype-teamcomm

This file governs work inside the `phenotype-teamcomm` repository.

## Identity

`phenotype-teamcomm` is the inter-agent coordination infrastructure for the
Phenotype ecosystem — a multi-agent coordination layer (sessions, file
reservations, inbox, live state, hook events) for AI-driven development.

Do not apply parent shelf instructions
(`/Users/<REDACTED>/CodeProjects/Phenotype/repos/AGENTS.md`) unless explicitly
referenced. Work from this directory and treat paths as local to
`phenotype-teamcomm`.

## Stack

- Language: Rust (edition 2021, MSRV 1.75)
- Build system: Cargo (workspace, resolver = 2)
- Async runtime: tokio (multi-thread)
- Serialization: serde + serde_json
- CLI parsing: clap (derive)
- Observability: tracing + tracing-subscriber
- Errors: anyhow (library surfaces will refine to `thiserror` over time)

## Layout

```
phenotype-teamcomm/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── teamcomm-protocol/     # wire types
│   ├── teamcomm-client/       # embeddable client library
│   ├── teamcomm-daemon/       # long-running coordinator (lib + bin)
│   ├── teamcomm-cli/          # `teamcomm` human CLI (bin)
│   └── teamcomm-mcp/          # MCP server (bin) + mcp/manifest.json
├── schemas/                   # protocol schema sources (to come)
├── docs/                      # design notes, PROTOCOL.md, etc.
├── tests/                     # cross-crate integration tests (to come)
└── configs/                   # forge/codex/claude/copilot hook snippets
```

## Workspace Discipline

- **You (the scaffold agent) own the workspace root.** Only the root
  `Cargo.toml`, `README.md`, `AGENTS.md`, `.gitignore`, `LICENSE`, and
  top-level `*/.gitkeep` files should be touched at the root.
- **Implementation agents (5, 6, 7, 8) own their crate subdirs.** Each agent
  works exclusively inside `crates/teamcomm-<name>/` and never edits another
  agent's crate or the workspace root.
- **Do not edit the stub `src/lib.rs` / `src/main.rs` to make compilation
  work around dependency issues.** Fix the `Cargo.toml` of the affected
  crate instead. The stubs must compile as-is.

## Required Operating Loop

1. Check AgilePlus for existing specs before implementation.
2. Research code and tests before editing.
3. Keep changes scoped to a single feature or bug fix.
4. Validate with quality-gate checks (see below).
5. Do not leave incomplete work or stub implementations.

## Canonical Surfaces

- **Spec tracking:** AgilePlus at `/Users/<REDACTED>/CodeProjects/Phenotype/repos/AgilePlus`.
- **Work audit:** `docs/worklogs/README.md` (to come).
- **Build/test:** Cargo workspace — see `Key Commands` below.

## Key Commands

```bash
# Build everything in the workspace
cargo check --workspace

# Build a single crate
cargo check -p teamcomm-protocol

# Format
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Run all tests once stubs are filled in
cargo test --workspace

# Run the daemon (once implemented)
cargo run -p teamcomm-daemon

# Run the CLI (once implemented)
cargo run -p teamcomm-cli -- --help
```

## Quality Rules

### Linting & Formatting

All code MUST pass linting and formatting checks before commit:

- `cargo fmt --all` must run clean.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass.
- Fix errors; do not suppress or ignore warnings.

### Testing & Specification Traceability

- All tests MUST reference a Functional Requirement (FR) once
  `FUNCTIONAL_REQUIREMENTS.md` exists in `docs/`.
- Every FR MUST have at least 1 test.
- Run tests locally and verify pass before pushing.

### Documentation

- Use Vale for Markdown validation where available.
- Keep docs organized per global structure: `docs/guides/`,
  `docs/reports/`, `docs/research/`, `docs/reference/`,
  `docs/checklists/`.
- Never create `.md` files at root level (except `README.md`, `AGENTS.md`).

## Governance Reference

- **Global baseline:** `~/.claude/CLAUDE.md` (dependency preferences, prose
  quality, context management, failure behavior).
- **Phenotype-org scripting:** `repos/docs/governance/scripting_policy.md`
  (Rust default; no new shell).
- **Git discipline:** Phenotype Git and Delivery Workflow Protocol
  (parent `CLAUDE.md`).
- **Child agents & delegation:** See parent `CLAUDE.md` (prefer subagents
  for multi-file work).

## Worktree Pattern

- **Feature work:** Use repo worktrees at
  `repos/phenotype-teamcomm-wtrees/<topic>/`.
- **Canonical repo:** Always on `main` except during merge operations.
- **No feature branches in canonical:** All work isolated in worktrees
  until integration.

## Integration & Handoff

When feature work is complete:

1. Ensure all tests pass and quality gates are clean.
2. Create a pull request or squash-commit to `main`.
3. Update AgilePlus work package status.
4. Archive worktree or keep for reference.

---

**Parent contract:** See `AGENTS.md` at
`/Users/<REDACTED>/CodeProjects/Phenotype/repos/AGENTS.md` for cross-project
agent coordination and parent shelf governance.
