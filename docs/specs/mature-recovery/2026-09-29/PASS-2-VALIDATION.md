# ShareCLI — recovery pass 2: trace authority, native oracle and catalog quarantine

Date: 2026-09-29. Analyzed product source remains `4f01d0199e82b62bcf20399afcc102f58a10ad07`. This pass operates on draft specification branch only. Native adversarial-test commit: `f1dd74bfed1f08d4a2cc1ab7deba63b7b3b6b1a9`.

## New source resolution

### SC-S15 — canonical FR and traceability documents

`docs/specs/FR.md` and `docs/specs/TRACEABILITY.md` were read beyond the root index. They classify FR-008 as accepted and map AC-008.13 to cwd/env isolation, AC-008.14 to Critical-before-Normal queue ordering, and later ACs to cache-key modes, semantic argv normalization and harness strategy routing.

This is important negative evidence: **the existing accepted acceptance surface does not establish semantic equivalence of the command's actual inputs.** A passing AC-008.13/19/20 can coexist with SC-F01. Trace completeness by filename/test association therefore cannot grade result correctness.

The same trace corpus contains extensive capability growth under single FRs. Requirement/AC count is not used as a maturity proxy. Existing stable IDs remain preserved until semantic reconciliation decides whether an AC is accepted product behavior, implementation-detail constraint, quality overlay, historical recovery criterion or superseded/generated audit residue.

### SC-S16 — existing audit/scorecard claims

`COMPREHENSIVE_AUDIT_SCORECARD.md` asserts FR-008/009 COMPLETE and full bidirectional traceability based on mapped source/test/doc presence. `WORK_DAG.md` and 88-pillar workflows contain unrelated rubric-driven tasks and scores.

**Disposition: AUXILIARY / NON-GRADING for this mature-recovery program unless individually requalified.** These artifacts may supply discovery leads. Their completeness/score labels cannot satisfy source resolution, journey closure, evidence identity or semantic correctness. The current counterexample is a direct falsification of using their FR-008 COMPLETE label as product acceptance.

### SC-S17 — native adversarial control added

`tests/recovery_fr008_semantic_identity.rs` at draft commit `f1dd74bfed1f08d4a2cc1ab7deba63b7b3b6b1a9` exercises the actual Rust `Hypervisor` with `CacheKeyMode::Git`. It creates a real Git repository, commits a baseline, writes `first edit`, runs `cat input.txt`, rewrites the same already-modified file to `second edit` without changing HEAD/status shape, and requires the second invocation to return the second bytes and not be a cache hit.

This is an **oracle-only expected-failure fixture**. No implementation fix is present. GitHub Actions was triggered for this commit; its result is pending at this document's creation. A failed unrelated workflow is not evidence for SC-F01; only a job that compiles/runs this exact test, with logs tied to this commit, can close the native reproduction step.

## Revised FR-008 semantic model

The safe default is not “same invocation string.” Reuse requires an **authorized equivalence adapter** that can establish the relevant input identity for a known operation. Candidate dimensions include executable/tool identity, normalized arguments, working tree/input snapshot, relevant environment/configuration, principal/authorization scope, dependency/toolchain identity and declared external inputs. Unknown or unobservable dimensions force bypass unless the accepted adapter explicitly proves they are irrelevant.

This does not require one universal Merkle model. Tool-native caches may already have stronger semantics and should be composed instead of wrapped. In-flight singleflight/coalescing and durable result caching are separate mechanisms: concurrent duplicate suppression can sometimes be safe with a narrower lifetime than replaying a five-minute-old result.

## Queue semantics correction

Existing AC-008.14 says Critical MUST acquire before Normal under contention. That is a priority property, not a complete fairness/recovery contract. The current aging implementation was introduced to prevent orphan starvation but its u8 cast and ticket-string ordering create counterexamples outside the narrow AC. The mature contract must define:
- live-owner versus orphan detection;
- lease/generation or equivalent fencing identity;
- bounded wait/reclaim semantics;
- fairness policy and whether strict priority may starve lower classes;
- clock source and overflow behavior;
- crash/kill/restart cleanup;
- exact behavior when cleanup itself fails.

Do not “fix the wrap” while leaving ownership semantics undefined.

## SOTA pass-2 implications

Systemd/cgroup-style native resource controls and PSI-like signals strengthen the case for using OS primitives for observation/admission rather than treating agent count as the fundamental signal. Bazel-like action/input identity and tool-native caches strengthen the rejection of generic arbitrary-command replay. sccache is a concrete example of a specialized cache/distributed execution mechanism whose narrower semantic domain is an advantage, not a missing feature.

The custom thesis that remains worth testing is the composition layer: heterogeneous agent observation, truthful owned-process control, safe adapter-selected work sharing, and a coherent operator experience. Generic supervision, generic cache mechanics and generic priority queues are not differentiation by themselves.

## Gate delta

- Source ledger: still OPEN; SC-S15/16 resolve two important families only partially.
- Catalog exclusion: **semantically decided, technically unenforced**. Scorecards/88-pillar outputs are discovery-only for this program.
- Native SC-F01 reproduction: TEST COMMITTED / CI PENDING.
- Queue native counterexamples: still model-only.
- Architecture freeze: BLOCKED.
- Completion percentage: null.
