# Architecture decision candidate SC-AD-03 — admission providers, lease truth, and tool-native jobservers

Status: **PROVISIONAL ACCEPT / native queue counterexamples pending**  
Date: 2026-09-30.

## Context

ShareCLI has at least two different concurrency problems:

1. **tool/build internal parallelism** — e.g. recursive build tools that already support a jobserver/token protocol;
2. **ShareCLI-owned admission of independent mutating workloads** — where ShareCLI must bound how many commands/workloads it lets run.

These should not be forced through one custom filesystem priority queue.

Current source also contains a historical `jobserver` strategy name, but its implementation merely delegates to the ordinary process/Hypervisor lane and ignores parsed `jobserver_auth`, `jobserver_tokens`, and `jobserver_borrow`. This is not actual jobserver participation.

GNU make's jobserver contract and the Rust `jobserver` crate provide an established token-based model for tools that can participate: acquire a token, perform bounded work, and return the exact token even on error/interrupt. POSIX and Windows use different underlying primitives while preserving the token contract.

## Decision candidate

Introduce an explicit **AdmissionProvider** boundary rather than treating `SlotQueue` as the universal concurrency owner.

### ToolNativeAdmission

For a tool/ecosystem with an accepted native coordination protocol:
- detect/attach to the native admission system;
- propagate it correctly to child work where required;
- acquire/release native tokens using established semantics;
- do not add a second independent ShareCLI concurrency cap unless policy explicitly composes both.

Initial candidate: GNU/Rust jobserver-compatible build processes.

### ShareCLILocalAdmission

For workloads ShareCLI itself must bound and for which no stronger native protocol applies:
- use a ShareCLI-owned local admission mechanism;
- separate **slot ownership/liveness** from **fairness/priority ordering**;
- make owner identity and lease/generation explicit;
- ensure crash/error/timeout returns or invalidates admission state;
- preserve platform-specific implementation freedom.

## Queue semantic corrections required

### Ownership/liveness

A waiter ticket is ordering metadata, not proof a live owner exists.

Stale/orphan detection SHOULD use an ownership identity such as:
- process identity plus start/generation evidence;
- random lease/generation token;
- bounded lease/heartbeat where process liveness alone is insufficient.

Priority aging MUST NOT serve as the only orphan-recovery mechanism.

The February donor is stronger than the current Rust port on one point: `queue::dequeue` and `queue::peek` perform `kill -0` on the recorded waiter PID and remove dead tickets. The Rust recovery dropped this liveness filter. That donor behavior should be recovered as an outcome, but not copied literally as the final authority model because a live reused PID can still satisfy `kill -0`. Mature ShareCLI therefore needs process-generation/lease evidence in addition to basic liveness.

### Fairness

Priority policy must be stated independently from filename representation.

Required properties:
- monotonic aging without integer wrap;
- numeric FIFO/order for equal effective priority;
- defined starvation policy;
- priority changes do not resurrect stale owners;
- clock anomalies cannot manufacture higher authority.

### Slot acquisition

The OS lock/semaphore/token is the mutual-exclusion primitive. Queue metadata may select who is eligible to attempt acquisition but cannot manufacture ownership of a slot.

### Evidence

A queue/admission receipt identifies:
- admission provider + version;
- lane/resource;
- owner/lease generation;
- priority/fairness policy;
- acquire/release timestamps;
- timeout/reclaim reason;
- execution subject.

## Existing mechanism disposition

| Mechanism | Disposition candidate |
|---|---|
| `SlotQueue` file locks | ADAPT for ShareCLI-local admission if crash/fairness model survives experiments |
| waiter ticket filenames | REPLACE/REFINE as policy metadata; no authority by lexical shape |
| age-only orphan handling | REJECT as liveness model |
| historical `jobserver` strategy alias | REJECT AS FALSE CAPABILITY until real protocol integration exists |
| GNU/Rust jobserver for eligible builds | INTEGRATE rather than reimplement |
| OS-native process/resource admission | COMPOSE where it supplies stronger resource enforcement |

## Native adversarial program

1. Critical aging at 255/256/300 steps — rank must remain monotonic/saturated.
2. Equal rank same timestamp sequence 2 vs 10 — numeric FIFO.
3. Owner process exits while waiter metadata survives.
4. PID reuse with stale ticket/lease.
5. holder closure errors/panics/timeouts — all admission state released.
6. waiter's own process dies before acquisition.
7. restart with stale queue directory.
8. clock moves backward/forward.
9. strict priority workload with lower-class starvation pressure.
10. tool-native jobserver fixture: ShareCLI must not oversubscribe the native token budget.

## Falsification criteria

Revise this decision if:
- the mature product authority requires one globally consistent queue across all tool families and native jobservers cannot preserve it;
- an established local scheduler/admission library satisfies the full accepted local contract with lower integration burden;
- or the queue outcome is better owned entirely by a broader runtime/fabric and ShareCLI only observes/adapts.

## Current gate

SC-F02/SC-F03 native tests are committed and pending execution. Until those and owner-death/restart experiments land, no queue implementation remediation is architecture-complete.
