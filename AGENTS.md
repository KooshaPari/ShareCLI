# AGENTS.md — sharecli

Extends shelf-level AGENTS.md rules for sharecli. This is the **single agent
entrypoint**: read this before editing.

## Project Identity

- **Name**: sharecli
- **Language**: Rust (edition 2021)
- **Purpose**: OS/kernel-adjacent agent runtime / hypervisor for multi-agent hosts
- **Working directory**: repository root (this file's directory)

## Quick commands

| Action | Command |
|--------|---------|
| Build | `cargo build --locked --all-features` or `just build` |
| Test | `just test` (= `cargo test --locked --all-features --all-targets`) |
| Fast test | `just test-nextest` |
| Lint | `just lint` (= `cargo clippy --all-targets --all-features --locked -- -D warnings`) |
| Format check | `just fmt-check` |
| Format fix | `just fmt` |
| Docs | `cargo doc --no-deps` |
| Toolchain | pinned by `rust-toolchain.toml` (stable + rustfmt + clippy) |

Run `cargo check` (or `just lint`) before opening a PR. Prefer `just test` as
the verify loop after behavioral changes.

## Key files

| Path | Why |
|------|-----|
| `FUNCTIONAL_REQUIREMENTS.md` | Root FR-NNN index (role stories + acceptance refs) |
| `docs/specs/FR.md` | Canonical FR-001..FR-005 + AC detail |
| `docs/specs/TRACEABILITY.md` | FR ↔ source ↔ test matrix |
| `WORK_DAG.md` | Claimable S/M tasks with FR refs |
| `PLAN.md` | Human roadmap (points at WORK_DAG) |
| `TEST_COVERAGE_MATRIX.md` | FR → test coverage status |
| `docs/ops/governance/*` | Status tokens | WBS / GAP-QA |
| `docs/ops/LOCAL_LOOP_BUDGETS.md` | Soft wall-clock budgets for agent loops | T-270 |
| `llms.txt` | LLM/agent file index |
| `src/main.rs` | CLI entry |
| `src/commands/` | Subcommand implementations |
| `src/runtime.rs` | Process pool / shared runtime |
| `src/config.rs` | TOML config |
| `tests/fr001_*.rs` | FR-001 acceptance tests |
| `tests/integration_cli.rs` | Binary e2e smoke |
| `justfile` | Local recipes |
| `.github/workflows/ci.yml` | Required CI |
| `.github/workflows/pr-lint.yml` | Requires `FR-` in PR body |

## Spec & FR rules

- Cite `FR-NNN` in PR bodies (see pull request template + `pr-lint.yml`).
- Prefer claiming a task from `WORK_DAG.md` over inventing scope.
- New modules: test file before implementation (Test-First Mandate below).
- Do not renumber published FR IDs.

## Relationship with thegent-sharecli

[`thegent-sharecli`](https://github.com/KooshaPari/thegent-sharecli) was a
separate Python-based project that explored CLI share/directory functionality
for multi-agent orchestration. **It is now archived** (public, read-only).

Sharecli (this repo) is the active Rust implementation for process management.
There is no code or dependency relationship between the two repos.

### Boundary

| Aspect | sharecli (this repo) | thegent-sharecli (archived) |
|--------|----------------------|-----------------------------|
| Status | **Active** | **Archived** |
| Language | Rust | Python |
| Purpose | Process management, pooling, resource limits | CLI share / dedup / coordination |
| Architecture | ProcessPool, SharedRuntime, ResourceManager | Ports & Adapters (Hexagonal) |
| Dependency | substrate, sysinfo, tokio | Independent (no shared deps) |

## Worktrees & parallel agents

- Keep the canonical checkout on `main`; do feature work in worktrees:
  `git worktree add ../sharecli-wtrees/<lane> -b feat/sharecli-<lane>`
- One lane owns a disjoint file set; do not edit another lane's owned paths
  without coordinating (see active `WORK_DAG.md` / parent dispatch).
- Never commit agent scratch dirs: `.claude/`, `.codex/`, `.gemini/`, `.cursor/`.

### Claim-lock protocol (L30.9 / T-260)

Before editing a shared path, claim it in the PR/branch description and in
`WORK_DAG.md` / GAP-QA Owner(machine) cell. Hold the claim until merge or
explicit handoff.

| Shared path | Default owner lane | Notes |
|-------------|-------------------|-------|
| `WORK_DAG.md`, `TEST_COVERAGE_MATRIX.md`, `docs/specs/*` | agent-c03 | FR acceptance chain |
| `audit/SCORECARD-v38.md`, `audit/.lane-*` | scorecard lane | Re-score with evidence |
| `docs/ops/governance/*` | any (Status tokens only) | Flip `Status:` + Evidence; keep keys |
| `.github/workflows/release.yml`, `Containerfile`, `Formula/*` | agent-c11 | Packaging / Wave4 |
| `src/commands/serve.rs`, `src/otel.rs`, `src/pprof_http.rs` | agent-c05 | Observability |
| `src/serve_auth.rs`, `src/audit_log.rs`, `THREAT_MODEL.md` | agent-c04 / C02 | AuthN / threat |
| `benches/`, `scripts/bench/*`, `docs/eval/*` | agent-c08 | Eval / perf gate |
| `crates/spawn-core*`, fuzz | spawn lane | Do not claim from C03 FR lane |

Conflict rule: if two agents need the same path, serialize (merge first claim)
or split into sequential WORK_DAG tasks. Do not dual-edit without a named
handoff in `.agileplus/worklog.md`.

## Forbidden operations

- Do not force-push `main` / `master`.
- Do not commit secrets (`.env`, credentials, tokens).
- Do not use `rm -rf` on the repo or worktrees.
- Do not skip hooks (`--no-verify`) unless explicitly requested.
- Do not touch release-critical paths unless your lane owns them
  (`release.yml`, `Containerfile`, fuzz, benches, `spawn-core` are often
  owned by other lanes).

## Gotchas

- CI uses `RUSTFLAGS=-D warnings` — local clippy must be clean.
- Process-pool tests are often `#[cfg(unix)]` / `#[cfg(windows)]` gated.
- TRACEABILITY lists FR-003..005 test files that may still be missing; claim
  `T-210..T-230` in `WORK_DAG.md` rather than inventing alternate names.
- UTF-8 only in text files (no Windows-1252 smart quotes).
- Prefer `just` recipes over ad-hoc cargo flags so CI and local match.

## Project-Specific Rules

### Test-First Mandate

- **For NEW modules**: test file MUST exist before implementation file
- **For BUG FIXES**: failing test MUST be written before the fix
- **For REFACTORS**: existing tests must pass before AND after

### Quality Gates

All PRs must pass:

- Format check (`just fmt-check`)
- Linting (`just lint`)
- Tests (`just test`)
- PR body includes an `FR-` reference (`pr-lint.yml`)

### Commit Messages

Format: `<type>(<scope>): <description>`

Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci`

## Naming Conventions

- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`

## Error Handling

- Use language-appropriate error handling patterns
- Never use unwrap/expect in production code
- Log all errors with structured logging

## Windows + Zig spawn-core

Zig `crates/spawn-core` uses POSIX `fork` / `waitpid` / pthread and does **not**
build on Windows. `spawn-core-sys` skips Zig when
`CARGO_CFG_TARGET_OS=windows` and uses a Rust stub (working semaphore;
`zig_spawn`/`zig_waitpid` → `Unsupported`). See
`crates/spawn-core-sys/README.md` and `build.rs`.

## Known external daemons (daemon-shield)

An external auto-commit daemon — **Airlock Bot**
(`airlock@phenoforge.local`) — periodically stages and commits working-tree
deltas with the message prefix `wip: auto-commit daemon <ISO timestamp>`. The
daemon is not a git hook configured by this repo (no `core.hooksPath` set, no
local script under `.git/hooks/`); it runs out-of-band. It has repeatedly
re-introduced rebase/merge conflict markers into
`crates/sharecli-fuse/src/lib.rs` (around lines 985–1037) — see prior fix
`54277b0 fix(fuse): resolve rebase markers in lib.rs (Q1)`.

### Suppression

The repo-local hook at `.githooks/pre-commit` checks staged blobs of the
protected hot file (`crates/sharecli-fuse/src/lib.rs`) for `<<<<<<<`,
`=======`, and `>>>>>>>` conflict-marker lines and rejects the commit if any
are found. The hook is dormant by default (no `core.hooksPath` is set); enable
it per clone with:

```bash
git config --local core.hooksPath .githooks
```

This is repo-local and reversible. Do not modify or disable the daemon from
this repo — its behavior is owned by the airlock lane. To clear an
already-dirty `lib.rs`:

```bash
git checkout HEAD -- crates/sharecli-fuse/src/lib.rs
# verify clean
git grep -nE '^<<<<<<< |^=======$|^>>>>>>> ' -- crates/sharecli-fuse/src/lib.rs
```
