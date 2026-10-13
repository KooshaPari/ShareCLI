# Raw human intent and formal synthesis — global nested ledger

**Source:** user messages in current discussion, 2026-10-12. **Evidence class:** directly supplied conversation text; **not** imported from the historical curation corpus. Exact spelling and punctuation in quotes are preserved.

## Verbatim source

> Let's discuss.
>
> Everything as an Immutable transactional ledger.
>
> 1 example of the problem. System level agents that need autonomy while also having fuzzy lines between good and harmful action/step

> The ledger is not individual but global and nested / tree like

> Humans and agents should be part of the same parent class.

> This research is for ShareCLI and phenofabric.

> Bake into the 2 relevant repos + pheno reg w commits

> All other docs types e.g. Docs intent following our prior standards updated?

## Synthesis — proposed, not a historical quote

The user is researching a **single globally logical, nested/tree-like immutable transaction history** across actors and work. Each consequence belongs to one authoritative containment scope, with explicit references for shared resources and producer work across branches. `Actor` is a common superclass of `Human` and `SoftwareAgent`; neither has default implicit superiority or a separate per-agent ledger. The fuzzy autonomy test is whether a change can be governed by bounded delegation, concurrency-aware commitments, outcome uncertainty and physical enforcement without requiring perfect judgment.

For ShareCLI: conserve local resource commitments across split, coalesce, cancellation, migration and retry. For PhenoFabric: coordinate cross-host placement and authority epochs. For PhenoRegistry: index the decisions and boundaries, not runtime history. A global model is not a demand for a universal physical database.

## Clarifications and non-inferences

- “Everything” remains a research thesis; the user did not specify logging every transient byte or collapsing all ecosystem storage.
- The user did not authorize production deployment, irreversible host operations, or elimination of established product authority.
- Immutable historical claims need not be immutable interpretations of truth; later observations are append-only corrections.
- Local implementation predicates and a SQLite experiment are not proof of distributed ledger correctness.

## Trace

Source → [ADR 0009](../adr/0009-global-nested-ledger-local-admission.md) → [proposed FR-013](../../docs/specs/FR.md) → [verification](../verification/global-nested-ledger.md) → `crates/sharecli-core/src/transaction_contract.rs` → native integration **pending**.
