# ShareCLI mature obligations — semantic slice 02: admission, leases, and fairness

Status: **PROVISIONAL SEMANTIC SLICE / NOT FULL CONTRACT**
Date: 2026-09-30.
Source snapshot: 4f01d0199e82b62bcf20399afcc102f58a10ad07.

This slice derives admission obligations from direct user contention intent, February donor behavior, current Rust source, SC-F02/03/06/07, and the provisional AdmissionProvider architecture.

## SC-MO-ADM-001 — admission provider selection is explicit

**Statement.** For each admitted workload family, ShareCLI MUST identify the admission provider/policy that owns concurrency authority: tool-native protocol, ShareCLI-local admission, OS/resource controller, or explicit composition.

**Counterexample.** Cargo/make-compatible work is limited independently by a native jobserver and an unrelated ShareCLI token count with no declared composition, causing accidental under/oversubscription.

## SC-MO-ADM-002 — false capability names are forbidden

**Statement.** ShareCLI MUST NOT report jobserver participation merely because strategy/configuration names contain jobserver fields.

**Current evidence.** Historical Rust jobserver strategy ignores jobserver auth/tokens/borrow and delegates to ordinary process/Hypervisor execution.

## SC-MO-LEASE-001 — queue ownership has generation-safe identity

**Statement.** ShareCLI-local admission ownership MUST use identity sufficient to distinguish a live current owner/waiter from a dead process or a later PID reuse.

**Positive acceptance.** dead waiter is removed/reclaimed; a new process reusing the same PID cannot inherit the old lease.

**Counterexample.** PID-only ticket passes liveness because a different process now owns the PID.

## SC-MO-LEASE-002 — dead waiter metadata cannot block live progress

**Statement.** Dead waiter metadata MUST cease participating in admission ordering within a bounded recovery interval independent of priority aging.

**Authority/evidence.** February donor dequeue/peek explicitly performed kill-0 and removed dead PID tickets; current Rust port dropped the liveness check.

## SC-MO-LEASE-003 — holder failure releases admission authority

**Statement.** Normal return, error, panic, timeout, process death, and host recovery MUST have defined slot/lease release or reclaim behavior.

**Positive control.** current OS file-lock/RAII path is being tested for post-panic lane usability.

## SC-MO-FAIR-001 — priority ordering is independent from storage encoding

**Statement.** Equal-effective-priority FIFO/ordering MUST compare semantic sequence/order values, not lexicographic ticket filenames.

**Counterexample.** sequence 10 is selected before sequence 2.

## SC-MO-FAIR-002 — aging is monotonic and bounded

**Statement.** Waiting-time priority adjustment MUST be monotonic according to the accepted fairness policy and MUST NOT wrap due integer conversion/overflow.

**Counterexample.** Critical rank at 255 steps is 255 but becomes 0 at 256.

## SC-MO-FAIR-003 — starvation policy is stated

**Statement.** The mature contract MUST state whether strict priority may starve lower classes, and if not, the bounded fairness mechanism/target.

**Counterexample.** continuously arriving high-priority work prevents Normal work indefinitely while every individual queue operation appears correct.

## SC-MO-CLOCK-001 — wall-clock metadata is not ownership authority

**Statement.** Clock movement or malformed/future timestamps MUST NOT manufacture ownership, freshness, or irreversible priority authority.

## SC-MO-NATIVEADM-001 — use native jobserver semantics where accepted

**Statement.** For GNU/Rust-jobserver-compatible build families, ShareCLI SHOULD attach/propagate the native token protocol rather than emulate its internal parallelism with a second generic queue, unless comparative evidence justifies otherwise.

**Verification.** nested build fixture where total concurrent jobs never exceeds inherited native token budget; token is returned after error/interrupt.

## SC-MO-RESOURCE-001 — resource enforcement is separate from queue ordering

**Statement.** CPU/memory/I/O/resource pressure policy MAY gate admission but MUST report whether it is observation, refusal, throttling, or actual OS enforcement.

**Counterexample.** a sampled pressure warning is reported as a hard resource limit.

## Evidence identity

Every ShareCLI-local admission receipt should include provider/policy version, lane/resource, execution subject, owner/lease generation, priority, acquire/release/reclaim reason, timestamps/monotonic duration source, and verifier evidence where acceptance is asserted.

## Slice exit

Promote only after native SC-F02/03/07 controls execute, owner-death/PID-reuse/restart fixtures land, jobserver integration is prototyped for one eligible build family, and independent review attacks starvation/clock/restart behavior.
