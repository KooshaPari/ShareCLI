# ShareCLI developer work packages — vertical spine v0.1

Status: **TIER-A READY / TIER-B MERGE-GATED**  
Date: 2026-09-30.

These packages implement additive structure required by the accepted/provisional v1.1 ontology. They do not authorize broad feature work.

## SC-WP-A01 — semantic identity types

Goal: introduce stable domain types without changing existing execution behavior.

Add:
- PolicyScopeID / PolicyRevision / PolicyDecisionID;
- ProcessGenerationID abstraction;
- OwnershipClaimID;
- InvocationID / ExecutionAttemptID;
- EquivalenceAdapterID + version;
- RecoveryOperationID;
- EvidenceReceiptID.

Constraints:
- do not alias these to PID/string filenames as their semantics;
- serialization versioned;
- old rows/config remain readable;
- unknown legacy identity stays unknown.

Acceptance:
- round-trip serialization;
- inequality fixtures where PID is same but generation differs;
- old persisted fixture loads without fabricated generation.

## SC-WP-A02 — recovery subject/operation model

Goal: stop losing process/evidence identity between SurfaceObservation and recovery execution.

Introduce additive:
`RecoverySubject {
 session_id,
 source_observation_ids,
 last_surface,
 last_process_generation?:,
 ownership_claim?:,
 evidence_grade,
 freshness,
 recipe
}`

and:
`RecoveryOperation {
 id,
 subject,
 plan_revision,
 principal,
 mode,
 state,
 attempts,
 result
}`

Do not yet replace public recovery execution.

Acceptance:
- observation materializes RecoverySubject preserving process start/generation evidence;
- legacy session becomes generation unknown;
- controller/store reopen preserves subject/operation;
- no child process is spawned by model tests.

## SC-WP-A03 — capability truth projection

Create one product-level capability object consumed by CLI/API/tray/dashboard:
`observed, attributed, owned, supervised, mediated, filesystem, optimization, result_shareable, recovery_managed`.

Each fact includes provenance/freshness where applicable.

Acceptance:
- observed-only fixture cannot render managed/optimized;
- mediated passthrough cannot render result-shareable;
- optional FUSE failure renders degraded/unavailable.

## SC-WP-A04 — equivalence adapter interface

Introduce:
`EquivalenceDecision = Bypass | InFlight(key) | Durable(key, evidence)`.

Unknown adapter or unsupported invocation = Bypass.

No current Git/Time/Args mode may automatically emit Durable through this new interface.

Acceptance:
- deterministic fixture adapter;
- changed declared input changes key;
- unknown command bypasses;
- adapter/version appears in evidence.

## SC-WP-A05 — trace/evidence qualification validator

Machine-check recovery docs/catalog:
- semantic trace edge requires obligation/decision/implementation/oracle/evidence identities;
- old audit/score rows are excluded unless qualified;
- green evidence requires candidate/config/verifier/run identity;
- skipped/not-run/stale/wrong-candidate cannot satisfy criterion.

## SC-WP-B01 — session recovery integration

Merge gate:
- resolve Exited observation semantics;
- generation-safe recovery model reviewed;
- SC-VS-S01..S12 oracle harness available.

Then migrate public session recovery onto RecoverySubject/Operation.

Forbidden shortcut:
- PID-only live check;
- copying `started_at` into a string and calling it solved;
- marking old rows verified.

## SC-WP-B02 — in-flight/durable split

Merge gate:
- exact in-flight-vs-durable oracle receipt;
- owner crash/retry semantics accepted.

## SC-WP-B03 — admission provider

Prototype:
- NativeJobserver;
- ProductLeaseProvider;
- provider selection/composition.

Merge gate:
- queue PID-reuse/fairness receipts;
- native jobserver comparative fixture.

## SC-WP-B04 — FUSE required mode

Merge gate:
- explicit dependent journey;
- fail-closed required-mode oracle.

## Package ordering

`A01 → A02/A03/A04 → A05 → B01/B02/B03/B04`

A02/A03/A04 may proceed in parallel after A01.
