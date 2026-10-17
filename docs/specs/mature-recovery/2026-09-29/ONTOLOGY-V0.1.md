# ShareCLI ontology v0.1 — work observation, mediation, and equivalence

Status: **DRAFT / evidence-backed slice, not full ontology freeze**.  
Source snapshot: `4f01d0199e82b62bcf20399afcc102f58a10ad07`.  
Date: 2026-09-29.

## Purpose

This slice resolves a recurring ambiguity: ShareCLI currently contains observation, supervision, proxy mediation, queueing, result reuse, filesystem interception and session/mesh mechanisms. They are not one capability and must not share one truth flag.

## First-class identities

| Entity | Meaning | Must not be conflated with |
|---|---|---|
| `Host` | OS/runtime host where observation/control occurs | product user, project |
| `Principal` | human/agent/service authority under which an action occurs | process PID |
| `Project` | configured product grouping/policy scope | cwd alone |
| `ObservedProcess` | process identified by host process evidence | owned/supervised process |
| `OwnedProcess` | process ShareCLI is authorized to control and whose generation is known | arbitrary observed PID |
| `Invocation` | requested executable + arguments + execution context | reusable work equivalence class |
| `InputSnapshot` | adapter-defined identity of inputs relevant to one invocation | Git HEAD/status shorthand |
| `EquivalenceClass` | explicit claim that two invocations may share work/result under a named adapter/policy | identical argv |
| `ExecutionAttempt` | one actual execution of an invocation | cached result |
| `ResultArtifact` | stdout/stderr/exit/result metadata tied to one execution/equivalence identity | freshness TTL |
| `Lease` | bounded ownership claim for coordination/queue/work | PID-only ownership |
| `MediationPath` | mechanism by which ShareCLI can intercept/redirect an invocation | observation capability |
| `EvidenceReceipt` | exact verifier observation of a subject/candidate/configuration | log text or prior scorecard |

## Capability-state vector

For each workload/tool/process, record independent booleans or richer states:

`observed → attributable → owned → supervised → mediated → filesystem_intercepted → optimization_eligible`

No implication is automatic except where an accepted adapter proves it.

Examples:
- process scan can yield `observed=true`, `mediated=false`;
- `helios-shield` proxy can yield `mediated=true` for a known executable;
- FUSE mount can yield filesystem interception without proving command-result equivalence;
- an owned process can be supervised without being cacheable.

## Work-equivalence contract

Generic rule:

> ShareCLI MAY share in-flight work or replay a durable result only when an accepted equivalence adapter establishes the relevant execution subject and inputs for the requested sharing lifetime. If equivalence cannot be established, the safe behavior is bypass.

An equivalence adapter may bind:
- executable/tool identity and version;
- normalized arguments;
- working directory or logical workspace;
- declared/observed input content identity;
- environment/configuration dimensions;
- dependency/toolchain identity;
- principal/authorization scope;
- declared external inputs;
- side-effect/nondeterminism policy;
- sharing lifetime: in-flight only vs durable replay.

No single universal adapter is presumed.

## Required invariants

1. **Observation is not authority.** Detecting a process cannot authorize kill/stop/interception.
2. **PID is not durable identity.** Owned control requires generation/start or equivalent anti-reuse evidence.
3. **Invocation equality is not result equivalence.** Same argv/cwd does not prove same inputs.
4. **TTL is freshness policy, not correctness.**
5. **Mediation must be truthful.** UI/API cannot claim optimization or interception for paths not actually mediated.
6. **Unknown equivalence fails open to execution, not to cached success.**
7. **Tool-native caches may dominate custom reuse.** ShareCLI should compose rather than double-cache where a stronger existing semantic domain exists.
8. **In-flight suppression and durable replay are distinct.** Their accepted risk envelopes may differ.
9. **Evidence binds exact candidate/configuration.** Historical scorecards do not qualify current behavior.

## Current implementation mapping — first slice

| Ontology relation | Current surface | Current confidence |
|---|---|---|
| observe process | fleet/proc scan surfaces | implemented surface exists; full platform truth mapping open |
| mediate executable | `harness-native/src/dispatcher.rs` via proxy/symlink | source-confirmed on Unix dispatcher path |
| choose strategy | `rules.conf` → dispatcher/strategy routing | source-confirmed; authority/domain of rules still open |
| derive cache identity | `sharecli-ipc/cache_key.rs` | source-confirmed; semantic adequacy challenged |
| lookup cached result | `sharecli-core::Hypervisor::run` | source-confirmed live caller |
| queue/admission ownership | `sharecli-ipc/queue.rs` and core routes | source-confirmed; fencing/fairness semantics open |
| filesystem intercept | sharecli-fuse + optional core guard | surface exists; actual required/optional reach matrix open |
| operator evidence | status/tray/serve/dashboard | exists; claims vs actual mediation state not fully traced |

## Immediate oracle consequences

- SC-F01 targeted native oracle must execute without unrelated tray compilation.
- Add negative controls for executable/version/env/principal changes.
- Add a capability-truth test: observed-only workload must never report itself as mediated/optimized.
- Queue experiments must use lease/generation semantics, not only priority order.
- Any future cache remediation is rejected if it simply hashes more generic fields without naming the supported semantic domain.

## Open decisions before v1.0 ontology freeze

- Which tool families merit first-party equivalence adapters?
- Is durable arbitrary-command replay retained at all?
- Is generic FUSE a mature mandatory capability, an optional adapter, or transition debt?
- What ownership primitive replaces PID-only reasoning across platforms?
- What exact distinction exists between project, workspace and session?
