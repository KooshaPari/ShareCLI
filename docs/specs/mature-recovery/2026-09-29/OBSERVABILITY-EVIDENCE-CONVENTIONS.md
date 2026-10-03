# ShareCLI observability/evidence conventions — v1.0

Date: 2026-10-01.

## Standards posture

Use OpenTelemetry-compatible process/system/hardware metric semantics where practical rather than inventing incompatible names/units.

Minimum benchmark observations map conceptually to:
- process CPU time/utilization;
- process memory usage/virtual memory;
- process disk/network I/O;
- system CPU utilization;
- system memory usage;
- system disk I/O/time;
- process counts/limits;
- hardware/GPU/temperature/power metrics where collectors support them.

## Qualification

OpenTelemetry semantic-convention stability varies. ShareCLI records:
- collector;
- collector/version;
- semantic convention/version where applicable;
- native raw source;
- units;
- sampling interval;
- freshness.

A standard metric name does not make the collector authoritative or accurate.

## Scheduler evidence

SchedulingReceipt may reference standardized telemetry but additionally requires product identities:
WorkItem, ResourceEnvelope, Policy, SchedulePlan, Placement, ExecutionAttempt and outcome.

## Cross-platform rule

Where OS metrics differ semantically, preserve platform/native meaning and normalize only through an explicit qualified derived metric. Do not compare superficially identical load values across OSes as if equivalent.

## Finality

Metric vocabulary is final enough for benchmark implementation. Numeric thresholds remain empirical.
