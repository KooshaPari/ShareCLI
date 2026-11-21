# sharecli — Technical Specification

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  CLI (clap derive)                    │
│  ps | start | stop | status | config | project | run │
├──────────┬──────────┬───────────┬───────────────────┤
│ Agent    │ Project  │ Runtime   │ Monitoring        │
│ Runtime  │ Registry │ Pool      │ Health checks     │
├──────────┴──────────┴───────────┴───────────────────┤
│              Configuration (TOML)                     │
│           ~/.config/sharecli/config.toml              │
├──────────┬──────────────────────────────────────────┤
│ OS       │  Process-Compose                         │
│ Process  │  YAML generation                         │
│ APIs     │  for multi-project orchestration          │
└──────────┴──────────────────────────────────────────┘
```

## Components

| Component | Location | Responsibility |
|-----------|----------|---------------|
| CLI Entry | `src/main.rs` | Command dispatch |
| Library | `src/lib.rs` | Public API exports |
| Config | `src/config.rs` | TOML config + CLI enums |
| Runtime | `src/runtime.rs` | ProcessPool + SharedRuntime |
| Monitoring | `src/monitoring.rs` | HealthStatus, ProcessStats |
| Commands | `src/commands/mod.rs` | All CLI implementations |

## Data Models

```rust
struct ProjectConfig {
    name: String,
    path: PathBuf,
    limits: ResourceLimits,
}

struct ResourceLimits {
    max_memory_mb: u64,
    max_procs: u32,
}

struct ProcessInfo {
    pid: u32,
    project: String,
    harness: String,
    status: ProcessStatus,
    memory_mb: u64,
    started_at: DateTime,
}

enum ProcessStatus { Running, Idle, Stopped, Error }

struct HealthStatus {
    healthy: bool,
    runtime_available: bool,
    pool_usage: PoolUsage,
}
```

## CLI Commands

| Command | Flags | Purpose |
|---------|-------|---------|
| `ps` | `--project`, `--harness` | List managed processes |
| `start` | `<project> --harness <type>` | Start harness process |
| `stop` | `--project`, `--all`, `--pid` | Stop processes |
| `status` | | Health + resource summary |
| `config` | `init|validate|show|get|set` | Config management |
| `project` | `add|remove|list|discover|generate` | Project management |
| `run` | `<runtime> --project` | Pooled runtime execution |
| `pool` | | Shared runtime pool status |
| `health` | `--harness` | Probe runtime health |
| `limits` | `set|get` | Resource limits |
| `check` | `<project>` | Verify resource limits |
| `optimize` | | Analyze and suggest improvements |
| `prune` | `--idle <duration> --dry-run` | Kill idle processes |

## Configuration

```toml
[projects]
helios-cli = "~/CodeProjects/Phenotype/repos/helios-cli"
portage = "~/CodeProjects/Phenotype/repos/portage"

[runtime]
max_memory_mb = 4096
max_processes = 100

[monitoring]
health_check_interval_s = 30
idle_threshold = "30m"
```

## Process-Compose Output

```yaml
# Generated process-compose.yml
processes:
  helios-cli:
    command: "sharecli start helios-cli --harness claude"
    readiness_probe:
      http_get:
        path: /health
        port: ${PORT}
    log_location: "./logs/helios-cli.log"
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Process list | <100ms |
| Health check | <500ms |
| Project discovery | <2s for 50 repos |
| Config load | <50ms |
| Process start | <2s |
| Memory overhead | <32MB (sharecli itself) |
| Idle prune scan | <1s |

---

## Proposed transaction contract architecture extension (2026-10-12)

**Canonical components:** [ADR 0009](docs/adr/0009-global-nested-ledger-local-admission.md) and [typed invariant tests](docs/verification/global-nested-ledger.md). `sharecli-core::transaction_contract` provides a first pure, synchronous predicate layer. It neither persists the global history nor mediates filesystem/OS effects. Do not confuse `sharecli-session`'s compactable terminal observations with immutable authoritative transaction history.

**Sequence (target, not implemented):** request/Actor identity → scoped grant/epoch verification → candidate plan/equivalence certificate → atomic reservation → effect-broker dispatch → receipt/observed enforcement → output verification → target-fenced publication → per-consumer acceptance. A remote Fabric placement proposal is not a committed lease.

**Boundaries:** one owning parent scope per transaction, explicit cross-branch references, independent consumers, physical reservation IDs not doubled by parent envelopes, stable logical-effect IDs across retries, sealed required-child membership for acceptance. Post-dispatch failure can be `outcome_unknown`. Expired leases do not prove physical reclaim.

**Realization gate:** durable local SQL/outbox or equivalent transactional adapter; authenticated rights; target enforcement/reclamation; native concurrency and crash recovery tests. Preserve existing architecture, rather than fabricating claims of completed runtime integration.
