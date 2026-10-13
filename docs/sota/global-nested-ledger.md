# SOTA candidates and falsifiable comparisons — 2026-10-12

**Status:** architecture research map, not a claim of exhaustive market discovery or reproduced benchmarks. Evaluation must distinguish candidate discovery from factual validation and measured product suitability.

| Candidate / primary reference | Relevant mechanism | Failure mode / falsifier | ShareCLI decision |
|---|---|---|---|
| [PostgreSQL serializable isolation](https://www.postgresql.org/docs/current/transaction-iso.html) | atomic competing admission in a single DB authority | external effects/OS operations are not in the SQL transaction | credible baseline for competing invariants |
| [SQLite transaction semantics](https://sqlite.org/lang_transaction.html) | local atomic state+event+outbox | wrong actor authority, nonmediated effects, machine failover | start local; no new DB engine |
| [Temporal Activities](https://docs.temporal.io/activity-definition) | durable workflow retries and unknown outcomes | retries duplicate external mutation without target idempotency | optional orchestration, not grant owner |
| [I-confluence research](https://arxiv.org/abs/1402.2237) | determine which invariants actually need coordination | arbitrary disjoint projections mistaken as invariant-independent | coordinate only overlapping rights |
| [Google Chubby](https://research.google.com/archive/chubby-osdi06.pdf) | generation/sequence fencing | stale writer still accepted by target | require target enforcement |
| [Linux Landlock](https://docs.kernel.org/userspace-api/landlock.html) | process-specific effect restriction | policy says “enforced” but adapter has no control | native adapter qualification, platform-specific |
| [Certificate Transparency RFC 9162](https://www.rfc-editor.org/rfc/rfc9162.html) | tamper-evident append-only history | append-only ≠ prevented bad action or ensured truthful event | integrity is a separate layer |

## Required differentiator, if proven

Preserve **global nested commitments across human/agent work transformation**, sharing one qualified producer without erasing distinct subscribers, effective authority or physical accounting, and keep execution useful standalone. No claim of novelty or superiority without baselines.

## Benchmark/negative gates

- Identical input and permissions vs near-identical shell command with different workspace/credentials.
- Two consumers, one cancellation, concurrent late join.
- Parent budget subdivided among 100 agents cannot multiply.
- Stale epoch writer cannot publish at destination.
- Unknown remote effect never auto-reissued under fresh identity.
- Lease expiry does not imply resource reclaim.
- Compare SQL/outbox baseline, runtime overhead, accepted useful goodput, tail latency and RT foreground disruption on actual target hosts.

**What is not established:** complete landscape coverage, distributed high-availability semantics, permission-enforcing native process broker, or numerical performance benefit.
