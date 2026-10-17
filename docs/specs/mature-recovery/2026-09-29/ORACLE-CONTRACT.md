# ShareCLI — independent acceptance/oracle design, pass 1

Source `4f01d0199e82b62bcf20399afcc102f58a10ad07`, date2026-09-29. **Design, not a deployed authoritative grader.** Full criterion inventory is not frozen. No imported generated catalog or model-only experiment participates in product acceptance.

## Control loop and lifetimes

Accepted assignment/contract → bounded worker context → minimal change → independent grader → multidimensional result → localized feedback → retry/replan/clarify/escalate. The worker may read the rubric. Its patch must not approve its own changed rubric, remove mandatory tests, relabel failures as skips, or move evidence from a different candidate into this result.

WorkerAttempt is ephemeral (agent/human/model/tools/worktree/lease/actions). DevelopmentEffort persists (intent, plans, dependencies, reviewed specs, grader runs and receipts). ProductState persists independently (accepted configuration, product/process/workspace identities, released artifacts, observed behavior and evidence). A supervised agent process in ShareCLI's domain is not automatically the developer-worker attempt building ShareCLI. Do not add a development-management platform to ShareCLI merely to model these lifetimes.

## Evidence identity

Every criterion receipt requires product ID; subject/capability; accepted contract revision; criterion ID; implementation candidate SHA and artifact digest; full configuration identity; environment/platform/dependency identity; verifier identity/version/digest; evaluation/run ID; timestamp; raw artifact location and digest; result and executed-case count. Product state and runtime process/lease generation are included where applicable. Candidate must be immutable before evaluation. Collection failures, wrong candidate, stale environment, skipped/empty suites, missing artifacts and mismatched digests are **UNKNOWN/INVALID**, never green.

Authority-aware trace edges preserve source, author/authority type, exact revision, validation state, relation and supersession. Inference is explicitly labeled even at high confidence. Intent↔design↔implementation↔test↔evidence↔runtime edges must be traversable both directions; an index filename is not an implementation edge.

## Adversarial oracle matrix

| Behavior | Positive oracle | Negative/adversarial controls | Required observation |
|---|---|---|---|
| Owned lifecycle | Actual spawned child visible and stopped | Foreign live PID, reused PID, wrong project, denial discarded by tray, repeated stop | OS process identity plus actual CLI/IPC/UI result |
| Safe sharing | Same authorized immutable inputs return equal current output with fewer executions | Same status/different bytes; tool/env/principal changes; unavailable fingerprint; malicious cache entry; mutation without familiar flags | Independent command baseline, execution counter, artifact hash and trusted scope |
| Cache/queue recovery | Waiter receives correct result after legitimate holder completion | SIGKILL holder; partial JSON; failed rename/unlink; stale/foreign lease; priority aging wrap; sequence10/2; backward clock | Durable state, bounded deadline and no wrong-owner action |
| FUSE | Declared optional/required mode accurately reflected | Missing privilege, unsupported OS, mount failure, same-path changed bytes/mtime, stale read, concurrent writes | Actual filesystem effects, provenance and mode-aware fail/degrade outcome |
| Resource policy | Measured pressure affects declared admission behavior | Missing collector, overload, dependency failure, stale healthy sample, recovery after refusal | Ground-truth host signals and control effects; not log keywords alone |
| Operator continuity | Restart/reconnect shows persistent state and truthful unknowns | Worker replacement, wedged sidecar, partial config save, lost socket, stale prior evidence | Fresh independent public-path receipt and durable state identity |

For each guard ask whether deleting it manufactures a false green. Useful mutations include removing candidate comparison, treating zero tests as pass, accepting cache output on changed content, trusting PID alone, allowing a live lease to be reclaimed, or swallowing a negative IPC result. Hold out adversarial fixtures where useful; secrecy is not the principal defense.

## Independence and scoring

Trusted grading policy must be selected outside the implementation candidate; changes require separate authorized review. Run baseline/candidate with the same sealed verifier, preserve raw append-only receipts, deny network/credential access not necessary for the fixture, and report collector faults separately. The candidate can supply test adapters but cannot decide acceptance eligibility of its own evidence. This enforcement infrastructure is not implemented by these documents.

Report functional coverage, trace validity, evidence validity, journey closure, regression, performance, reliability, security, accessibility/usability, uncertainty and transition debt separately. Critical ownership/equivalence/security failures veto acceptance; no average can hide them. Scope expansion and engineering progress use separate deltas. Position, delta, slope, confidence, regressions, oscillation and evidence age require actual comparable observations; no velocity/asymptote is inferred from this one pass.

Current vector: mature denominator UNKNOWN; native functional/journey coverage UNKNOWN; model counterexamples reproduced3; risk closure0 demonstrated; independent fresh review NOT_RUN. These are evidence statuses, not a product percentage.
