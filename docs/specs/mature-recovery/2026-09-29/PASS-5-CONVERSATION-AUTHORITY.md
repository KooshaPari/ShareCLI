# ShareCLI — pass 5 conversation authority: bounded optimization, not universal interception

Date: 2026-09-30.
Status: authority recovery; does not by itself freeze implementation.

## Direct user-authored context recovered

### 2026-08-09 — scaling/duplicate-work problem
User context targeted agent-harness scale of roughly 64 agents × 8 subagents, with duplicate builds/work, queue/coalesce of identical Rust/Cargo work, daemon/Unix-socket reuse, and pooled process/network resources.

**Authority consequence:** avoiding duplicated work and coordinating contention are genuine product outcomes, not merely a July implementation artifact.

### 2026-08-10 — terminal semantics and bounded optimization
The user explicitly required that agents retain terminal access and asked to optimize unknown filesystem/other calls rather than remove terminal semantics.

The user also rejected a parser-rewrite framing as nonsensical in the current form and questioned the overhead/value of two parsers versus direct calls and real Zsh.

**Authority consequence:** ShareCLI is not authorized to become a pseudo-shell or require arbitrary commands to be rewritten into a bespoke command language merely to optimize them.

### 2026-08-27/28 — standalone wedge and adaptive granularity
User context says ShareCLI should remain a standalone adoption wedge inside the broader runtime/agent ecosystem rather than absorb the distributed fabric. Distribution/sharding may be atomic in principle but should normally be coarsened because distribution has cost; adaptive fusion/fission, caching/pinning/prefetching are preferred to literal syscall RPC as a default.

**Authority consequence:** ShareCLI should expose bounded local/runtime capabilities and optional fabric integration, not grow into the full distributed scheduler/fabric.

### 2026-04-01 — lifecycle/observability roots
Earlier user context described many CLI agents tied to isolated worktrees, central observability/control, bounded task/acceptance contracts, and lifecycle management.

**Authority consequence:** observation/control/workspace isolation are genuine mature outcomes. This does not authorize every later FUSE/cache/mesh mechanism.

## Authority impact on current architecture candidates

### SC-AD-01 — split in-flight coordination from durable replay
**Strengthened.**

The user-authored duplicate-work problem clearly supports in-flight/duplicate suppression as a product outcome. Nothing recovered requires generic durable replay for arbitrary commands. SC-F01 demonstrates that the current coupled cache can extend a useful concurrent equivalence relation beyond its justified lifetime.

### SC-AD-02 — explicit EquivalenceAdapters with fail-to-bypass
**Strengthened with direct intent support.**

The recovered user preference for bounded/tool-specific mechanisms, direct APIs/adapters where recognized, and real shell fallback for arbitrary shell work is consistent with:

`known accepted adapter → optimize`
`unknown semantic domain → execute normally`

This is stronger authority than a purely assistant-invented adapter model. The exact interface/name remains a design decision, not a quoted user requirement.

### Universal transparent interception
**Not supported by recovered user intent.**

The user asked to optimize unknown filesystem/other calls while preserving terminal semantics, but this is not equivalent to authorizing unsafe universal command-result replay. Observation, FUSE/filesystem acceleration, command mediation, and durable reuse remain separate capability dimensions.

## Mature-boundary correction

ShareCLI mature identity is provisionally:

> a standalone OS-adjacent agent workload runtime that observes heterogeneous CLI agents, manages authorized workloads/constraints, reduces redundant work where an explicit semantic mechanism makes that safe, coordinates isolated workspaces/resources, and preserves normal terminal/tool behavior outside optimized domains.

It is **not** provisionally:
- a replacement shell/parser;
- the whole distributed fabric;
- an arbitrary-command result cache;
- an agent benchmark/evaluation product.

## Remaining conversation authority gaps

- original February agent-harness decisions and exact accepted FUSE/mesh outcomes;
- whether durable reuse was explicitly requested as a user-facing capability or only inherited from the historical harness;
- required versus optional filesystem interception;
- accepted cross-platform scope for mediation.
