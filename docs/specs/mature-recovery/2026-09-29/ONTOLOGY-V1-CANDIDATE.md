# ShareCLI ontology v1.0 candidate — workload coordination contract

Status: **V1.1 CANDIDATE / adversarial-review refinements incorporated; experiments still gate freeze**  
Date: 2026-09-30.  
Frozen implementation source: `4f01d0199e82b62bcf20399afcc102f58a10ad07`.

This supersedes ontology v0.1 as the working mature-product vocabulary. It does not declare implementation completion.

## Product thesis

ShareCLI is a workload-observation and coordination layer for heterogeneous developer/agent processes. It may observe, supervise, mediate, admit, share equivalent in-flight work, integrate stronger native coordination/cache mechanisms, and optionally intercept filesystem activity. These are independent capabilities.

It is **not** a universal arbitrary-command result cache, a process classifier that gains control authority by observation, or a requirement that every workload traverse one custom execution engine.

## Identity model

`Host`
→ contains `ProcessGeneration`

`Principal`
→ owns/authorizes `Project/Workspace`
→ requests `Invocation`

`Invocation`
+ `ExecutionContext`
+ optional accepted `EquivalenceAdapter`
→ `EquivalenceDecision`

`EquivalenceDecision`
→ BYPASS
→ IN_FLIGHT_SHARE
→ DURABLE_REUSE_ELIGIBLE

Actual execution:
`ExecutionAttempt → ResultArtifact → EvidenceReceipt`

Coordination:
`AdmissionRequest → AdmissionProvider → Lease → Release/ReclaimReceipt`

Optional mediation:
`Invocation → MediationPath → mediated Invocation`

Optional filesystem capability:
`FilesystemPolicy(off|optional|required) → FilesystemSession?`

## First-class entities

### Host
Stable host/device identity plus OS/runtime capability facts.

### Principal
Human, agent or service authority. Principal identity is not PID identity.

### ProcessGeneration
A particular process lifetime. Minimum identity is platform-specific but MUST resist PID reuse. PID is an attribute, not the entity.

### OwnershipClaim
Durable authority/registration binding a Principal/Project to a ProcessGeneration or managed workload. It survives replacement of the ephemeral ShareCLI controller/worker and has its own generation/provenance.

### PolicyScope
A host/project/workspace/principal-scoped policy input. Multiple scopes may overlap; precedence/composition is resolved explicitly into a PolicyDecision rather than by configuration accident.

### PolicyDecision
Versioned resolved policy for admission/mediation/equivalence/filesystem behavior, with provenance to contributing PolicyScopes.

### Project
Product policy/authorization grouping.

### Workspace
Execution/input namespace. A workspace MAY map to a filesystem path but is not defined solely by cwd.

### Invocation
Requested executable/tool + argv + declared execution context. It is not automatically shareable.

### ExecutionContext
Relevant environment, workspace, toolchain, principal/scope, target/platform and declared external-input facts.

### EquivalenceAdapter
Versioned domain-specific authority capable of determining which Invocation/InputSnapshot pairs may share work/results and for what lifetime.

### InputSnapshot
Adapter-defined immutable/sufficient identity for inputs relevant to one equivalence decision.

### EquivalenceDecision
Explicit result:
- `BYPASS(reason)`;
- `IN_FLIGHT_SHARE(class_id)`;
- `DURABLE_REUSE_ELIGIBLE(class_id, policy)`.

Unknown/failed equivalence resolves to BYPASS.

### ExecutionAttempt
One real attempt to execute an Invocation. One Invocation may create multiple attempts through retry/speculation/racing.

### AttemptDisposition
State/reason for an attempt such as selected, superseded, cancelled, failed or unknown.

### ResultSelection
Evidence-backed choice of which successful attempt/result is authoritative for the invocation. Durable reuse may bind only to an accepted selected result.

### ResultArtifact
Result bound to execution/equivalence identity and provenance. Producer may be a ShareCLI ExecutionAttempt or an imported/native tool-cache mechanism; provenance must distinguish them. It is not a TTL entry by definition.

### AdmissionProvider
Authority controlling concurrency/admission for a workload family:
- NativeJobserver;
- ProductLeaseProvider;
- OSResourceProvider;
- explicit CompositeProvider.

### Lease
Generation-safe bounded admission ownership. Lease identity is independent of integer PID and filename encoding.

### MediationPath
Actual interception/redirection route such as an authorized proxy/dispatcher. Observation does not imply mediation.

### FilesystemPolicy
`off | optional | required`.

### FilesystemSession
Verified interception/isolation session with declared coverage/scope. Optional failure produces truthful degradation; required failure prevents dependent execution. Session readiness does not imply all subprocess/filesystem activity is covered.

### EvidenceReceipt
Verifier observation bound to exact subject, candidate, configuration, adapter/provider version and environment.

## Capability vector

For each workload:

`observed, attributable, owned, supervised, mediated, admitted_by, equivalence_state, in_flight_shared, durable_reused, filesystem_mode, filesystem_ready, recovery_managed`

No state implies another unless an accepted transition says so.

## Mature invariants

1. Observation never grants control authority.
2. Process ownership binds to ProcessGeneration, not PID alone.
3. Unknown equivalence executes rather than replays.
4. In-flight sharing and durable reuse are distinct contracts.
5. Durable reuse is adapter-specific opt-in.
6. TTL/freshness cannot substitute for semantic identity.
7. Adapter/provider version participates in evidence/reuse authority.
8. Side-effect/nondeterministic eligibility is explicit.
9. Native tool coordination/caches are preferred where stronger than generic duplication.
10. Admission ownership is generation-safe and reclaimable.
11. Priority/fairness semantics are independent from storage encoding.
12. Wall clock is not ownership authority.
13. Resource observation/refusal/throttling/enforcement are distinct.
14. Mediation claims require an actual mediation path.
15. Filesystem interception is independent from equivalence.
16. Required filesystem capability fails closed.
17. Optional filesystem degradation is explicit.
18. Speculation cannot exceed durable-equivalence authority.
19. Evidence binds the exact implementation/dependency/configuration candidate.
20. UI/API status is a projection of these facts, not a substitute for them.
21. Overlapping policy scopes resolve through an explicit PolicyDecision.
22. Controller/worker replacement does not destroy durable OwnershipClaims.
23. Multiple ExecutionAttempts require explicit disposition/result selection.
24. Native/external cache results preserve producer provenance rather than masquerading as ShareCLI execution.
25. Filesystem interception claims include coverage scope.
26. Distributed/remote coordination is not part of the currently accepted local/owned-workstation product boundary; future mesh expansion requires a separate architecture/authority gate.

## Mechanism disposition

| Existing mechanism | Mature disposition |
|---|---|
| proc scan / fleet observation | retain behind ProcessGeneration/capability truth |
| helios-shield dispatcher/proxy | retain as one MediationPath, not universal truth |
| Args/Time/Git generic durable cache modes | transition/compatibility candidates; not mature universal equivalence |
| Lock-Wait-Cache | split: in-flight sharing may survive; durable replay requires adapter contract |
| SlotQueue | replace/refactor behind AdmissionProvider/ProductLease semantics where native provider unavailable |
| jobserver-named strategy | cannot claim native jobserver until actual token protocol is used |
| FUSE | optional capability adapter by default; required only for accepted dependent journeys |
| speculation tracker | subordinate to accepted durable-equivalence policy |
| thermal/resource watchers | observation/policy inputs, not automatic hard enforcement |

## Architecture freeze blockers

Final v1.0 acceptance still requires:
- current in-flight-vs-durable native experiment;
- queue PID-reuse/fairness native receipts;
- one native-jobserver vs ProductLease comparative prototype;
- required-filesystem negative prototype;
- public capability-truth fixture;
- independent adversarial review.

Until then this is the canonical **candidate** ontology and new requirements should use it rather than mechanism-shaped legacy vocabulary.
