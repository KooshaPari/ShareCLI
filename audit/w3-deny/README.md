# W3 cargo-deny desk receipts

Scope: `deny.toml` only; base `7522488`, branch `stubs/w3-deny-20261016`.
All commands ran on the Windows desk in the dedicated `w3-deny` worktree.
Compute placement: local CPU; no heavy remote work required.

Hosted CI minutes are exhausted until November 1; desk receipts are the verification.
No hosted green result is claimed.

## Commands and results

| Command                                    | Exit | Evidence                                                                 |
| ------------------------------------------ | ---- | ------------------------------------------------------------------------ |
| `cargo deny --version`                     | 0    | cargo-deny 0.20.2 already installed; no installation required            |
| `cargo deny check` (before)                | 9    | `before.log`, `before.exit`                                              |
| `cargo deny --offline check` (final after) | 0    | `after.log`, `after.exit`                                                |
| `cargo fmt --all -- --check` (baseline)    | 1    | `fmt.log`, `fmt.exit`; existing formatting differences outside this lane |

Full, unabridged before/after output is retained alongside this file.

Before: four `source-not-allowed` errors for the PhenoInfra packages and
`RUSTSEC-2024-0449` for unsafe-libyaml; stale `RUSTSEC-2026-0049` ignore warning.
Summary: `advisories FAILED, bans ok, licenses ok, sources FAILED`.

After: `advisories ok, bans ok, licenses ok, sources ok`.
Existing duplicate/license/advisory-not-detected warnings remain nonfatal.

## Policy decisions and limits

- PhenoShared was already allowed on main; its exact URL remains, now with a
  provenance comment. PhenoInfra is the actual missing source and is deliberately
  allowed using its manifest URL; the manifest pins revision `dd040ed1`.
- Remove the stale RUSTSEC-2026-0049 exception. Keep other existing exceptions
  with their reasons and an explicit 2026-11-15 review/expiry comment each.
- Add the unsafe-libyaml exception aligned with issue #905 and the OSV deadline
  of 2026-11-15. Replacing serde_yaml remains necessary; this is no remediation.
- Comment deadlines are manual review deadlines, not enforced cargo-deny expiry.
- Existing overlapping PRs #896 and #899 were inspected; this bounded W3 PR
  does not carry their lockfile or Rust formatting changes.
- The Atlas dossier was unavailable at the prescribed local path.
- No Rust implementation, lockfile, workflow, or other checkout was changed.
- FR-002: dependency-policy configuration only; no functional behavior change.

Additional verification: `cargo check --locked` exited 0 (see `cargo-check.log`
and `cargo-check.exit`); existing warnings remain. Clippy and behavioral tests
were not run for this policy-only change; no full lint/test gate is claimed.

The first online after-check completed successfully (exit 0). A later online
recheck and a `CARGO_NET_OFFLINE=true` retry were interrupted (exit -1) after
Cargo waited on the shared package-cache lock. Their partial logs are preserved
as `interrupted-recheck.log` and `env-offline-interrupted.log`.
`after.log` is the final `cargo deny --offline check` receipt using the already
fetched dependencies. The first successful online log was overwritten by the
recheck; its result was observed directly, but its full output is not retained.

The optional `cargo deny --log-level warn --manifest-path ./Cargo.toml
--all-features check` was interrupted after over six minutes in Cargo resolution
(exit -1). No all-features success is claimed. Only verified W3 child processes
were stopped; no other lane's process was stopped.
