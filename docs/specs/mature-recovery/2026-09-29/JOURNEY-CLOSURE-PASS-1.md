# ShareCLI journey closure matrix — pass 1

Status: **DRAFT / implementation-reachability assessment**  
Date: 2026-09-30.

Journey closure is not inferred from requirement/test count. A journey closes only when its actual entrypoint, state transitions, side effects, observations and evidence all exist under the accepted ontology.

## SC-J01 — install → trustworthy local observation

**Outcome:** operator installs ShareCLI and obtains a truthful inventory/resource view of relevant local agent workloads.

Path:
`install → sharecli proc/status/serve → process/resource collectors → ObservedProcess/Observation → CLI/API presentation`

Current:
- CLI observation surfaces are mounted.
- serve/dashboard is mounted.
- process scanning/resource sampling exists.
- capability truth model is not yet the canonical presentation model.
- installed artifact/platform evidence is incomplete.

Classification: **NARROW FUNCTIONAL PATH / NOT CLOSED**.

Breaks:
- installation/platform evidence;
- observation freshness/error provenance;
- UI/API distinction between observed and owned/mediated;
- unsupported-platform behavior.

## SC-J02 — observe → explicitly adopt/start → safely control owned workload

**Outcome:** operator can start or explicitly adopt a workload, later stop/restart the exact same ProcessGeneration without touching an unrelated process.

Path:
`Principal/Project → PolicyDecision → OwnershipClaim → ProcessGeneration → start/adopt → status → stop/restart → evidence`

Current:
- start/stop/ps/status are mounted.
- PID-centric current implementation exists.
- v1.1 generation-safe OwnershipClaim is not implemented.
- PID-reuse oracle pending.

Classification: **PRIMITIVES EXIST / IDENTITY SAFETY BLOCKS CLOSURE**.

## SC-J03 — competing work → admission → bounded execution

**Outcome:** concurrent workloads are admitted/refused/queued according to explicit policy without leaked slots, starvation artifacts or duplicate native throttling.

Path:
`Invocation → PolicyDecision → AdmissionProvider → Lease → ExecutionAttempt → release/reclaim → evidence`

Current:
- SlotQueue/thermal/resource primitives exist.
- native-jobserver integration claim not proven.
- fairness/ownership adversaries pending.
- provider composition not implemented.

Classification: **PRODUCT HUSK / NOT CLOSED**.

## SC-J04 — duplicate/equivalent work → safe sharing

**Outcome:** eligible duplicate work shares only within an established equivalence domain; unknown work executes independently.

Path:
`Invocation → EquivalenceAdapter → InputSnapshot → EquivalenceDecision → in-flight or durable sharing → selected ResultArtifact → receipt`

Current:
- Hypervisor/coalesce/cache implementation is reachable.
- generic durable modes are natively falsified.
- adapter contract not yet implemented.
- in-flight-vs-durable experiment pending.

Classification: **CURRENT PATH IS FUNCTIONALLY UNSAFE FOR MATURE CONTRACT**.

## SC-J05 — local session interruption → evidence-based recovery

**Outcome:** after controller/terminal interruption, operator inspects durable observations and recovers only sufficiently verified local sessions/layouts.

Path:
`SessionObservation/SurfaceObservation → durable ledger → RecoveryPlan → dry run → authorized recovery → new ProcessGeneration/Surface state → evidence`

Current:
- session list/inspect/observe/watch/recovery-plan/recover/layout surfaces are mounted.
- dry-run-first semantics exist.
- exact OwnershipClaim/ProcessGeneration adoption semantics are not yet mapped.
- provider degradation and stale observation handling exist conceptually but need public-path oracle.

Classification: **STRONGEST CURRENT VERTICAL-SLICE CANDIDATE / NOT YET CLOSED**.

## SC-J06 — filesystem-dependent local work → explicit interception policy

**Outcome:** operation requiring filesystem interception either runs under verified coverage or fails closed; optional mode degrades truthfully.

Current:
- FUSE mount/status/commit/discard/list/provenance surfaces are mounted.
- core fallback is best-effort.
- required mode is not implemented.

Classification: **OPTIONAL CAPABILITY EXISTS / REQUIRED JOURNEY NOT CLOSED**.

## SC-J07 — update/uninstall without lying about durable state

**Outcome:** operator updates/rolls back/uninstalls with explicit treatment of config, evidence, cache, leases/mounts and managed processes.

Current:
- upgrade/uninstall/undo surfaces are mounted.
- v1.1 state compatibility/disposition has not been proven end-to-end.

Classification: **SURFACES EXIST / LIFECYCLE CONTRACT NOT CLOSED**.

## First vertical slice recommendation

Use **SC-J05 local session recovery** before broad cache/mesh work because it already has:
- real persistence;
- human CLI;
- machine-readable observations;
- explicit evidence grades;
- dry-run-first behavior;
- recovery execution;
- restart/recovery semantics.

Required closure oracle bundle:
1. register exact surface/session/process generation;
2. persist observation;
3. restart ShareCLI controller;
4. inspect recovery plan;
5. replace/reuse PID adversarially;
6. prove stale/reused process is not auto-adopted;
7. dry-run emits exact intended action;
8. execute authorized recovery;
9. observe new generation/surface;
10. verify evidence binds old observation + recovery operation + new subject.

This slice can prove the v1.1 identity/evidence spine without depending on distributed mesh or durable command caching.
