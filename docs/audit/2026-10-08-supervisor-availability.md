# Supervisor availability contract — audit 2026-10-08

Status: candidate on isolated audit branch; not merged or deployed.

## Observed failure and counterexample

GitHub Actions run [37744971827](https://github.com/KooshaPari/ShareCLI/actions/runs/37744971827) failed four dashboard snapshot tests because `build_status_json()` required a resident IPC supervisor even in dashboard-only use. An absent socket is not evidence of an empty managed-process registry.

A separate dashboard audit run [37758674573](https://github.com/KooshaPari/ShareCLI/actions/runs/37758674573) passed the library suite but failed a `rustfmt` check in `src/commands/serve.rs`.

## Candidate behavior

- Unix `sharecli start` and `stop` fail closed when their supervisor is unavailable; no ephemeral fallback registry.
- The dashboard continues to expose host and agent observations in degraded mode.
- The dashboard emits `supervisor_error` when managed-process data cannot be read. `status.total_processes = 0` must not be interpreted as a verified zero when that error exists.
- When available, the dashboard obtains managed process data from the resident IPC supervisor, not a fresh `ProcessPool`.
- A JSON serialization regression verifies the explicit error field.

## Remaining gates

1. Execute `cargo test --locked -p sharecli --lib`, the IPC suite, and `rustfmt --check` on this exact branch revision.
2. Test daemon-up and daemon-down WebSocket responses end-to-end, including no false zero-process state.
3. Resolve process exit/reaping, PID reuse, restart recovery, build-permit lifetime, and global environment mutation.
4. Review parity with the separate `audit/dashboard-supervisor-20261008` branch before merging; avoid duplicate divergent implementations.

No production merge or deployment is authorized by this document.
