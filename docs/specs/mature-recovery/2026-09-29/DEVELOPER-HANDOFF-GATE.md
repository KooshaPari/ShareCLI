# ShareCLI developer-agent handoff gate — v0.1

Status: **PARTIAL HANDOFF READY**  
Date: 2026-09-30.

Developer agents MAY begin bounded remediation/prototype work listed below. They MUST NOT treat the mature specification/design program as complete or implement speculative unresolved families as settled architecture.

## Green-to-implement work packages

### SC-DEV-01 — split in-flight coordination from durable result storage
Goal: create explicit internal abstractions so an execution can be shared while active without automatically becoming replayable afterward.

Constraints:
- preserve normal execution when no adapter authorizes durable reuse;
- no generic Time/Args/Git fallback may manufacture durable eligibility;
- existing behavior may remain behind compatibility flags during migration;
- do not delete historical modes until migration/compatibility decision is explicit.

Required tests:
- concurrent same accepted equivalence executes underlying command once;
- after completion, changed relevant input executes again unless durable adapter authorizes replay;
- owner failure wakes/retries/bypasses correctly;
- no cached-success result from an UNKNOWN equivalence decision.

### SC-DEV-02 — equivalence adapter interface
Implement a minimal versioned adapter contract and BYPASS/IN_FLIGHT/DURABLE decision type.

Do **not** implement a broad catalog yet. Start with:
- one deterministic fixture adapter;
- unknown adapter fallback = BYPASS;
- evidence exposes adapter/version/relevant-input identity.

### SC-DEV-03 — queue identity/fairness correctness
Fix independently demonstrated structural defects without expanding queue scope:
- saturating aging, no u8 wrap;
- semantic numeric FIFO;
- stale/dead waiter reclaim;
- generation-safe ownership abstraction;
- cleanup failures observable.

Cross-platform ownership may use platform adapters. Do not hard-code Linux pidfd as the public ontology.

### SC-DEV-04 — truthful FUSE mode plumbing
Introduce off/optional/required policy and capability result.
- optional unavailable → continue + degraded evidence;
- required unavailable → fail before dependent command;
- off → no mount attempt.

### SC-DEV-05 — capability-truth projection
Expose enough machine-readable state to distinguish observed/owned/mediated/equivalence/filesystem/recovery facts.

## Prototype-only work packages

### SC-EXP-01 — native jobserver
One nested-build fixture comparing native jobserver, ProductLease, and explicit composition.

### SC-EXP-02 — process-generation providers
Compare Linux pidfd and corresponding Windows/macOS primitives behind one ProcessGeneration/OwnerHandle interface.

### SC-EXP-03 — tool-specific durable adapter
Select one real tool family only after comparative value vs its native cache is demonstrated.

## Not authorized yet

- universal durable command cache rewrite;
- universal FUSE requirement;
- mesh/distributed expansion;
- new dashboard feature growth unrelated to capability truth;
- third-party orchestration platform;
- requirement-count padding;
- declaring generic cache modes correct by adding more hash fields.

## Mandatory independent receipts before merging architecture-sensitive remediation

Each PR must identify:
- ontology/obligation IDs;
- decision record;
- exact candidate SHA;
- exact dependency revisions;
- positive and negative oracle commands;
- run/job IDs;
- executed-case count;
- unresolved risks.

A developer-written unit test alone is insufficient for the architecture-risk gates.

## Handoff verdict

**READY FOR DEVELOPER AGENTS: YES, bounded work packages SC-DEV-01..05 and experiments SC-EXP-01..03.**

**READY FOR “implement the whole mature ShareCLI”: NO.**

Specification/design completion remains gated by the pass-2 coverage ledger and independent architecture review.
