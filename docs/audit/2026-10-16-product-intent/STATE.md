# Current usability and user journey

Source snapshot: `e182151d4183c964547c017a4cc3f606a7cfb8ae`. Observation: 2026-10-16 Pacific.

## Operator journey

```text
install exact artifact -> discover agents -> inspect pressure/reports
                                            |
                           select safe work/optimization
                                            |
                          admit -> execute locally/remotely
                                            |
                    cancel/retry/recover -> observe true outcome
                                            |
                        compare baseline -> report real savings
```

| Journey stage                          | Source-backed capability                                                                      | Material limit                                                                                                                        | Usability assessment                                                            |
| -------------------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Install and identify                   | Install script, archives, container and host-specific tray paths documented                   | Latest listed release is v0.3.0 from July 5; no current signed/installed receipt tied to this main                                    | Historical distribution exists; current install acceptance UNKNOWN              |
| Register/configure                     | TOML configuration and project registry command paths, FR-002/003 tests                       | Current executable/profile was not exercised                                                                                          | Implemented source; runtime UNKNOWN                                             |
| Discover existing agents               | Process-tree/pattern detection and host inventory commands, FR-006 tests                      | Platform/process-access variation and false positives need native verification                                                        | Implemented source; detection alone does not intercept work                     |
| Observe load                           | CLI/status/report/serve/watch and host resource sampling, FR-007 tests                        | Unsupported platform samplers and missing pressure signals remain; no current desk receipt                                            | Partial source; some metrics may be unavailable                                 |
| Supervise real work                    | `ProcessPool::spawn` delegates to Substrate; library `Hypervisor::run` executes real commands | Public CLI `run` uses a different pooled path which ignores script execution                                                          | Separate working primitives; unsafe to describe every public path as functional |
| Bound contention                       | Queue/cache and thermal gate exist                                                            | Build permit is scoped to spawn call; unsupported platform thermal policy may report Green; cancellation/restart integration unproven | Partial; protection for entire workload lifetime not accepted                   |
| Route heavy work to stronger host      | Maildir queue and NATS registration primitives                                                | Fleet listing is an explicit empty stub; no source-to-remote-result placement receipt                                                 | Integrated automatic placement UNKNOWN                                          |
| Optimize with reports but no harnesses | Dashboard has process/pool/savings panels                                                     | No verified learned-strategies/report-only optimization journey; current savings calculation can mislabel usage as savings            | Requested journey not accepted                                                  |
| Cancel/retry/recover                   | Kill/queue/session and guard primitives, existing repair PR876                                | No same-artifact crash/cancellation/consumer-isolation acceptance                                                                     | UNKNOWN end-to-end                                                              |
| Demonstrate benefit                    | Coalesce meters and benchmark/pilot infrastructure                                            | No current matched baseline, foreground-latency or real-fleet savings receipt                                                         | Benefit UNKNOWN                                                                 |

## Blockers that affect the product's value

1. **False pooled-run success.** `src/main.rs` dispatches `Run` to `run_pool`; `src/commands/mod.rs:1169` calls `run_with_pool`; `src/runtime.rs:626` ignores `_script`, obtains a pooled PID and returns formatted text. Acquisition at `:574` starts `--version`. This is independently consistent with [issue886](https://github.com/KooshaPari/ShareCLI/issues/886). It does not negate the separate real Hypervisor/library execution path.
2. **Potentially unsafe work equivalence.** `crates/sharecli-ipc/src/cache_key.rs` Git mode hashes argv, cwd, HEAD and porcelain status, not changed input bytes. Different contents can share those values. Args mode deliberately omits cwd/env. Existing [PR878](https://github.com/KooshaPari/ShareCLI/pull/878) raises this concern; reproduce through the selected public entrypoint and bind an explicit pure-task contract before broader coalescing.
3. **Admission does not cover the child lifetime.** `src/runtime.rs:349` explicitly scopes the build permit to the spawn call. That cannot alone enforce a cap on simultaneous running builds. Join/cancel/crash must reclaim capacity based on observed termination, not intent.
4. **Fleet discovery is not complete scheduling.** `crates/sharecli-fleet/src/registry.rs:78` returns an empty device list. Maildir queue status/reclaim in `src/commands/mesh.rs` is real local queue functionality; it is not proof of load-aware remote placement and result return.
5. **Savings figures need a trustworthy model.** `src/dashboard.js:534` falls back from `saved_rss_bytes` to total agent RSS, then labels that memory avoided by coalescing. Its idle variables subtract idle from total and are shown as idle slots. Explicit zero values are also lost by `||` fallback. These are source-derived display/model problems, not measured false savings on a running instance.
6. **Artifact and release gates remain open.** Release workflow describes unsigned archives; the hard signing run failed. SignPath policy text and successful soft readiness do not establish cryptographic signatures or notarization.

## Runtime and historical receipts

`docs/ops/dashboard-live-verification-2026-09-01.md` records Windows health/dashboard/WebSocket behavior, an older binary and a missing WinFSP driver. It is useful historical evidence, not a current-version or all-platform certificate. Native FUSE tests are explicitly privileged/gated; ordinary construction tests do not qualify a mounted filesystem.

A local `sharecli` executable was found in the operator's Cargo installation. The bounded process sample showed concurrent build activity, not an identified `sharecli serve` instance; this is not proof no remote or differently named service exists. No current device/runtime observation was made. No build process owned by another lane was touched.

## Appropriate current use

Source review supports continuing isolated discovery/status and existing library/component development. It does not support promising automatic fleet optimization, correct results from the pooled `run` interface, safe coalescing of arbitrary commands, production native parity or quantified savings. A reviewer must bind a released binary and exact profile before making a user-facing availability claim.
