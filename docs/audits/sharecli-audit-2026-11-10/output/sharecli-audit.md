# ShareCLI technical audit against operator intent

Audit date: 2026-10-14 Pacific. Evidence: read-only inspection of `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli`, HEAD `fec57c5ce45935203c63e5b4c487fddf642ff18c`. References below are repository-relative file and line anchors. Findings describe this local checkout, not a verified installed binary or deployed web instance.

## Executive assessment

ShareCLI has substantial process pooling, host observation, thermal admission, IPC, coalescing and command-strategy machinery. The observed dashboard and detection contract remain centered on managed processes and recognized coding-agent families. This is insufficient for the operator's desired always-useful system optimization console. A zero-agent system should still expose process inventory, resource pressure, optimization opportunities and a usable policy catalog; an unknown harness should become an evidence-backed discovery candidate rather than disappear from the recognized-agent table.

The user-reported running web instance was not reproduced or captured. Source inspection found a credible dashboard delivery defect, definite misleading savings calculations in an external JavaScript file, a managed-pool/host-agent distinction that can make the empty state misleading, and a definite Windows spawn-core functional gap. No source was modified, tests/builds executed, runtime reconfigured, or process terminated.

## Findings ordered by priority

| ID | Priority | Finding | Evidence | Impact / required acceptance |
|---|---|---|---|---|
| A01 | High | Native Windows spawn-core has no working process spawn/wait implementation and no inspected `wsl.exe` backend | `crates/spawn-core-sys/src/windows_stub.rs:1`, `:99`, `:112`; `build.rs:43` | Windows binary must implement native host process control and a separate WSL backend; semaphore compilation is not process-management parity. |
| A02 | High | External dashboard JavaScript referenced without a matching HTTP route | `src/dashboard.html:808`; `src/commands/serve.rs:247-260`, `:415` | Source router serves HTML, image assets, health, config, metrics, profiling and WS; it has no `/dashboard.js` route or static fallback. Richer panels in the external JS are therefore unavailable through this router as inspected. Verify with a text-only HTTP route test before claiming deployed impact. |
| A03 | High | Savings attribution conflates observed agent RSS with avoided memory | `src/dashboard.js:534` | `savings.saved_rss_bytes || agents.total_rss_bytes || null` reports current agent RSS as saved memory when savings are absent; zero is also discarded. Require measured avoided work or explicitly labeled forecast with baseline, interval, confidence and costs. This defect is in the externally referenced JS; A02 may prevent its execution. |
| A04 | High | General-process policy path is missing in the inspected native strategy dispatcher | `crates/harness-native/src/dispatcher.rs:315-345` | Agent predicate runs before strategies; human/non-agent path directly executes real command. Existing strategies cannot establish all-process optimization from this path. Add independently scoped opt-in process classes and observation-only default. |
| A05 | Medium | Empty managed pool directs user to start a harness, even though host observation is separate | `src/dashboard.html:464-478`; `src/commands/serve.rs:673-714` | `processes` comes from `ProcessPool.list`, while `agents` comes from `AgentProcSnapshot`. An empty managed list does not establish no harnesses on host. Show separate host observed, managed, recognized, candidate and observation-health counts. |
| A06 | Medium | Detector is extensible through definitions but does not discover previously undefined families | `crates/sharecli-fleet/src/detect.rs:24-60`, `:67-89`; `proc_scan.rs:89-100` | TOML adds patterns without recompilation, but `scan_agents` only emits `match_known_agent` matches. No unknown candidate output exists in that scan contract. Add probabilistic evidence and operator review before classification/activation. |
| A07 | Medium | Pool efficiency display computes busy slots and labels them idle | `src/dashboard.js:525-530`, `:542-549` | `total - idle` is summed as totalIdle, then used for idle/total and idle-slot labels. Correct the numerator and define usefulness independently from idle occupancy. Zero/absent data need explicit handling. |
| A08 | Medium | Inspected dashboard snapshot lacks typed savings and policy catalog fields | `src/commands/serve.rs:663-671` | Contract contains gate/host_watch/pool/status/agents/processes. The external JS optionally expects `data.savings`; this envelope supplies no top-level savings object. Add versioned policy/recommendation/measurement API rather than client inference. |

## Existing capability inventory and fit

- Host detection: known families include Claude, Codex, Gemini, Cursor agent, Aider, Amp, Goose, Forge, Jcode and OpenCode. Runtime `agent_patterns.toml` definitions extend these. The configured names do not establish support for every alias/fork or unknown agent.
- Process trees: `sharecli-fleet/src/proc_scan.rs:103-146` walks ancestors and labels descendants; Linux uses `/proc`, other platforms use sysinfo (`:155-170`). Observation is broader than only ShareCLI-managed pool entries.
- Console transport: embedded HTML plus WebSocket. Existing gate, host-watch, pool, status and agent summaries form useful foundations. `serve.rs:1047-1121` contains operator-envelope tests, but this audit did not run them and serialization tests do not prove browser execution.
- Strategy execution: `harness-native/src/strategies/mod.rs:1-18` declares batch, causal order, circuit breaker, coalesce, debounce, hypervisor lane, incremental, jobserver, load balance, proactive warm, process, queue, resource throttle, retry and speculative modules. `RuleOpts` (`:25`) includes TTL, concurrency, cache keys, retry, causal domain, breaker and jobserver controls. Presence is implementation evidence, not measured effectiveness.
- Policy source: `harness-native/src/dispatcher.rs:110-145` loads `rules.conf`, resolves exact/subcommand and wildcard keys, and defaults to passthrough. This is an existing policy adapter to migrate into an inspectable versioned catalog, not a learned-optimization knowledge base by itself.
- Native spawn: POSIX Zig hot core with Rust FFI. Unix build invokes Zig (`spawn-core-sys/build.rs`); Windows bypasses Zig and provides real counting-semaphore behavior alongside unsupported process operations.
- Distribution: root `Cargo.toml:13-16` lists aarch64 macOS and x86_64 Linux metadata targets, with no Windows target there. Windows tray crates and tests exist, but their existence does not prove a released Windows management binary.
- Adjacent scope: mesh, sync, session, fleet, FUSE and tray crates exist. They are integration starting points; this audit does not establish complete cross-device peripheral/I/O/storage virtualization or cloud deployment control.

## Interpretation of scheduling ambition

The inspected mechanisms are admission, pooling, launch policy, coalescing and resource observation. Unix `nice`, macOS `taskpolicy`, POSIX spawn and semaphore management are useful OS interfaces; they do not prove arbitrary syscall deduplication, a replacement kernel scheduler, hardware scheduling control, or optimization of every unrelated process. Treat each such ambition as a separate capability with platform, privilege, scope and benchmark evidence.

Safe semantic optimization requires explicit command equivalence, working directory, inputs, environment, tool version and side-effect classification. Never coalesce arbitrary mutations, interactive sessions, credential-bearing commands or non-deterministic work solely because command strings match. Learned recommendations should remain shadow proposals until negative controls establish preserved exit codes, output, effects and isolation. Existing strategy names alone do not satisfy those gates.

## Native Windows and WSL audit requirements

The operator explicitly requires a Windows-native executable that manages WSL through `wsl.exe`; a WSL-installed Unix binary is an interim bridge only. Design `spawn-core` as a backend contract supporting native Windows host and Windows-controlled WSL processes. Inventory installed/running distributions using bounded structured parsing of WSL CLI output; resolve distribution/user/path explicitly; use argument vectors and avoid shell-string interpolation. Separate Windows process ID, WSL distribution and Linux process ID, with lifecycle ownership and start identity to prevent PID reuse mistakes.

Acceptance must cover Unicode and space-containing paths, host/guest environment mapping, stdout/stderr and cancellation propagation, exact exit status, stopped/missing distribution behavior, denied elevation, distro shutdown/restart, nested children, deadlines and reconnect. Windows jobs may govern native Windows process trees; WSL guest process policy requires a guest capability contract. Do not report Windows job containment as guest kernel containment. Runtime proof must originate on a Windows host; macOS source review cannot qualify it.

## Verification and remediation sequence

1. Establish installed daemon identity, binary hash/version, served HTML/JS route status and host scan health with CLI/HTTP text. Preserve the user's no-screenshot policy.
2. Add focused route/asset and empty-state contract tests in an authorized implementation lane. Cover empty managed pool with recognized host agents; zero agents with general work; inaccessible scan; disconnected daemon; unknown candidate; genuinely idle host.
3. Correct savings math and require typed measurement provenance. Unknown savings render unavailable, true zero remains zero. Never substitute allocated/current RSS for avoided work.
4. Expose the existing rule and strategy machinery as read-only catalog before enabling new policy mutation. Separate configured policy, active policy, proposed learning and measured result.
5. Introduce general-process observation and candidate discovery, then shadow optimization. Gate action by process ownership and command semantics.
6. Implement native Windows/WSL spawn-core backends and qualify on Windows hardware. Expand release target metadata only after runtime acceptance.
7. Link mesh compute/storage/I/O capability contracts to Fino Fabric and deployment intent to Byteport; avoid claiming adjacent product goals are implemented by a ShareCLI dashboard.

## Evidence limits

No builds or tests were run: source audit was the requested output, the canonical checkout is already dirty, and builds would write into source-adjacent Zig caches. Test files are inventoried as evidence of intended verification only. No existing green result was rerun. GitHub PR, issue and Dependabot reads through `/opt/homebrew/bin/gh` failed with `error connecting to api.github.com`; current remote status remains UNKNOWN. No ChatGPT chat export, Cursor history, cloud chat, runtime daemon identity, installed binary version, performance baseline or numeric savings was verified in this technical lane. The parent report tracks broader source recovery.

Memory quick pass returned no relevant ShareCLI/Fabric/dream-loop registry matches, so no memory-derived technical claims are used. Source refs were observed in a live shared checkout and may move after this snapshot. No source commits/pushes/PRs were made, consistent with the explicitly read-only repository scope.
