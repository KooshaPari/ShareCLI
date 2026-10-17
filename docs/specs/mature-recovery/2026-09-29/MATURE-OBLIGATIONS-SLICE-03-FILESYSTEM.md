# ShareCLI mature obligations — semantic slice 03: filesystem interception

Status: **PROVISIONAL SEMANTIC SLICE / NOT FULL CONTRACT**  
Date: 2026-09-30.

## SC-MO-FS-001 — interception policy is explicit

Every operation that may use filesystem interception MUST carry an accepted mode: off, optional, or required.

## SC-MO-FS-002 — required interception fails closed

When a journey's correctness depends on interception/isolation, failure to establish and verify the interception layer MUST prevent the dependent operation from executing.

**Counterexample:** CoW-required work silently runs against the backing workspace because FUSE mount failed.

## SC-MO-FS-003 — optional degradation is truthful

Optional interception MAY fall back to the backing path, but product/UI/API/evidence MUST identify the degraded result and MUST NOT claim intercepted provenance.

## SC-MO-FS-004 — mount readiness is evidence-bound

A created directory or spawned mount thread is not sufficient. Readiness evidence MUST bind to the exact mount/session used by the operation.

## SC-MO-FS-005 — interception and work equivalence are independent

A FUSE session does not make two command executions semantically equivalent, and a valid equivalence adapter does not imply filesystem interception.

## SC-MO-FS-006 — lifecycle cleanup is bounded and observable

Normal completion, error, cancellation, worker/process death and restart MUST have defined mount cleanup/recovery behavior; cleanup failure must remain observable rather than green.

## SC-MO-FS-007 — platform capability is explicit

Unsupported or privilege-constrained platforms/configurations MUST report capability state. Platform absence must not be inferred from product-wide failure when unaffected journeys remain valid.

## SC-MO-FS-008 — provenance is exact

Where FUSE/CoW provenance is used, the evidence MUST include session identity, backing/mount identity, operation/execution identity, adapter/version and whether fallback occurred.

## Exit

Promote after:
- native optional-failure receipt is retained;
- required-mode negative oracle is implemented against a candidate mode contract;
- one actual FUSE/CoW-dependent journey is identified or FUSE is demoted further;
- restart/cleanup behavior is attacked;
- independent review compares worktrees/native filesystem alternatives.
