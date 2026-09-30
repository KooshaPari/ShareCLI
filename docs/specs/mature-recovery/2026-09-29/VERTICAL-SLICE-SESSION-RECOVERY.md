# ShareCLI session recovery vertical-slice contract — v0.1

Status: **DESIGN/ORACLE CONTRACT / production remediation Tier B**  
Date: 2026-09-30.

## Current structural blocker

The frozen session model records process evidence on `SurfaceRecord.process`:
- PID;
- tty;
- cwd;
- argv;
- `started_at`.

But `AgentSession` and `ResumeRecipe` — the objects consumed by `RecoveryExecutor` — contain no ProcessGeneration/OwnershipClaim identity.

`RecoveryExecutor::execute` checks:
- confidence;
- session state;
- argv/cwd recipe validity;

then spawns the recipe.

It has no process-generation fact available to revalidate.

Therefore generation-safe recovery cannot be implemented as a small conditional inside the existing executor. The identity must survive observation→resolution→plan→execution.

## Required vertical identities

### RecoverySubject
- durable session identity;
- harness/session identity;
- originating observation(s);
- last known Surface identity;
- last known ProcessGeneration identity if one existed;
- OwnershipClaim/authority;
- evidence grade/freshness.

### RecoveryOperation
- operation ID;
- subject;
- plan revision;
- principal;
- requested mode: dry-run/execute;
- provider/adapter;
- created time;
- state/result.

### ProcessGeneration
Platform-specific stable identity. PID is an attribute. Where only weak evidence exists, the subject cannot be auto-adopted.

## Required semantics

1. Recovery of an exited/crashed prior process may launch a new generation from a verified recipe.
2. Recovery MUST NOT assume a currently-live process with the same PID is the observed generation.
3. If a live matching session already exists, policy decides attach/adopt/skip; it is not silently duplicated.
4. Controller restart does not erase RecoverySubject/Operation.
5. Dry-run is the default and has no launch side effect.
6. Execute requires explicit authorization.
7. Successful spawn creates a new ProcessGeneration/Observation; it does not mutate history to say the old process resumed.
8. Evidence binds old observations, plan, operation, launch attempt and new generation.
9. Ambiguous/stale evidence produces skipped/clarify/degraded outcome, never unattended launch.
10. Provider/surface capability failure is distinct from recipe identity failure.

## Oracle bundle

### SC-VS-S01 durable observation
Append exact observation, close/reopen SQLite, recover identical observation + materialized session.

### SC-VS-S02 dry-run
Verified subject produces plan/result but no child process.

### SC-VS-S03 ambiguous evidence
Heuristic/unavailable subject cannot execute.

### SC-VS-S04 PID reuse
Recorded old ProcessGeneration has PID P/start A. Fixture creates/represents PID P/start B. Recovery must not adopt/control B as A.

### SC-VS-S05 already-live exact generation
If exact original generation is still live, recovery policy must not launch a duplicate by default.

### SC-VS-S06 controller restart
Create RecoveryOperation, recreate service/controller, continue observing same operation/subject.

### SC-VS-S07 authorized launch
Exited verified subject + explicit execute spawns one new generation using argv, never shell.

### SC-VS-S08 launch failure
Missing executable/invalid cwd returns bounded failure with no false Recovered state.

### SC-VS-S09 observation after launch
New generation/surface observation is appended and linked to operation.

### SC-VS-S10 stale evidence
Observation older than configured accepted freshness is excluded from unattended execution.

### SC-VS-S11 capability degradation
Missing Ghostty/native provider remains explicit degraded state and does not invent surface recovery.

### SC-VS-S12 idempotent operation retry
Retrying the same RecoveryOperation does not launch a second process after accepted success/unknown state without reconciliation.

## Required additive model change before implementation

Do not overload `AgentSession`.

Introduce an additive recovery-plan/subject object carrying process-generation/observation/authority identity. Existing session rows remain readable and migrate to `generation=unknown` where evidence never existed.

Historical rows MUST NOT be retrospectively marked generation-verified.

## Promotion gate

The session slice is closed only when SC-VS-S01..S12 execute against mounted CLI/RPC/application paths where applicable, not only unit helpers.
