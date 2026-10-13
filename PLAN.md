# ShareCLI Implementation Plan

> **Agent backlog:** claimable tasks live in [`WORK_DAG.md`](WORK_DAG.md)
> (FR-linked, effort ≤ M). This file is the human roadmap summary only.

## Overview

Build a shared CLI process manager for multi-project agent orchestration.

## Current focus (C03 Agent Readiness)

Lift L30 Agent Readiness toward grade **C** by making FRs machine-readable,
tasks claimable, and agent entrypoints complete. See `WORK_DAG.md` tasks
T-100..T-160 (this wave) and T-200..T-310 (next).

## Phases (historical roadmap)

### Phase 1: Process Management — FR-001, FR-005
- Process spawning and monitoring
- Resource limits (CPU, memory)
- Process lifecycle + signal handling

### Phase 2: Multi-Project Support — FR-002, FR-003
- Project configuration / registry
- Cross-project isolation boundaries
- Shared state via config persistence

### Phase 3: Health & Orchestration — FR-004
- Pool / health status surfaces
- Agent-facing status commands
- Traceability: `docs/specs/FR.md` + `TRACEABILITY.md`

### Phase 4: CLI / DX polish
- Completions, journeys, friction log
- Golden output / visual fixtures
- FR-006+ (workflows, cast promotion)

## How to pick work

1. Open [`WORK_DAG.md`](WORK_DAG.md).
2. Claim a **READY** task whose predecessors are **DONE**.
3. Reference the FR ID in the PR body (enforced by `pr-lint.yml`).

---

## Proposed ledger-integration wave — 2026-10-12

1. **Committed as candidate:** preserve human verbatim intent and reconcile ADR/FR/traceability; pure guard predicates in `sharecli-core`.
2. **CI gate:** compile, fmt, lint and run exact-branch unit tests; repair failures without changing historical accepted identifiers.
3. **Local authority gate:** wire authorized execution admission to durable transactional resource/event/outbox coordination and trusted native reclamation observations.
4. **Sharing gate:** exercise two independently authorized subscribers with cancellation/join races and evidence-linked acceptance.
5. **Fabric gate:** adopt standalone root explicitly, verify epochs at targets, unknown outcome reconciliation and partitions.
6. **Finality gate:** compare safe native outcome goodput and latency against queue+event-log/SQL baselines; cross-link receipts in the acceptance matrix.

[Work breakdown](WORK_DAG.md) · [ADR 0009](docs/adr/0009-global-nested-ledger-local-admission.md). Steps 2–6 are not complete.
