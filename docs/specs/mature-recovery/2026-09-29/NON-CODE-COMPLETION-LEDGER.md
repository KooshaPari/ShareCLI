# ShareCLI non-code completion ledger — v1.0

Date: 2026-10-01.
Purpose: authoritative denominator for **specification, documentation, test/oracle design, research, architecture and governance work**. Production implementation/runtime proof is a separate denominator.

## Finalized non-code foundations

| Family | State | Finality |
|---|---|---|
| direct product thesis | CLOSED | direct-user authority: high-concurrency workload/resource defragmentation and optimization |
| product boundary | CLOSED | scheduling/packing/queues/coalescing/cache/speculation/resource pressure core; generic agent planning/consensus not implied |
| authority model | CLOSED | user intent > accepted design > source fact/verified observation > imported assertion > inference |
| source/evidence identity doctrine | CLOSED | exact candidate/config/verifier/run/raw provenance required |
| worker/development/product lifetime separation | CLOSED | three lifetimes explicit |
| mature-first/stage doctrine | CLOSED | stages project mature ontology; no disposable PID/cache identity |
| ontology | V1.2 CLOSED FOR CURRENT AUTHORITY | WorkGraph/resource/scheduling + process/recovery/equivalence model represented |
| quality-overlay doctrine | CLOSED | quality properties attach by subject; no cloned generic requirements or invented SLOs |
| MACE/autograder doctrine | CLOSED | independent multidimensional grading; critical dimensions non-averagable |
| catalog quarantine | CLOSED AS POLICY | generated/legacy catalogs cannot establish acceptance |
| work-package DAG doctrine | CLOSED | machine package dependencies/gates/evidence encoded |
| orphan/scope doctrine | CLOSED | mounted code does not define accepted product scope |

## Contract families represented

- process observation and attribution;
- ownership/supervision/mediation;
- ProcessGeneration and controller replacement;
- policy scope/resolution;
- work equivalence;
- in-flight vs durable result sharing;
- execution attempts/result selection;
- filesystem interception;
- session/surface recovery;
- resource observation and pressure;
- workload graph/resource vectors/envelopes;
- admission/queue/fairness;
- scheduling/placement/replanning;
- packing/backfill;
- coalescing;
- speculation;
- native admission/cache/scheduler composition;
- security/authorization;
- persistence/migration/recovery;
- CLI/API/operator truth;
- install/update/uninstall/platform lifecycle;
- evidence/trace;
- stage projections and journeys.

No requirement count target is used.

## SOTA/alternatives status

### Architecture-significant conclusions closed enough for specification
- native GNU jobserver should be composed when available;
- product leases cover non-participating workloads;
- scheduler strategies are pluggable;
- deterministic FIFO/binpack/backfill/fairness/device-aware/pressure-aware strategies precede learned scheduling;
- coalescing/cache eligibility is adapter-bounded semantic policy, not generic command hashing;
- speculative racing is a strategy with explicit budget/delay/cancellation/waste accounting;
- Ray/Volcano/Slurm/Nomad/Bazel are prior art/benchmarks, not mandatory runtime identity.

### Empirical questions intentionally not spec-finalized
- best strategy for the user's actual workload distribution;
- numeric resource/latency/throughput targets;
- learned scheduler value;
- exact speculation delay/budget;
- tool-specific durable equivalence adapters.

These require benchmark/pilot evidence and are not missing documentation.

## Journey finality

Mature journeys are defined; two critical proof slices are explicit:
1. identity/recovery: local session recovery;
2. product thesis: constrained workload scheduling/packing versus naive baseline.

Journey **contracts/oracles are final enough to implement**. Journey closure is runtime evidence and remains separate.

## Test/oracle design finality

Required adversarial families are represented:
- PID reuse/controller replacement;
- stale/missing/conflicting evidence;
- equivalence changed bytes/workspace/env/external input;
- in-flight vs durable owner failure;
- queue fairness/aging/reclaim;
- hard resource envelope;
- unknown demand;
- plan vs actual placement;
- pressure observation vs enforcement;
- speculation loser/waste;
- native provider composition;
- FUSE optional/required;
- session exit/recoverability;
- update/uninstall state disposition.

## Remaining non-code blockers

These are the only reasons this ledger does not claim global non-code 100%:

1. **Claude corpus not yet ingested.** User states it contains highly relevant conversations; authority/alias denominator cannot be called exhausted before review.
2. **Exited-session semantic decision.** Direct authority does not yet decide deliberate completion vs crash-resumable exit policy.
3. **Fresh independent falsification review.** Must occur after Claude delta and empirical architecture-risk receipts.
4. **Empirical architecture gates:** in-flight/durable, queue/PID-reuse, native jobserver composition, required FUSE, scheduling benchmark. Their oracle design exists; outcomes cannot be documented in advance.
5. **Numeric quality targets.** Must come from measured baseline/user objectives, not invention.

Everything else should be treated as documentation/specification debt only if a concrete uncovered behavior is produced.

## Finality rule

A future reviewer may reopen this non-code contract only by supplying:
- new user authority;
- a previously unreviewed source family;
- a concrete behavior/journey/configuration/failure architecture the ontology cannot explain;
- external prior art that falsifies a bootstrap/differentiation decision;
- empirical evidence falsifying an assumption.

“More requirements could be written” is not sufficient.
