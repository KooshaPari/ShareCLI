# Architecture decision candidate SC-AD-01 — split in-flight coalescing from durable result reuse

Status: **PROVISIONAL ACCEPT — pending expanded native matrix and independent review**  
Date: 2026-09-30.

## Evidence

SC-F01 is natively reproduced on the real Hypervisor: Git mode returned stale first-edit output after relevant file bytes changed.

Current `Hypervisor::run` couples:
1. pre-lock durable cache lookup;
2. advisory-lock duplicate suppression;
3. post-lock cache sharing;
4. TTL result persistence;
5. speculative pre-execution;
6. one cache-key mode selected from Time/Args/Git.

This means “avoid duplicate concurrent work” and “replay a previous result later” currently inherit the same equivalence relation even though their risk/lifetime semantics differ.

## Decision candidate

Replace the conceptual single `CoalesceCache` product contract with two independent capabilities:

### InFlightCoordinator
Purpose: suppress duplicate **currently executing** work under a bounded accepted adapter.

Required identity:
- adapter ID/version;
- normalized invocation;
- adapter-declared relevant inputs available at request time;
- principal/project sharing scope;
- execution generation.

Lifetime ends when the owning execution completes/fails/is abandoned. It does not imply later replay.

### ResultReuseStore
Purpose: optional durable replay for adapters that can prove stronger equivalence.

Requires:
- immutable or sufficiently complete input identity;
- tool/executable/dependency identity;
- result provenance;
- accepted freshness/invalidation policy;
- authorization scope;
- explicit side-effect/determinism eligibility.

Unknown domain → bypass.

## Consequences

- Generic arbitrary-command durable replay is **rejected as the default architecture**.
- Time/Args/Git historical modes may remain compatibility inputs only inside explicitly bounded adapters; their names are not acceptance guarantees.
- Speculation may only populate ResultReuseStore for adapters whose durable-reuse contract permits speculative execution. It cannot use a generic command-frequency histogram to pre-execute arbitrary side-effectful work.
- A tool-native cache should be delegated to when it already has a stronger semantic domain.
- The UI/API must distinguish `shared_in_flight` from `reused_durable_result`.
- Queue/admission is separate again; mutating commands are not made safe merely by routing to a queue.

## Falsification criteria

Reject/revise this decision if:
- a generic equivalence model is demonstrated across the accepted arbitrary-command domain with tractable cost and negative controls;
- the intended product authority explicitly excludes durable replay and only requires in-flight sharing;
- or a tool-native composition makes even first-party InFlightCoordinator unnecessary.

## Required experiments before freeze

1. expanded Time/Args/Git/environment matrix;
2. concurrent duplicate fixture proving one execution/two correct consumers;
3. owner crash while waiter exists;
4. failed execution propagation;
5. same apparent invocation under different principal/project;
6. adapter mismatch/version change;
7. speculation side-effect adversarial fixture.
