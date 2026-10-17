# Pooled execution safety repair (2026-10-17)

FR-001; issue #886. Baseline: ec7bcbd9c630a1e393f5cd5ee044f0a9d92b7548.

## Accepted intent and current boundary

PRD.md defines an OS-adjacent native runtime with optional FUSE, not a VM
isolation requirement. The existing Hypervisor::run/run_queued and harness-native
strategies execute real native commands. They are not connected to `sharecli run`.
`sharecli run <node|bun> <project>` has no argv/script input or project-command
resolution. Its previous SharedRuntime implementation discarded the script,
probed `--version`, and could report a pooled PID as successful work.

## Implemented behavior

The pooled entrypoint now returns a typed `std::io::ErrorKind::Unsupported`
before acquire, runtime probes, worker creation, or resource acquisition. The CLI
prints a precise backend-unavailable error, exits nonzero and cannot print pooled
success/output. Help labels this capability unavailable. This is a safety repair;
it does not complete requested execution, cancellation, timeouts or permit lifetime.

The ordinary `start` command detaches a managed process and reports spawn, not
its eventual child exit; it is not an output/exit propagation substitute. No host
execution fallback or new isolation requirement was introduced.

## Closure oracles

- CLI with empty PATH must reject for the missing backend, not fail spawning node.
- CLI with available sentinel node/bun executables must leave the invocation marker
  absent: even probing an installed executable is forbidden on this path.
- Runtime script and empty-script requests return Unsupported for node/bun/unknown;
  both pools stay empty.
- The existing native queued Hypervisor adapter is separately exercised with real `/bin/sh` output, stderr, exit 7, missing executable failure and subsequent slot recovery. This does not wire pooled CLI execution. No successful pooled-run witness is claimed. Define and wire command input plus
  lifecycle semantics before exposing pooled execution as available.

## Placement and preservation

Local single-job Rust using an owned target copied with APFS reflinks from an
existing cache. Initial vm_stat free pages 113793 x 16384 (~1.74 GiB), inactive
167803 (~2.56 GiB); 137 GiB disk available. No global cargo/rustc kills, shared
profiles, VM state, production services, screenshots or recordings. Canonical
checkout's dirty Cargo.lock/tray changes were preserved. PRs #876/#878/#931 untouched.

## Validation limits

`cargo check --locked` and `cargo clippy --locked --bin sharecli -- -D warnings` passed. Workspace `cargo fmt --check` is blocked by a
pre-existing duplicate `[lib]` key in cached PhenoShared agileplus-cache/Cargo.toml
at locked revision 68beca26. Direct rustfmt check of owned files passes. Full
workspace gates are not claimed. See receipt.json and retained logs for exact
commands and outcomes. No user acceptance or full product acceptance is claimed.

## State and Forward Docket update (scoped evidence, not acceptance)

Governing SFD v1 source: 05e163820f1a207f2f827a5485ce6ca04084107f.
Closest Viable Product remains the accepted broader goal. This repair only closes
false acknowledgment at the pooled entrypoint. Source view: implemented fail-closed
guard and honest help. Verified view: candidate check/clippy and scoped test receipts.
Usability view: users receive a precise rejection; pooled command execution is
unavailable. User-validation view: UNKNOWN, no operator acceptance receipt.
Remaining work: define explicit command input, connect the intended native execution
lane, prove output/exit and full child-lifetime permits, cancellation, timeout,
crash/restart and resource reclamation. These obligations are not retired by this PR.

Focused final checks: runtime unit 1/1 and integration 3/3 passed. Generic CLI startup may initialize logging (verbose/TTY) before dispatch; no harness probe, pool construction or requested work occurs.
