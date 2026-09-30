# ShareCLI mature obligations — semantic slice 01

Status: **PROVISIONAL SEMANTIC SLICE / NOT FULL CONTRACT**  
Date: 2026-09-30.  
Source snapshot: `4f01d0199e82b62bcf20399afcc102f58a10ad07`.

This slice covers the mature spine now supported by source, conversation authority, native evidence and current SOTA. It does **not** replace existing FR IDs yet. IDs below are recovery-contract candidates.

## SC-MO-OBS-001 — truthful observation identity

**Statement.** A reported workload/process MUST be bound to enough host/process identity to distinguish the observed execution from a later PID reuse.

**Authority/rationale.** Direct user intent requires central observability/control; current process scanning is a mature capability. Observation cannot safely become control authority when PID identity is ambiguous.

**Positive acceptance.**
- live process appears with PID plus start/generation or equivalent anti-reuse identity;
- refresh observes the same generation as the same execution.

**Counterexample.**
- process exits, PID reused, ShareCLI carries prior ownership/classification into the new process.

**Journeys.** observe heterogeneous agents; owned lifecycle.
**Verification.** spawn/exit/reuse fixture with OS-ground-truth comparison.

## SC-MO-OWN-001 — observation does not authorize control

**Statement.** ShareCLI MUST NOT stop, mutate, intercept, or claim ownership of a process solely because it was detected/classified as an agent.

**Authority/rationale.** User intent distinguishes central observation from bounded runtime control; capability-matrix source inspection proves detection and mediation are different paths.

**Positive acceptance.** externally launched detected process is visible but control attempt is refused absent accepted ownership.
**Counterexample.** detected foreign PID is killed because family classifier matched.
**Criticality.** veto-class correctness/security criterion.

## SC-MO-CAP-001 — capability truth is explicit

**Statement.** Operator/machine surfaces MUST represent observation, ownership, supervision, mediation, filesystem interception, optimization eligibility, result sharing and recovery management as independent capability facts or an equivalent non-misleading model.

**Counterexample.** dashboard says a process is “optimized” because it is merely detected.

## SC-MO-MED-001 — mediation is evidenced by actual path

**Statement.** A workload MAY be marked mediated only when its invocation actually traversed an accepted ShareCLI mediation path such as an authorized dispatcher/adapter.

**Current trace.** `helios-shield` proxy/symlink dispatcher is one source-confirmed mediation path.

**Counterexample.** process scanner discovers cargo, so UI claims cargo commands are routed through Hypervisor.

## SC-MO-EQ-001 — unknown semantic domain bypasses sharing

**Statement.** If no accepted equivalence adapter can establish the relevant execution identity, ShareCLI MUST execute normally rather than return a shared/durable result.

**Authority.** Direct user preference favors bounded/tool-specific optimization and normal shell fallback; SC-F01 natively falsifies generic Git-mode durable replay.

**Positive acceptance.** unknown command returns `BYPASS`/equivalent and executes.
**Counterexample.** fallback chooses Time/Args/Git heuristic durable cache because no adapter matched.

## SC-MO-EQ-002 — adapter identity/version participates in reuse authority

**Statement.** Every accepted sharing decision MUST identify the equivalence adapter and version/policy that established eligibility.

**Counterexample.** cache entry created under adapter v1 is silently trusted after v2 changes relevant-input semantics.

## SC-MO-EQ-003 — relevant input identity is adapter-declared and falsifiable

**Statement.** An equivalence adapter MUST define which executable/tool, arguments, workspace/input content, environment/configuration, dependency/toolchain, principal/scope and external-input dimensions are relevant or explicitly irrelevant.

**Positive acceptance.** negative fixture changes each declared-relevant dimension and prevents incorrect sharing.
**Counterexample.** changed file bytes or environment value returns stale output.

## SC-MO-INFLIGHT-001 — in-flight duplicate suppression has bounded lifetime

**Statement.** ShareCLI MAY allow one execution to satisfy concurrent equivalent requests, but that equivalence authority expires when the owning execution completes/fails/abandons unless a separate durable-reuse contract applies.

**Authority.** User duplicate-work problem; SC-AD-01 candidate; native split experiment pending.

**Counterexample.** a later invocation inherits the in-flight equivalence relation after input state changed.

## SC-MO-REUSE-001 — durable result replay is explicit opt-in

**Statement.** Durable result reuse MUST require an adapter/policy that permits replay and binds immutable/sufficient input identity, result provenance, freshness/invalidation and authorization scope.

**Counterexample.** successful result automatically stored for five minutes for every non-mutating command.

## SC-MO-NATIVE-001 — delegate to stronger tool-native semantics

**Statement.** When a supported tool already exposes a stronger accepted cache/equivalence system, ShareCLI SHOULD integrate/delegate rather than maintain a parallel result cache unless comparative evidence establishes product value.

**Rationale.** sccache/build-system prior art and existence gate.

**Verification.** paired workload comparing status quo/tool-native vs ShareCLI coordination + tool-native semantics.

## SC-MO-FAIL-001 — failed/unavailable identity never becomes a cache green

**Statement.** Fingerprint/adapter failure, missing dependency identity, collector error, unsupported platform or ambiguous scope MUST produce bypass/unknown, not reusable success.

## SC-MO-SIDEFX-001 — side-effect/nondeterministic eligibility is explicit

**Statement.** Side-effectful or nondeterministic commands MUST NOT become durable-reuse eligible through argv pattern heuristics alone.

**Negative cases.** network/time/random reads, writes without familiar flags, hidden service mutation.

## SC-MO-SPEC-001 — speculation cannot outrun equivalence authority

**Statement.** Speculative pre-execution MAY occur only where the accepted adapter explicitly permits durable and speculative execution under the same evidence/authorization scope.

**Counterexample.** frequency histogram causes arbitrary command with external side effects to execute preemptively.

## SC-MO-FS-001 — filesystem interception mode is truthful

**Statement.** Filesystem interception MUST be represented as required, optional/degraded or unavailable. A failed optional FUSE mount cannot count as interception evidence.

**Native evidence.** SC-F01 run executed without FUSE after mount degradation and still exercised the cache path.

## Slice exit criteria

This slice can be promoted into the mature contract only after:
- equivalence-domain matrix and in-flight split experiments are harvested;
- ownership/PID-reuse fixtures execute;
- required/optional FUSE intent is reconciled;
- a first bounded adapter prototype is compared against tool-native/status-quo behavior;
- independent review actively attempts counterexamples.

No requirement-count target applies.
