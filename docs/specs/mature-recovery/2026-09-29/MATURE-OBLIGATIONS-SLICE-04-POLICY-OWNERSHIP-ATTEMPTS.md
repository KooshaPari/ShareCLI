# ShareCLI mature obligations — semantic slice 04: policy, ownership, attempts

Status: **PROVISIONAL SEMANTIC SLICE / NOT FULL CONTRACT**  
Date: 2026-09-30.

## SC-MO-POL-001 — overlapping policy scopes resolve explicitly
Host, Principal, Project and Workspace policy inputs MAY overlap. The product MUST produce a versioned PolicyDecision with provenance to contributing scopes and deterministic precedence/composition.

**Negative:** whichever config file is loaded last silently decides whether a command may be mediated or reused.

## SC-MO-POL-002 — policy changes invalidate dependent authority
A PolicyDecision used for admission/equivalence/filesystem behavior MUST identify the policy revision. Evidence/reuse decisions from incompatible policy revisions cannot silently qualify a later operation.

## SC-MO-OWN-001 — ownership survives controller replacement
OwnershipClaim binds authorized Principal/Project to ProcessGeneration/workload independently of the ephemeral ShareCLI controller process.

**Positive:** supervisor process restarts and can re-establish control only after validating durable claim + live ProcessGeneration.
**Negative:** new controller sees same PID and assumes ownership.

## SC-MO-OWN-002 — adoption is explicit
Adopting an externally created or orphaned process into managed ownership requires an authorized transition and evidence. Observation alone is insufficient.

## SC-MO-ATT-001 — retries/speculation create distinct attempts
Every real execution has a distinct ExecutionAttempt identity even when derived from the same Invocation/equivalence class.

## SC-MO-ATT-002 — attempt disposition is durable
Selected, superseded, cancelled, failed and unknown attempts remain distinguishable for evidence/recovery.

## SC-MO-ATT-003 — one result becomes authoritative explicitly
When multiple attempts succeed, ResultSelection identifies the accepted result and reason/policy. Durable reuse cannot bind to a losing/speculative result merely because it finished first.

## SC-MO-RES-001 — native/external cache provenance is preserved
If an underlying tool/native cache supplies a result, ShareCLI records imported/native producer identity rather than inventing a local ExecutionAttempt that did not occur.

## SC-MO-FS-009 — filesystem coverage is scoped
FilesystemSession states which paths/process descendants/operations are covered. Ready mount != universal I/O interception.

## SC-MO-BOUND-001 — current mature boundary is local/owned-workstation
Distributed/remote mesh coordination is outside the currently accepted mature spine unless separately authorized. Existing mesh code is not authority to expand scope.

## Verification backlog
- conflicting Host/Workspace policies;
- controller restart + PID reuse;
- explicit adoption of external process;
- two speculative successes with deterministic selection;
- imported native-cache hit;
- subprocess/path escaping filesystem coverage;
- mesh feature cannot become required for local journey.
