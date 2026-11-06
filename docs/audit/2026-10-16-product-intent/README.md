# ShareCLI product intent, usability and finish plan

Observation: 2026-10-16 Pacific. Source: `e182151d4183c964547c017a4cc3f606a7cfb8ae`.
This is a bounded engineering assessment, not release acceptance or a replacement for the canonical PRD, FRs, ADRs or existing owner lane.

**Current conclusion: substantial implementation, incomplete user journey, no current proof of the requested economic/resource benefit.** The library can execute and coalesce work; the CLI's pooled `run` path still returns a formatted success without executing the requested work. Automatic remote placement is not established. Signed release acceptance and a real-machine savings report remain open.

| Read next                                   | Purpose                                                                        |
| ------------------------------------------- | ------------------------------------------------------------------------------ |
| [Current usability](STATE.md)               | What an operator can reasonably expect, concrete blockers and evidence limits  |
| [Goals and dispositions](GOALS.md)          | FR/epic mapping, implemented versus partial/stub/proposed/extracted behavior   |
| [Finish plan and estimates](FINISH-PLAN.md) | Value-first dependency graph, bounded work packages, engineering effort ranges |
| [Evidence and decisions](EVIDENCE.md)       | Reproducible source anchors, hosted observations and unresolved feedback       |

## Product promise and scope

ShareCLI discovers existing coding agents without replacing their executables, observes resource pressure, safely avoids redundant work and protects host responsiveness. It supports operator lifecycle/configuration/health interfaces and a shared execution substrate. The latest recovered direction adds automatic placement of heavy work on a stronger host, process-spam defragmentation, a useful reports-only optimization experience and honest projected/realized savings.

The delivery test is one installed supervisor handling adversarial real workloads under contention while preserving task semantics and demonstrating meaningful benefit. Merely listing processes, publishing a scorecard, raising filtered coverage or rendering a dashboard does not satisfy that test.

The accepted source contract remains [PRD](../../../PRD.md), [FR index](../../../FUNCTIONAL_REQUIREMENTS.md) and [product finality](../../deploy/FINALITY.md). Optional FUSE is preserved; global nested transactions are proposed and need an explicit integration decision. Agent-evaluation pipelines remain external under the accepted ADRs.

## What the estimates mean

| Horizon                                | Proposed evidence-backed finish                                                                                                                                               | Remaining engineering effort    |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- |
| Core value proposition (CVP)           | Safe real work, bounded contention, automatic heavy-work placement on the selected host, truthful reports-only optimization and measured benefit on a supported pair of hosts | 34-72 engineer-days             |
| Minimum usable product (MVP)           | CVP plus the existing four-host install/serve/tray/dashboard floor and independent release/assurance evidence                                                                 | 43-92 engineer-days cumulative  |
| Full currently mapped maturity horizon | MVP plus optional native FUSE qualification, accepted nested-ledger integration and coordination parity where retained                                                        | 61-137 engineer-days cumulative |

These are planning ranges for the work packages below, not calendar promises or claims that the current source is a particular percentage complete. An engineer-day means approximately eight focused engineering hours including review and acceptance work. No historical throughput calibration is available. Confidence is low overall, medium for the directly observed local repairs. Reuse of accepted existing PRs is assumed; rewriting them would increase effort. External reviews and waiting time are excluded. Parallelism changes elapsed time, not the sum of engineering effort.

CVP is a sequencing slice, not permission to delete the wider backlog or waive the four-host MVP floor. The latest full acceptance still requires green deployment, semantic completion of retained capabilities, a signed artifact and a real-number dogfood report. No criterion is silently sunset by this document.

## Evaluation boundary

No code changes, builds, test reruns, service changes or captures were performed. Existing source/tests and hosted conclusions were inspected. A local executable exists, but its SHA/version was not bound to source or exercised. No currently installed product journey was qualified. Work-package estimates should be revised after the first real execution and two-host receipts reveal the actual integration cost.
