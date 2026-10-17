# ShareCLI quality and benchmark contract — v1.0

Date: 2026-10-01.

## Purpose

Define how quality targets are established without inventing numbers.

## Benchmark subject identity

Every result binds:
- workload fixture/version;
- WorkGraph and adapter identities;
- machine/hardware/OS;
- background load;
- ResourceEnvelope;
- SchedulingPolicy/strategy/version;
- ShareCLI candidate/dependencies;
- start/end timestamps;
- raw resource/execution traces.

## Required baselines

For any optimization claim compare on the same fixture/environment:
1. naive/status-quo execution;
2. bounded FIFO/control;
3. candidate strategy;
4. relevant native alternative where available.

## Metrics

Correctness/hard gates:
- work result correctness;
- no unauthorized equivalence/share;
- no hard resource-envelope violation unless oversubscription explicitly allowed;
- no leaked durable ownership/lease;
- no false green.

Optimization dimensions:
- useful work completed/time;
- makespan;
- deadline miss count/tardiness where applicable;
- peak/area CPU, RAM, GPU/VRAM, disk/I/O pressure;
- queue wait distribution;
- fairness/starvation;
- scheduler overhead;
- retries/cancellations;
- duplicate work avoided;
- cache hit correctness;
- speculative work/waste;
- recovery time after worker/controller failure.

## Target-setting rule

No numeric SLO becomes normative until one of:
- direct user objective;
- empirical baseline + accepted improvement target;
- external interoperability/standard constraint;
- safety/operability threshold.

Historical numbers without workload/environment are hypotheses only.

## Statistical discipline

- repeated trials where nondeterminism matters;
- report distribution/variance, not only best run;
- identical fixture and comparable warm/cold cache state;
- separate correctness failures from performance;
- no cross-machine comparison without normalization/qualification.

## Acceptance

A strategy can be accepted for a workload family without being globally preferred. SchedulerStrategy selection remains policy/adapter-specific.
