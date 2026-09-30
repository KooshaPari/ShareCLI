# ShareCLI mature obligations — semantic slice 05: resource policy, security, state, interfaces, lifecycle

Status: **PROVISIONAL / denominator expansion, not freeze**  
Date: 2026-09-30.

## Resource observation and policy

### SC-MO-RESRC-001 — observations identify source and freshness
CPU/memory/thermal/contention observations used for policy or UI MUST identify host, collector, collection time/freshness and unavailable/error state.

### SC-MO-RESRC-002 — observation, admission, throttling and enforcement are distinct
A reported threshold or pressure signal MUST NOT imply ShareCLI enforced a resource ceiling.

### SC-MO-RESRC-003 — policy decisions preserve contributing observations
Admission/refusal/throttle decisions MUST trace to the PolicyDecision and relevant resource observations.

### SC-MO-RESRC-004 — stale/missing telemetry cannot manufacture safety
Collector failure or stale data produces explicit degraded/unknown policy behavior according to accepted configuration.

## Security and authority

### SC-MO-AUTH-001 — mutating control requires trusted principal/project authority
Stop/restart/mediate/intercept/adopt operations bind to trusted session/domain authority, not process classification or request-supplied owner labels.

### SC-MO-AUTH-002 — capability escalation is explicit
Moving a workload from observed→owned, owned→mediated, or optional→required filesystem behavior requires an authorized transition.

### SC-MO-AUTH-003 — external/native integration credentials are scoped
Credentials/tokens/handles supplied to native jobserver, tool caches or integrations are scoped to the operation/provider and are not emitted into general evidence/logs.

## State and recovery

### SC-MO-STATE-001 — durable product state is separate from worker attempts
Project config, ownership claims, accepted policies, durable leases/results/evidence survive worker/controller replacement according to their own lifecycle.

### SC-MO-STATE-002 — schema/version migration is explicit
Persisted configuration/cache/lease/evidence formats carry version/migration semantics. Historical cache entries are never silently reinterpreted as stronger equivalence evidence.

### SC-MO-STATE-003 — recovery never upgrades uncertainty
After restart, unresolved ownership/execution/evidence remains UNKNOWN/degraded until independently reconciled.

## Human and machine interfaces

### SC-MO-API-001 — CLI/API/tray/dashboard project common product facts
Interfaces may differ in interaction design but MUST NOT maintain conflicting ownership/capability/evidence truth.

### SC-MO-API-002 — machine interfaces expose exact identities
Machine-readable output includes stable workload/process-generation, operation/attempt, adapter/provider and evidence identities where relevant.

### SC-MO-UI-001 — status labels are semantically bounded
Labels such as managed, optimized, cached, mediated, intercepted or recovered map to explicit capability facts and cannot be inferred from mere detection.

### SC-MO-UI-002 — degraded/unknown states are visible
Optional FUSE failure, telemetry failure, unsupported adapter, stale observation and uncertain recovery cannot be collapsed into healthy/green presentation.

## Distribution and lifecycle

### SC-MO-DIST-001 — installed artifact identity is verifiable
Supported releases identify exact product version/build/signature provenance appropriate to the platform.

### SC-MO-DIST-002 — update/rollback preserves compatible state or migrates explicitly
Update/rollback does not silently invalidate ownership/policy/evidence semantics.

### SC-MO-DIST-003 — uninstall state disposition is explicit
Configuration, logs/evidence, caches, leases/mounts and managed-process state have documented retain/remove behavior.

### SC-MO-PLAT-001 — support matrix is evidence-backed
A platform is supported for a journey/capability only after that exact journey/capability has appropriate platform evidence. CI compilation alone is insufficient.

## Quality overlays introduced by this slice

Applicable overlays are attached by subject rather than cloned into every requirement:
- observation freshness;
- control latency;
- admission throughput/fairness;
- recovery time;
- privilege/authorization;
- resource overhead;
- accessibility of operator surfaces;
- platform compatibility.

Numeric targets remain OPEN until benchmark/user-operability evidence justifies them.

## Exit
Requires security threat mapping, public interface mapping, persistence schema inventory, installed lifecycle fixtures and platform-specific journey evidence.
