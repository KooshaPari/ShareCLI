# ShareCLI vertical-slice autograder contract — v0.1

Status: **CONTROL PLANE / independent grader specification**  
Date: 2026-09-30.

## Dimensions

A work package result is a vector, not one score:

- contract conformance;
- trace completeness;
- identity integrity;
- positive behavior;
- negative/adversarial behavior;
- recovery behavior;
- evidence identity;
- regression;
- platform applicability;
- unresolved uncertainty.

Critical identity/security dimensions cannot be averaged away.

## Hard fail conditions

Any of:
- skipped/not-run oracle reported green;
- evidence candidate/dependency mismatch;
- old generated scorecard used as acceptance;
- PID-only ProcessGeneration claim;
- observed workload reported owned/mediated without evidence;
- unknown equivalence produces durable reuse;
- required FUSE path silently degrades;
- recovery launches from ambiguous evidence;
- worker/controller replacement loses durable accepted state.

## Package grader

For each SC-WP:
1. load exact obligation/decision IDs;
2. verify diff touches only authorized surfaces or has explicit expansion;
3. compile/test;
4. execute package positive controls;
5. execute negative controls;
6. inspect exact evidence receipt;
7. reverse-trace changed implementation to contract;
8. detect orphan new public behavior;
9. compare against prior accepted candidate for regressions;
10. emit PASS/FAIL/UNKNOWN per dimension.

## Session vertical grader

Critical criteria:
- S01 persistence;
- S02 dry-run no side effect;
- S03 ambiguous evidence blocks;
- S04 PID reuse safe;
- S05 already-live no duplicate;
- S06 controller restart continuity;
- S07 exact authorized launch;
- S08 bounded launch failure;
- S09 new-generation evidence;
- S10 stale evidence blocks unattended;
- S11 provider degradation truthful;
- S12 retry idempotency.

No overall PASS if S03/S04/S05/S06/S12 is FAIL/UNKNOWN.

## Feedback compression

Worker receives:
- failing criterion IDs;
- smallest reproducing fixture;
- expected vs observed identity/state;
- exact relevant contract fragment;
- previous attempt delta.

Do not send entire dossier back on every retry.

## Anti-gaming

- grader files protected from implementation worker changes unless separately reviewed;
- held-out PID/generation/observation fixtures;
- mutation controls remove generation check / force cache hit / flip capability truth;
- exact grader revision in receipt;
- immutable attempt history.
