# ShareCLI forward WBS — mature recovery after handoff gate

Status: ACTIVE  
Date: 2026-09-30.

## Critical path

`W4 native falsification → W5 architecture candidate → W6 bounded implementation → W7 vertical/public-path verification → W8 contract expansion → W9 independent review → W10 spec/design freeze`

## W4 — finish architecture-risk evidence

### W4.1 In-flight vs durable
- execute current `recovery_fr008_inflight_vs_durable`;
- preserve raw receipt;
- if candidate fails later replay assertion, classify current coupling;
- if passes, inspect whether pass is accidental due another behavior.

Exit: exact run/job/candidate/environment receipt.

### W4.2 Queue correctness
- execute aging overflow;
- numeric FIFO;
- dead waiter;
- PID-reuse waiter;
- panic/error/timeout cleanup.

Exit: every adversary classified PASS/FAIL/INVALID independently.

### W4.3 Capability truth
Fixture matrix:
- externally launched detected process;
- dispatcher-mediated process;
- cache/in-flight eligible process;
- FUSE optional unavailable;
- FUSE required unavailable.

Exit: public machine state distinguishes each.

## W5 — architecture experiments

### W5.1 Native jobserver comparison
A/B/C:
- native jobserver;
- ProductLeaseProvider;
- explicit composition.

Metrics: peak jobs, elapsed, nested propagation, leak after interrupt, recovery.

### W5.2 ProcessGeneration provider
Prototype platform adapter:
- Linux pidfd;
- Windows process handle + creation identity;
- macOS stable generation candidate.

### W5.3 Equivalence adapter
One deterministic bounded adapter, preferably a real high-value tool only after status-quo/native-cache comparison.

### W5.4 Required filesystem mode
Prove dependent execution never starts if required interception cannot be established.

## W6 — implementation spine

Dependency order:
1. capability-state types;
2. ProcessGeneration/OwnerHandle;
3. AdmissionProvider + Lease;
4. EquivalenceDecision/Adapter;
5. in-flight coordinator;
6. durable ResultStore interface;
7. FUSE policy/session;
8. evidence receipt schema;
9. CLI/API/tray projections.

Parallelizable:
- 2 and 4;
- 3 and 7 after common capability types;
- UI projections only after stable state schema.

## W7 — public-path verticals

### W7.1 Observe-only
foreign process detected; no control authority.

### W7.2 Owned supervision
owned child start/observe/stop/restart with PID reuse negative.

### W7.3 Equivalent concurrent work
two concurrent requests share one execution.

### W7.4 Changed-input later work
same invocation shape after relevant input change executes anew.

### W7.5 Admission recovery
holder dies; waiter recovers without stale authority.

### W7.6 Filesystem modes
off/optional/required truth.

## W8 — remaining mature contract families

Decompose only after spine evidence:
- mesh/session scope;
- remote/distributed coordination if still authorized;
- resource/thermal policy;
- configuration/projects;
- health/status;
- tray/dashboard;
- CLI/IPC protocol;
- storage/migrations;
- auth/security;
- packaging/install;
- platform parity;
- release/update/rollback/uninstall;
- observability;
- docs/support/accessibility.

For every family: authority → ontology → obligations → implementation → oracle → evidence → reverse trace.

## W9 — independent adversarial review

Separate reviewer attacks:
- hidden side effects;
- external inputs;
- cross-principal reuse;
- process replacement;
- lease theft;
- clock movement;
- unsupported OS;
- FUSE degradation;
- stale evidence;
- native-provider conflict;
- candidate changing grader.

All findings enter contradiction/risk ledger.

## W10 — freeze gate

Require:
- ontology accepted;
- architecture decisions accepted;
- no blocking authority conflict;
- family denominator closed or explicit excluded scope;
- every critical obligation has positive + negative oracle;
- bidirectional trace complete for accepted scope;
- architecture-risk experiments executed;
- implementation-ready work packages unambiguous;
- independent review complete.

No numeric requirement-count target.
