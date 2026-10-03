# ShareCLI decision candidate SC-AD-05 — local durable coordination, not distributed agent orchestration

Status: **PROVISIONAL ACCEPT / direct supersession check remains open**  
Date: 2026-09-30.

## Evidence

Current source contains:
- `sharecli-mesh` Maildir task queue, SmartMerger and WorktreePool;
- session persistence/recovery and terminal-surface capability work;
- historical February “agent mesh” research describing much broader consensus/escalation/autonomy concepts;
- explicit boundary comments in `sharecli-mesh`: Maildir execution substrate is ShareCLI-owned while control-plane orchestration remains in `thegent`;
- ADR-0002 excludes agent-evaluation product scope;
- session research explicitly says NATS is unnecessary for a single-host control plane.

These sources are not equal authority. The broad historical mesh research is prior/proposal material; current boundary and product thesis support local OS-adjacent coordination.

## Decision

ShareCLI's current mature spine includes **local durable coordination primitives**, not a general distributed multi-agent orchestration/consensus platform.

Retain mature concepts where independently justified:
- durable task/work coordination;
- generation-safe leases;
- worktree/merge coordination;
- session/surface observations;
- bounded recovery/adoption;
- local machine interfaces.

Do not infer mature requirements for:
- autonomous multi-agent consensus;
- confidence-weighted escalation;
- distributed membership across arbitrary remote hosts;
- BFT/blackboard orchestration;
- agent planning/task decomposition;
- model evaluation.

Existing code named `mesh` must trace to a local coordination obligation or remain optional/supporting/transition scope.

## Session recovery disposition

Session recovery is a mature local coordination capability candidate because it has:
- durable SQLite ledger/migrations;
- explicit evidence grades;
- dry-run-first recovery;
- shell-free argv launch;
- independent surface capabilities;
- degraded provider semantics.

It must still satisfy the v1.1 OwnershipClaim/ProcessGeneration model. A saved PID/session recipe cannot authorize unattended control after identity becomes ambiguous.

## Growth boundary

Future remote/distributed coordination requires a separate accepted architecture decision covering:
- Host/Principal trust;
- distributed lease/consensus semantics;
- partition behavior;
- remote credential/transport security;
- durable evidence authority;
- ownership boundaries with thegent/other orchestration products.

No current mesh implementation silently crosses this boundary.

## Counterexamples

- a local Maildir task queue is described as distributed agent orchestration;
- stale session recipe resumes a different process generation;
- historical consensus research becomes mandatory because it lives under recovery artifacts;
- local worktree merge capability forces remote membership protocol into CVP;
- missing NATS/etcd/CRDT is scored as ShareCLI product incompleteness.

## Gate consequence

Mesh/session no longer blocks product identity as an undefined feature bucket. Detailed local coordination/session obligations and public-path evidence remain open.
