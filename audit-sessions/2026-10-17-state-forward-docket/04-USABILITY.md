# 04 — Current usability and beneficiary journeys

## 4.1 Identified public surfaces

| Surface | Type | Real consumer | Where it is exercised today |
|---|---|---|---|
| `sharecli ps` | CLI | human operator | CI shell completion test; `Eval corpus (soft)`; `live pool probe (soft)` |
| `sharecli start` | CLI | human operator | `c01_climb2_core.rs` integration; FR-001 AC |
| `sharecli stop` | CLI | human operator | FR-001 AC |
| `sharecli status` | CLI | human operator | live pool probe |
| `sharecli config` | CLI | human operator | PR #916, #917 coverage |
| `sharecli project` | CLI | human operator | SPEC.md |
| `sharecli run` | CLI | human operator | FR-001 AC |
| `sharecli --help` | CLI | human operator | zsh compdef test; bash/fish/pwsh emitted (not asserted) |
| `sharecli completions zsh` | CLI subcommand | ops / install | one positive test |
| `sharecli man` | CLI subcommand | ops / install | one positive test |
| `sharecli-fuse mount_smoke` | FUSE smoke binary | dev / CI | `c01_fuse_mount_lifecycle.rs` |
| `sharecli-tray` (linux) | tray daemon | operator desktop | (no live consumer today; library API tested) |
| `sharecli-tray-windows` | tray daemon | operator desktop | (no live consumer today; library API tested) |
| `sharecli-ipc` (binary) | IPC daemon | local agents | `c01_climb2_ipc.rs` |
| `sharecli-session` (binary) | session manager | operator | integration test |
| `sharecli-ffi` C-ABI | 6 `pub extern "C" fn` | third-party embedders | PR #925 coverage; FFI-AC not in CVP-1.0 |
| `cargo doc --open` | doc surface | developer | rustdoc 64% on hot crates |
| `github.com/KooshaPari/ShareCLI` (README, FR.md, SPEC.md) | docs surface | operator, contributor | rendered on every PR |
| Tracera sharecli product card | web | org-visible | not yet created (this SFD) |
| AgilePlus sharecli backlog | web | operator | not yet created (this SFD) |

## 4.2 Demonstrated journeys (CVP)

| ID | Beneficiary / entry | Steps | Durable output | Observed result | Limits |
|---|---|---|---|---|---|
| **J1** | **Operator cold-start**: fresh install on macOS, Linux, Windows | 1. `cargo install sharecli --path .` (or `brew install sharecli` when available); 2. `sharecli init` writes `~/.config/sharecli/config.toml`; 3. `sharecli ps` lists no processes; 4. `sharecli start myproject --harness claude` launches a managed agent; 5. `sharecli ps` shows it; 6. `sharecli stop myproject`; 7. `sharecli ps` empty. | `~/.config/sharecli/config.toml` survives restart | Manual: PASS on dev host. CI: `c01_climb2_core.rs` and `c01_climb2_fleet.rs` cover the cycle. `live pool probe (soft)` last green 2026-10-17 07:11:32 UTC. | not exercised on Windows in this session; ksni backend not in this PR set |
| **J2** | **Resource-cap enforcement**: operator sets CPU/MEM cap, agent attempts to exceed | 1. `sharecli config set runtime.caps.memory_mb 256`; 2. `sharecli start bigagent --harness codex`; 3. observe `sharecli status` shows cap applied; 4. agent process is SIGSTOPped/SIGKILLed at cap. | config persisted across restarts | CI: `cb37decc` test, `Quality Gate` Unit Tests, `idle RSS hard budget` workflow | "Verified cap enforcement" not measured at the agent-process level; only config persistence tested |
| **J3** | **Coalesce under contention**: two agents submit the same tool call | 1. two `sharecli start` instances; 2. both invoke the same logical command via IPC; 3. sharecli's `CoalesceCache` returns the cached result to the second; 4. one tool execution, two responses. | `crates/sharecli-ipc/src/cache_key.rs` log | CI: `c01_climb2_ipc.rs`. `SpeculativeCoalescer` (FR-008) tested in PR #927. | not run end-to-end with two live agents in this session |
| **J4** | **Operator visibility (fleet)**: human sees all detected agents + cap usage | 1. `sharecli status --format table` lists every agent with PID, project, harness, CPU%, MEM MB; 2. `sharecli status --format json` emits machine-readable. | durable state in sharecli session store | CI: `teamcomm-cli/src/output.rs` tests (PR #924 / #925). | not run with 3+ live heterogeneous agents in this session |
| **J5** | **Reproducible install**: operator wipes host, re-installs, gets the same result | 1. `git clone https://github.com/KooshaPari/ShareCLI`; 2. `cd ShareCLI; cargo build --release`; 3. `./target/release/sharecli --version` reports `sharecli 0.8.0`; 4. `./target/release/sharecli completions zsh > _sharecli`. | installed binary at known SHA | `Hermetic Build (Hard Gate)` PASS 2026-10-17 07:12:04 UTC and 07:08:09 UTC. `Repro Check (L52)` PASS 07:05:20 UTC. | requires Rust toolchain on host; no prebuilt binary artifact published yet |

## 4.3 Journeys deliberately excluded from CVP

| Excluded | Reason |
|---|---|
| Mobile companion | ADR-0001: not in product |
| MCP server mode | ADR-0004: not in product |
| Multi-host fleet | sharecli is single-host; cross-host is a separate offering (if any) |
| FFI embedder integration | FFI is in CVP-1.0 code; full embedder use is deferred to v1.1 per FR-010 |

## 4.4 Journey verification status

| Status | Count | Journeys |
|---|---|---|
| Verified end-to-end on dev host | 1 | J1 (manual cold-start) |
| Verified in CI on main | 4 | J1, J2, J3, J4 (each covered by ≥1 CI test) |
| Reproducible install | 1 | J5 (Hermetic Build + Repro Check) |
| Real beneficiary session (recorded) | 0 | none — gap to MVP |
| Failed during this session | 0 | (no journey broke during SFD work) |
| Blocked | 0 | (tray-icon bug blocks Linux CI on main; does not break the J1 cycle on macOS dev) |

## 4.5 Beneficiary cohort (operator personas)

| Persona | Count | Current pain | sharecli helps with |
|---|---|---|---|
| Solo developer running 2-3 coding agents | most common today | manual `kill`, `pgrep`, `htop` | `sharecli ps/start/stop` + caps |
| Team of 5 devs sharing 1 host | common | chaos in `~/.claude/`, `~/.codex/`, race conditions | per-agent cap, coalesce, FUSE intercept |
| Bench orchestrator | niche | unrelated, lives in `phenotype-tooling` | not in scope |
| Operator running 50+ agents | edge | needs fleet registry, mesh | `sharecli-mesh` provides; per-agent UX may be inadequate |
