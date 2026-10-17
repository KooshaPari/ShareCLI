# ShareCLI mature obligations — semantic slice 06: workload scheduling and defragmentation

Status: **PROVISIONAL / DIRECT-USER-THESIS DERIVED**  
Date: 2026-09-30.

## Work/resource model

### SC-MO-SCHED-001
Every schedulable WorkItem has durable identity, readiness/dependency state and adapter/source provenance.

### SC-MO-SCHED-002
Resource demand may be measured, declared, estimated or unknown; provenance/confidence are preserved. Unknown demand is not zero.

### SC-MO-SCHED-003
ResourceEnvelope binds host/domain, resource dimensions, observation freshness and policy reservations.

### SC-MO-SCHED-004
SchedulingPolicy explicitly represents priorities, fairness, deadlines/time horizons and permitted optimization strategies.

## Admission / queue / placement

### SC-MO-SCHED-010
Admission produces explicit admit/queue/refuse/defer decision with reason and policy revision.

### SC-MO-SCHED-011
Queue order/aging/fairness are deterministic under the selected policy and cannot depend on lexical filenames/PID accidents.

### SC-MO-SCHED-012
SchedulePlan distinguishes proposal from actual Placement/ExecutionAttempt.

### SC-MO-SCHED-013
Placement cannot knowingly exceed hard ResourceEnvelope constraints absent explicit oversubscription policy.

### SC-MO-SCHED-014
Replanning preserves WorkItem identity and records why prior plan changed.

## Coalescing / caching / speculation

### SC-MO-SCHED-020
Coalescing/in-flight sharing requires accepted semantic equivalence; scheduler similarity alone cannot merge work.

### SC-MO-SCHED-021
Durable cache reuse requires stronger adapter evidence than in-flight coordination where semantics demand it.

### SC-MO-SCHED-022
SpeculationGroup declares budget/limit/cancellation/result-selection policy.

### SC-MO-SCHED-023
Losing speculative attempts are cancelled/reclaimed where safe and remain visible in evidence/cost accounting.

### SC-MO-SCHED-024
Native tool caches/jobservers/schedulers may satisfy capabilities through adapters and should be preferred when they dominate custom implementation.

## Pressure / stability

### SC-MO-SCHED-030
PressureEvent is an observation input, not proof that throttling occurred.

### SC-MO-SCHED-031
Pressure response may throttle/defer/cancel/replan according to policy and records affected work.

### SC-MO-SCHED-032
A scheduler must recover leaked/expired allocations without using PID existence alone as ownership proof.

## Outcome evidence

### SC-MO-SCHED-040
Optimization receipt binds workload, resource envelope, policy, scheduler/adapters, candidate, execution and measured outcomes.

### SC-MO-SCHED-041
Evaluation compares against realistic status quo/native alternatives on the same workload/environment.

### SC-MO-SCHED-042
No single metric establishes success. Applicable dimensions include useful throughput, completion/deadline, utilization, fairness, stability, overhead, wasted speculation and correctness.

### SC-MO-SCHED-043
An optimization that improves throughput by violating correctness/resource hard limits is not accepted.

## First scheduling vertical slice

Deterministic fixture:
- several WorkItems with CPU/RAM-like resource vectors and dependencies;
- envelope intentionally too small for naive all-at-once execution;
- naive baseline records pressure/failure/elapsed behavior;
- ShareCLI policy queues/packs work;
- exact placements/attempts recorded;
- all work completes without envelope violation;
- compare elapsed/useful throughput/peak pressure/queue fairness.

Then add one duplicate/equivalent pair to prove coalescing is independent from packing.

This is now a required CVP-level product proof alongside the session-recovery identity slice.
