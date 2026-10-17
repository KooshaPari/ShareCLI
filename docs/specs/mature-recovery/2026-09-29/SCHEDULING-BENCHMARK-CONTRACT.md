# ShareCLI scheduling benchmark contract — v0.1

Date: 2026-09-30.
Status: **required before SC-WP-B05 can move beyond reference model**.

## Goal

Measure whether ShareCLI improves useful workload completion under constrained resources versus realistic baselines, without violating correctness or hard resource limits.

## Fixture

A deterministic synthetic-but-representative workload must contain:
- >= 8 WorkItems;
- CPU and memory demands with at least two sizes;
- at least one dependency chain;
- at least one independent branch;
- at least one unknown-demand item;
- at least one duplicate/equivalent pair reserved for the later coalescing subtest;
- optional duration estimates with provenance;
- one ResourceEnvelope deliberately too small for naive all-at-once execution.

The first fixture must be runnable on ordinary CI without privileged resource controls. A second native/host-pressure fixture may use real processes and OS telemetry.

## Baselines

### B0 naive all-at-once
Launch all ready work without envelope-aware coordination.

Purpose:
- expose contention/oversubscription behavior;
- establish elapsed/peak pressure/wasted work baseline.

### B1 bounded FIFO
Respect hard envelope and queue overflow in arrival order.

Purpose:
- correctness/control baseline;
- distinguish value of smarter packing/backfill from merely limiting concurrency.

Candidate strategies are compared against B0 and B1.

## Required metrics

Correctness/hard gates:
- all required WorkItems produce correct result;
- no accepted semantic duplicate is merged incorrectly;
- no hard ResourceEnvelope violation by the scheduler model;
- no leaked allocations;
- dependency ordering respected.

Performance/quality vector:
- makespan;
- useful-work throughput;
- queue wait distribution;
- deadline/tardiness where fixture includes deadlines;
- peak CPU/memory requested and observed where measurable;
- scheduler decision overhead;
- idle capacity/fragmentation;
- fairness/starvation indicators;
- number of launches;
- duplicate work avoided;
- speculative work/waste where applicable.

## Evidence identity

Receipt binds:
- workload fixture/version/digest;
- scheduler strategy/version;
- policy revision;
- envelope/configuration;
- candidate SHA;
- runtime/OS/hardware where native;
- raw event timeline;
- metric computation version.

## Acceptance

B05 reference correctness can pass independently.

B05 product-value claim requires:
- same workload/environment across baselines/candidate;
- no correctness/hard-limit regression;
- at least one material improvement in intended objective;
- no hidden unacceptable regression in another critical dimension;
- repeated runs where timing noise matters.

No fixed percentage improvement is invented in advance.

## Later extensions

- backfill with duration estimates;
- discrete GPU/VRAM/device constraints;
- deadlines/priorities;
- pressure-driven replanning;
- equivalence/coalescing;
- speculation;
- native jobserver composition;
- real agent workload replay.
