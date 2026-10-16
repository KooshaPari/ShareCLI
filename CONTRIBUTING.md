# Contributing to sharecli

First off, thank you for considering contributing to **sharecli** — it is
people like you who make this OS/kernel-adjacent agent runtime / hypervisor better
for everyone running multi-agent host infrastructure on Phenotype.

This document is a tier-0 guide. For deeper context, see:
[`AGENTS.md`](AGENTS.md) · [`SPEC.md`](SPEC.md) · [`docs/`](docs/)

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How Can I Contribute?](#how-can-i-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Improving Documentation](#improving-documentation)
  - [Pull Requests](#pull-requests)
- [Development Setup](#development-setup)
- [Local Quality Gates](#local-quality-gates)
- [Style Guide](#style-guide)
- [Commit Messages](#commit-messages)
- [Testing Policy](#testing-policy)
- [Release Process](#release-process)
- [License](#license)

---

## Code of Conduct

By participating in this project, you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md). Instances of unacceptable behavior may
be reported privately to the maintainer: **KooshaPari** (https://github.com/KooshaPari).

---

## How Can I Contribute?

### Reporting Bugs

- Use the [Bug Report](../../issues/new?template=bug_report.md) issue template.
- Provide a clear, descriptive title (`sharecli: <command> panics when …`).
- Include steps to reproduce, expected vs. actual behavior, and your environment
  (OS, Rust version: `rustc --version`, sharecli version: `sharecli --version`).
- Attach the smallest possible reproducer and any relevant logs.

### Suggesting Enhancements

- Use the [Feature Request](../../issues/new?template=feature_request.md) issue
  template.
- Search existing issues first — your idea may already be tracked.
- Describe the **motivation** and the **proposed solution**; reference any
  related `FR-####` items from [`FUNCTIONAL_REQUIREMENTS.md`](FUNCTIONAL_REQUIREMENTS.md).

### Improving Documentation

- Typos, clarifications, and examples are always welcome.
- Larger docs work belongs in [`docs/`](docs/) and should be linked from
  [`docs/index.md`](docs/index.md).

### Pull Requests

1. Fork the repository and create your branch from `main`:
   ```bash
   git checkout -b feat/<short-topic>
   ```
2. Follow the **Test-First Mandate** in `AGENTS.md`:
   - New module → test file first.
   - Bug fix → failing test first.
   - Refactor → existing tests must pass before and after.
3. Ensure the local quality gate is green:
   ```bash
   just ci-fast     # fast lane (fmt, lint, test, build)
   just gate        # full gate (adds audit, deny)
   ```
4. Make sure your branch is up to date and the PR description references any
   related issue.
5. Squash or rebase before merge; conventional commit messages preferred.

---

## Development Setup

**Prerequisites**

| Tool | Min version | Install |
|------|-------------|---------|
| Rust | stable (≥ 1.74) | `rustup install stable` |
| `just` | ≥ 1.25 | `cargo install just` |
| `cargo-deny` | latest | `cargo install --locked cargo-deny` |
| `cargo-audit` | latest | `cargo install --locked cargo-audit` |
| `typos-cli` | latest | `cargo install --locked typos-cli` |

**Quick start**

```bash
# Clone
git clone https://github.com/KooshaPari/sharecli.git
cd sharecli

# Install repo-local tools
just install-tools

# Build & run
just build
cargo run -- --help

# Run the full test suite
just test
```

The CLI configuration is stored at `~/.config/sharecli/config.toml` after you
run `sharecli config init`.

---

## Local Quality Gates

`just` is the canonical task runner. Key recipes:

| Command | Purpose |
|---------|---------|
| `just fmt` | Auto-format the workspace |
| `just fmt-check` | Verify formatting (used in CI) |
| `just lint` | `cargo clippy --all-targets --all-features -- -D warnings` |
| `just test` | `cargo test --all-features --all-targets` |
| `just test-nextest` | Faster parallel tests via `cargo-nextest` |
| `just coverage` | `cargo llvm-cov` lcov output |
| `just audit` | `cargo audit` (RustSec advisories) |
| `just deny` | `cargo deny check` (licenses, bans, advisories, sources) |
| `just typos` | Spell-check with `typos` |
| `just gate` | `lint + test + audit + deny + fmt-check` |
| `just ci` | Full local CI simulation |
| `just grade` | Show the `audit_scorecard.json` snapshot |

> **Never disable a quality gate to make CI green.** If a gate fails, fix the
> underlying issue, or — for a documented exception — add the entry to
> `deny.toml` (with a justification comment) or `_typos.toml` (extend-words).

---

## Style Guide

- **Rust**: enforced by `rustfmt.toml` and `clippy.toml`. Run `just fmt` and
  `just lint` before pushing.
- **Indentation**: 4 spaces for Rust, 2 spaces for YAML/JSON/Markdown/TOML
  (per `.editorconfig`).
- **Max line length**: 100 columns.
- **Naming**: `PascalCase` types, `snake_case` functions, `SCREAMING_SNAKE_CASE`
  constants, `snake_case` modules.
- **Errors**: never use `unwrap()` or `expect()` in production code. Propagate
  with `?`, log with `tracing`, and return concrete error types.
- **Logging**: use `tracing::{info, warn, error, debug}` — never `println!`
  in library code.

---

## Commit Messages

We follow the **Conventional Commits** specification (lightweight form):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

| Type | When to use |
|------|-------------|
| `feat` | New user-facing feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `test` | Add or correct tests |
| `chore` | Tooling, CI, dependencies, non-source change |
| `ci` | CI workflow change |
| `perf` | Performance improvement |

### Signed commits / DCO (soft)

Prefer **Developer Certificate of Origin** sign-off on commits:

```
Signed-off-by: Your Name <you@example.com>
```

Use `git commit -s` (or configure a prepare-commit-msg hook). GPG/SSH commit
signatures are encouraged for maintainers. Branch ruleset `main-signed-commits`
(id **19181236**) already requires signed commits on `main`. Soft CI may warn
when PR commits lack `Signed-off-by` or are unverified.

Maintainer Verified setup (GPG key `60BC1DAF830B0BC4` / SSH signing alternative):
[`docs/ops/gpg-verified-commits-l34.md`](docs/ops/gpg-verified-commits-l34.md).
Overview: [`docs/ops/signed-commits.md`](docs/ops/signed-commits.md).

Examples:

```
feat(pool): add SharedRuntime::health_probe() with --harness hint
fix(limits): clamp max_processes to usize::MAX instead of panicking
docs(README): clarify `sharecli project discover` recursion
chore(deps): bump clap to 4.5.20
ci(workflows): SHA-pin all third-party actions
```

---

## Testing Policy

See [`AGENTS.md`](AGENTS.md) for the project-wide mandate. In short:

- **Unit tests** live next to the code (`#[cfg(test)] mod tests`) and cover
  the happy path plus edge cases.
- **Integration tests** live in `tests/`. Each test file must annotate which
  functional requirement it covers with an `FR-####` comment near the top
  (e.g. `// FR-001: process listing`).
- **Coverage** is reported via `cargo llvm-cov` and uploaded by CI; the
  aspirational target is **≥ 85%**.
- **Do not delete failing tests** to make CI green — see
  `AGENTS.md` → Quality Gates.

---

## Release Process

Releases are driven by the `.github/workflows/release.yml` pipeline:

1. Bump `version` in `Cargo.toml` (and `VERSION` if present).
2. Update `CHANGELOG.md` (use `just changelog` to regenerate from conventional
   commits via `git-cliff`).
3. Commit with `chore(release): vX.Y.Z` on `main`.
4. Push — the `Release` workflow builds, signs, attests, and (after approval)
   publishes to crates.io.

---

## License

By contributing, you agree that your contributions will be licensed under the
**MIT** license (see [`LICENSE`](LICENSE)). The project is dual-released under
`MIT OR Apache-2.0` at the maintainer's discretion.

---

Thank you for your contributions!
