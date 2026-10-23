# Security Policy

We take the security of **sharecli** — the OS/kernel-adjacent agent runtime /
hypervisor for Phenotype's multi-agent host infrastructure — seriously.

> **TL;DR** — please **DO NOT** open a public GitHub issue for security
> vulnerabilities. Use the private reporting channel below.

---

## Supported Versions

| Version | Supported          | Notes |
|---------|--------------------|-------|
| `main`  | :white_check_mark: | Bleeding edge; security fixes land here first |
| `0.1.x` | :white_check_mark: | Current pre-1.0 line; actively maintained |
| `< 0.1` | :x:                | Pre-alpha — no longer supported |

> Until 1.0, every minor version (`0.1.x`, `0.2.x`, …) may ship breaking
> changes. We commit to backporting security fixes to the **most recent
> minor** for at least 90 days.

---

## Reporting a Vulnerability

Please report security issues **privately** via one of:

1. **GitHub private vulnerability reporting**:
   <https://github.com/KooshaPari/sharecli/security/advisories/new>
2. **Email**: open a Security Advisory draft and invite
   `@KooshaPari` — GitHub will then route the conversation through their
   private advisory channel.

**Do not** open a public issue, discuss the bug on social media, or post a
proof-of-concept to a public gist before coordinated disclosure.

### What to include

To help us triage quickly, please provide:

- A clear, technical description of the vulnerability and its impact.
- A reproducible proof-of-concept (command, config, payload).
- Affected version(s) and commit SHA(s).
- Environment (OS, Rust toolchain version, sharecli version: `sharecli --version`).
- Suggested fix or mitigation, if any.
- Whether the bug is being actively exploited or disclosed elsewhere.

### What to expect

- **Acknowledgement** within **48 hours** of the report.
- **Triage decision** (accepted / declined / needs more info) within **5
  business days**.
- **Patch timeline** disclosed once root-cause is confirmed. Critical
  vulnerabilities target a fix within **7 days**; high severity within
  **30 days**.
- **Credit** in the release notes / security advisory unless you prefer to
  remain anonymous.

---

## Security Best Practices for Users

- **Pin to a specific version** in production (`sharecli = "=0.1.7"` in
  `Cargo.toml` or a specific binary tag from the GitHub Releases page).
- **Run with least privilege**: sharecli spawns child processes — use a
  dedicated low-privilege user where possible.
- **Audit config files**: `sharecli config validate` parses and validates
  `~/.config/sharecli/config.toml`. Review it before `sharecli project
  discover` walks a directory tree.
- **Lock down process-compose**: when generating `process-compose.yml` from
  registered projects, review the file before deploying it — auto-generated
  Compose files inherit permissions from the generating user.

---

## Dependency Scanning

Sharecli is scanned for known vulnerabilities on every push, every PR, and
on a weekly schedule:

- [`cargo audit`](https://github.com/rustsec/rustsec) — RustSec advisory DB
  → `.github/workflows/audit.yml` (SARIF uploaded to the Security tab).
- [`OSV-Scanner`](https://google.github.io/osv-scanner/) — OSV/GHSA lockfile
  feed → `.github/workflows/osv.yml` (weekly + PR; SARIF upload).
- [`cargo deny`](https://github.com/EmbarkStudios/cargo-deny) — license,
  ban, advisory, and source policy → `.github/workflows/deny.yml`.
- **OSSF Scorecard** — supply-chain health score → `.github/workflows/scorecard.yml`.
  The Binary-Artifacts check flags committed executables and object files.
  Zig build caches (`.zig-cache/`, `zig-out/`) are **local build products**,
  gitignored, and must not be committed — see `scripts/check-no-build-artifacts.sh`.
- **GitHub Dependabot** — automatic PRs for outdated / vulnerable
  dependencies; cargo patch/minor + github-actions grouped
  (see `.github/dependabot.yml`).
- **CodeQL** — static analysis → `.github/workflows/sast.yml`.

Container runtime hardening notes: [`docs/ops/container-hardening.md`](docs/ops/container-hardening.md).

The full policy lives in `deny.toml` at the repo root. Allowed licenses,
banned crates, allowed git sources, and per-advisory ignore-list with
justifications are all defined there.

---

## Maintainer authentication (2FA)

Maintainers and admins must enable **GitHub two-factor authentication** (hardware key preferred). Soft policy: [`docs/ops/maintainer-2fa.md`](docs/ops/maintainer-2fa.md). Org-wide enforcement is a GitHub Organization settings follow-up.

---

## Threat Model (Summary)

Full STRIDE + attack-surface inventory: **[`THREAT_MODEL.md`](THREAT_MODEL.md)**  
(reviewed yearly; last reviewed 2026-07-13 — W5.3 post-federation).

| In-scope | Out-of-scope |
|----------|--------------|
| Configuration injection via `~/.config/sharecli/config.toml` | Attacks requiring local code execution |
| Project discovery path traversal (`sharecli project discover`) | Network-level DoS against the user's host |
| Process-pool resource exhaustion (memory, fds) | Side-channel attacks between sibling processes |
| process-compose YAML generation correctness | Security of the `substrate` SDK internals (tracked upstream) |
| Path / argument injection into spawned commands | Kernel-level sandbox escapes |
| Tmpfile / state-file races (`sharecli status` cache, etc.) | — |
| `sharecli serve` HTTP/WS (Bearer + JWT AuthN, metrics, pprof, audit) | OAuth code-flow / SAML / multi-tenant AuthZ |

---

## Acknowledgments

Thank you to everyone who reports vulnerabilities responsibly. The project
would not be trustworthy without you.

---

For answers to general questions (non-security), please use
[GitHub Discussions](https://github.com/KooshaPari/sharecli/discussions)
or the [issue tracker](https://github.com/KooshaPari/sharecli/issues).
