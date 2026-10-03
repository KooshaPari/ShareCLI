# ShareCLI capability truth matrix — draft 0.1

Status: DRAFT. Source snapshot `4f01d0199e82b62bcf20399afcc102f58a10ad07`.

## Truth dimensions

Each workload/tool/platform instance is assessed independently on:

| Dimension | Meaning | Evidence needed |
|---|---|---|
| Observed | ShareCLI can identify the process/workload and report it | live process/sample tied to exact PID+generation |
| Attributed | ShareCLI can classify it to agent/tool/project with stated confidence/authority | classifier/rule provenance |
| Owned | ShareCLI is authorized to mutate/control it | spawn/registration/ownership evidence |
| Supervised | ShareCLI has an active lifecycle/control relationship | start/stop/restart public-path receipt |
| Mediated | invocation passes through a ShareCLI interception/dispatcher path | actual path evidence, not detection |
| FS-intercepted | filesystem accesses pass through declared FUSE/adapter path | mount/path observation and behavior |
| Optimization-eligible | an accepted adapter/policy permits coalescing/cache/queue optimization | adapter identity + exact configuration |
| Result-shareable | exact invocation/input identity qualifies for in-flight or durable sharing | equivalence receipt |
| Recovery-managed | durable state permits restart/reconnect/reclaim semantics | restart/crash receipt |

## Current source-backed baseline

| Workload/path | Observed | Owned | Supervised | Mediated | FS intercepted | Optimization eligible | Notes |
|---|---|---|---|---|---|---|---|
| arbitrary host agent found by proc scan | yes candidate | no by default | no by default | no | no | no | observation must not imply control |
| ShareCLI-spawned managed process | yes | yes candidate | yes candidate | not necessarily | no by default | no by default | lifecycle mapping still needs public-path proof |
| command launched through `helios-shield` proxy and classified human | possible | n/a | n/a | yes dispatcher path, then exec real | no | no | human path bypasses strategy |
| command launched through `helios-shield` proxy and classified agent, passthrough rule | possible | n/a | n/a | yes | no | no | mediation exists without optimization |
| command launched through proxy + coalesce/cache rule | possible | n/a | n/a | yes | optional | **candidate only** | eligibility must be constrained by equivalence adapter; current generic modes contested |
| FUSE-mounted path | process-independent | n/a | n/a | command may be unmediated | yes for mounted path | separate | FS interception does not prove command equivalence |
| tray/dashboard observation | presentation only | n/a | n/a | no | no | no | UI must report underlying state truthfully |

## Product-facing invariant

Every UI/API/CLI representation of a workload SHOULD eventually expose enough capability state to avoid the false statement:

> “ShareCLI sees this agent, therefore ShareCLI is optimizing/controlling it.”

The mature model should represent a capability vector rather than one binary “managed” flag.

## Verification backlog

1. Spawn one process through ShareCLI and one externally; verify ownership/control separation.
2. Detect a known agent without proxy interception; prove `mediated=false`.
3. Route a known executable through `helios-shield` with passthrough; prove `mediated=true`, `optimization_eligible=false`.
4. Route a bounded tool adapter through coalescing; require adapter/equivalence evidence before `result_shareable=true`.
5. Mount FUSE for a workspace while executing a command outside the dispatcher; prove FS interception and command mediation remain independent.
6. Restart supervisor/sidecar and verify which capability claims persist, expire, or become UNKNOWN.

## Architecture consequence

The core product state should contain capability facts with provenance, not infer them on presentation. This enables a monitor-only ShareCLI configuration, a supervisor-only configuration, and opt-in mediated adapters without misrepresenting unsupported control.
