# Architecture decision candidate SC-AD-02 — equivalence adapters are explicit and fail to bypass

Status: **PROVISIONAL ACCEPT**  
Date: 2026-09-30.

## Context

SC-F01 is natively reproduced on the real Hypervisor. The broader Time/Args/Git/environment matrix is executing separately and is not pre-judged here.

Historical cache-key modes encode mechanism-level heuristics. They do not state which command families, inputs, side effects, principals, tool versions or external dependencies make result sharing correct.

## Decision

Introduce a product-level `EquivalenceAdapter` concept. Durable result reuse and in-flight sharing must name the adapter that established eligibility.

Conceptual contract:

`classify(invocation, execution_context) -> {BYPASS | IN_FLIGHT_ONLY(identity) | DURABLE(identity, policy)}`

An adapter identity includes its own version. Its output binds the exact dimensions it claims relevant and records any dimensions intentionally excluded.

### Default adapter

Unknown command/domain → `BYPASS`.

There is no generic fallback from “cannot identify inputs” to Time/Args/Git durable caching.

### Tool-native adapter

If an established tool already owns a stronger cache/equivalence model, ShareCLI delegates rather than storing a second result cache. ShareCLI may still coordinate admission/observation around it.

### Bounded first-party adapter

Only justified for a command family whose relevant inputs/side effects can be specified and adversarially tested.

## Acceptance invariants

- adapter/version participates in evidence and reuse identity;
- principal/project sharing scope is explicit;
- side-effectful/nondeterministic commands cannot be made durable by heuristic argv filtering alone;
- a failed/unavailable fingerprint yields BYPASS;
- adapter upgrade cannot silently trust entries from an incompatible adapter version;
- UI distinguishes bypass, in-flight sharing, durable reuse and tool-native delegation;
- speculation requires explicit durable+speculative eligibility.

## First prototype recommendation

Do **not** begin with shell commands. Begin with a narrow deterministic read/build family where input identity is already available or can be delegated to the tool's own cache. The experiment compares:
1. status quo/tool-native cache;
2. ShareCLI admission + tool-native cache;
3. ShareCLI first-party adapter only if it adds measurable value.

This decision intentionally permits the result that ShareCLI should own no durable cache for that tool.
