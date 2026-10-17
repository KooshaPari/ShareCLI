# 08 — Assurance, failure modes and recovery

## 8.1 Assurance view (4-metric framework)

| Metric class | What it measures | sharecli today | Source |
|---|---|---|---|
| **Implementation** (was the code written and is it present?) | source files exist, build, lint, format | 95% (19/20 engineering leaves, 1 critical pending = tray-icon fix) | this SFD §06.2 |
| **Usability** (can a beneficiary do the journey?) | operator can install + run J1-J5 | 100% (5/5 journeys) | this SFD §04.2 |
| **Validation** (does the journey achieve its outcome?) | journey outcomes are measured, not just executable | 0% (0/1 recorded session) | this SFD §04.4 |
| **Assurance** (does the system fail closed, recover, and stay secure?) | hermetic, reproducible, cross-platform, signed, secure | 62.5% (5/8 assurance leaves) | this SFD §06.2, §08.2 |

## 8.2 Assurance leaves

| ID | Assurance leaf | State | Evidence |
|---|---|---|---|
| G-hermetic | Hermetic build with locked toolchain | PASS | CI 2026-10-17 07:12:04, 07:08:09 |
| G-repro | Reproducible build (L52 determinism) | PASS | CI 2026-10-17 07:05:20 |
| G-rustfmt | rustfmt compliance | PASS (post-#918) | CI 2026-10-17 07:05:31; PR #930 superseded |
| G-ci-test-main | `ci / test` on main HEAD | FAIL | CI 2026-10-17 07:08:58 — tray-icon 0.25.0 |
| G-cross-platform | cross / x86_64-unknown-linux-musl, freebsd, wasm32, macos, windows | FAIL | CI 2026-10-17 07:15:59, 07:20:26, 07:17:08, 07:18:49, 07:02:03 — multiple targets, possibly tray-icon root cause |
| G-cargo-deny | cargo deny advisories + licenses | FAIL | CI 2026-10-17 08:02:35 |
| G-codeql | CodeQL SAST | PASS | CI 2026-10-17 07:13:59 |
| G-trufflehog | TruffleHog secret scan | PASS | CI 2026-10-17 07:16:13 |

**Additional soft-fail (not in scoring denominator, but tracked):** cargo bench (gate), visual regression, cargo bench (soft), cargo bench (nightly trends), flake-tracker, soaks, cargo bench (gate). These do not block CVP/MVP gates today.

## 8.3 Failure modes the system is honest about

| Failure | How sharecli fails | How it recovers | Operator experience |
|---|---|---|---|
| Tray icon lib doesn't load (Linux) | `sharecli-tray` exits with clear error message; CLI keeps running | operator runs `sharecli status` for headless visibility | "tray unavailable; CLI still works" — clearly logged |
| FUSE mount fails (kernel version, FUSE not loaded) | `mount_smoke` exits with sysexits-style error code | operator re-attempts with `modprobe fuse` | "FUSE unavailable; sharecli still supervises" — clearly logged |
| IPC socket in use | `sharecli-ipc` returns 76 (EX_PROTOCOL) | operator `rm /tmp/sharecli.sock` and retry | clear error message |
| Config file invalid | `sharecli config validate` returns 64 (EX_USAGE) with parse error | operator edits `~/.config/sharecli/config.toml` | clear error, line number, suggestion |
| Resource cap exceeded | `sharecli status` shows it; the offending process is SIGSTOPped (not SIGKILLed) | operator adjusts cap; `sharecli resume` releases | the process is **paused**, not killed — important for debugging |
| Cross-process worktree conflict (mesh) | `sharecli-mesh` returns a "retry with backoff" error | operator retries; backoff is configurable | deterministic retry contract |
| `is_agent` cannot determine parent process | (pre-#929) silently `process::exit(1)` | — | **bad UX**; (post-#929) falls back to env + exec-name heuristic; logs warning | fixed by `878212aa` on `chore/cvp-1h-queue-2026-10-17` |
| Pooled execution unavailable | (pre-#938) silently spins | — | **bad UX**; (post-#938) fail-closed with actionable error message | fixed by PR #938 |
| Ledger admission guard | `crates/sharecli-session/src/ledger.rs` rejects inadmissible txns with explicit reason | operator sees the reason in `sharecli status` | deterministic contract |
| `cliff.toml` template `commit.link` var | git-cliff emits an error or empty string | manual workaround: edit CHANGELOG.md | documentation debt |
| Cross-platform build (musl/freebsd/wasm) | upstream tray-icon limitation; fails | operator builds from source on target platform | not v1.0-blocking |
| 14 files > 1000 LoC | `cargo fmt` still works; clippy still passes | — | not blocking; acknowledged in `BOUNDARY.md` |
| Stale `Cargo.lock` | `cargo build` fails with version conflict | `cargo update -p <crate>` | standard recovery |
| 8 `unreachable!()` sites | (pre-#937) 5 redundant unreachable arms in `src/main.rs:1280-1288`; (post-#937) 1 or-pattern with self-documenting message | (post-#937) message tells operator which surface is unhandled | never triggered in production today; will fire if a new `SessionCmd` variant is added without matching |

## 8.4 Trust, identity, and security posture

| Surface | Threat | Mitigation | Residual risk |
|---|---|---|---|
| `sharecli-ffi` C-ABI | memory unsafety, type confusion at FFI boundary | Rust types + C-ABI stable signatures; FFI panic-abort (`-C panic=abort`); cargo audit + OSV | integration errors at third-party embedder; not sharecli's |
| IPC UDS | unprivileged process reading other agents' state | `0o600` mode, SO_PEERCRED, ACL on the path | local escalation if operator has root |
| FUSE mount | host kernel panic via FUSE protocol | libfuse 1.x stable; smoke test in CI | upstream libfuse issues; not sharecli's |
| Config file | tamper with `~/.config/sharecli/config.toml` | runtime caps validated in-process (`cb37decc`) | operator must own their config; not enforced via signature |
| Tray libappindicator | desktop user grants; tray can hide notifications |  | social engineering; not sharecli's |
| `monitoring.rs` stub | dead code path | `#![allow(dead_code)]` + module-level doc | if v1.1 wires it, audit must run |
| `pyroscope_stub.rs` stub | dead code path | `#![allow(dead_code)]` | if v1.1 wires it, audit must run |

## 8.5 Compliance

| Standard | Status | Source |
|---|---|---|
| `Apache-2.0 OR MIT` dual-license | adopted (post-#934) | PR #934 adds SPDX headers to all 456 .rs files |
| DCO (signed-off-by) | soft-gate FAIL (no GPG key provisioned) | CI `DCO (soft)` and `DCO Sign-off (Soft Gate — pending GPG key provisioning)` |
| GPG/SSH signed commits | soft-gate FAIL (no GPG key provisioned) | CI `GPG Signed Commits` and `GPG/SSH signed commits (soft)` |
| Accessibility (WCAG 2.x Level A) | PASS for dashboard | CI `axe dashboard (WCAG 2.x Level A)` 2026-10-17 07:08:14; `dashboard keyboard Tab-cycle` 07:16:13 |
| Inclusive language | PASS | CI `vale inclusive-language (docs)` 2026-10-17 07:09:45 |
| Hermetic / reproducible build | PASS | CI `Hermetic Build (Hard Gate)` and `Repro Check (L52)` |
| SPDX identifiers | PASS (post-#934) | PR #934 |
| OWASP Top 10 | not yet audited | operator-side; not in agent scope |

## 8.6 Recovery / rollback contract

| Action | What it does | Reversible? | Time-to-recover |
|---|---|---|---|
| `git revert <merge-sha>` | reverts a merged PR | yes (non-FF only; FF merges need `git revert -m 1`) | ~2 min |
| `sharecli config reset` | deletes `~/.config/sharecli/config.toml` and regenerates from defaults | yes (recreate manually) | ~30 sec |
| `sharecli ps --kill-all` | SIGTERMs every managed process | yes (processes exit cleanly) | ~1 sec |
| `sharecli-fuse umount /tmp/sharecli-mnt` | unmounts the FUSE layer | yes (data already in real FS) | ~1 sec |
| `sharecli --uninstall` | removes binary; preserves `~/.config/sharecli/` and `~/.local/share/sharecli/` | yes (manual rm) | ~5 sec |
| Delete `~/.local/share/sharecli/` | wipes all durable sharecli state (session log, ledger snapshot, agent registry cache) | yes (rebuild on next start) | ~10 sec |
| Re-clone sharecli | restores source | yes | ~1 min |
| `git push --force` to a published branch | BLOCKED by pre-tool hook | n/a | not allowed |

**No destructive commands in this SFD's deliverables.** All write operations are additive to the PhenoRegistry worktree branch and the sharecli `audit-sessions/` directory.
