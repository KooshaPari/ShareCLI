# ShareCLI session recovery semantic contradiction — exited observations

Date: 2026-09-30.

## Source fact

Frozen `SessionStore::recovery_plan(max_age)`:
- selects newest fresh observation per surface;
- ignores malformed/future/stale observations;
- **skips the observation entirely when `ObservationKind::Exited`**;
- then includes only auto-resumable sessions.

Frozen `RecoveryExecutor::launch` separately permits session states:
- Active;
- Exited;
- Pending.

Thus planning and execution disagree about whether exited evidence can enter recovery.

## Why this matters

The intended session-recovery journey includes recovery after terminal/controller/harness crashes. Depending on observation semantics, an exited prior harness may be exactly the subject that needs a resume recipe.

But blindly including every exited observation could also be wrong:
- deliberate user exit;
- completed task;
- explicit logout;
- stale superseded surface;
- session already resumed elsewhere.

Therefore the correct rule is not “remove the Exited filter” or “executor should reject Exited” without product semantics.

## Required ontology refinement

Separate:
- `Process/SurfaceObservationKind` — what happened to observed runtime;
- `SessionRecoverability` — whether the durable logical harness session is eligible for recovery;
- `RecoveryReason` — crash/provider loss/operator request/planned restart/etc.;
- `RecoveryDisposition` — resume/skip/clarify/already-live/completed.

An exited process generation does not imply the logical session is complete, and an active historical session row does not prove the process is live.

## Required oracle

Create observations for:
1. clean deliberate completion;
2. crash/terminal loss with exact resumable session identity;
3. stale exited observation followed by newer active generation;
4. exited old generation with same PID reused elsewhere;
5. already-resumed session on another surface.

The recovery plan must select by accepted logical-session/recovery policy, not simply `ObservationKind != Exited`.

## Gate

SC-J05 remains NOT CLOSED. No production change authorized until the intended exit/recovery semantics are resolved from user intent/current session design and encoded in the vertical-slice contract.
