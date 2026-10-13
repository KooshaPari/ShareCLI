# ADR 0009 — Global nested transaction contract: ShareCLI local admission

**Status:** Proposed; first pure admission predicates committed, runtime enforcement not wired  
**Date:** 2026-10-12  
**Related:** ADR 0007 (ports/adapters); PhenoFabric ADR 0036; ShareCLI recovery SC-MO-SCHED obligations; research dossier dated 2026-10-08.

## Decision

ShareCLI remains an independently usable local governor. Its local scope is a managed authority domain that may subsequently be adopted into the one *logical*, globally nested transaction tree coordinated across participating domains by PhenoFabric. It does not create an individual journal for each agent, and joining the fabric must not retroactively merge or validate conflicting authority. This does not make Fabric a universal database or transfer domain authority from adjacent products.

Human and SoftwareAgent are both Actor kinds. Actor kind is separate from granted rights, role, scope and effective epoch. Actor types have no implicit bypass. Requests, decisions, dispatches, effect observations and acceptance are distinct attributable records.

Every consequential transaction has one owning parent scope. Its dependencies, shared producers, target resources and evidence may be referenced across branches; this does not imply a second ledger. Parent intent and budget cannot be multiplied by spawning child agents or splitting WorkItems. Scope ancestry must be verified by the owner of the canonical tree, not guessed from a string prefix.

### Local admission and optimization rules

1. Preserve source Intent, WorkItem, Attempt, ConsumerSubscription, Reservation, LogicalEffect and accepted outcome as separate identities across split/share/reorder/migrate/cancel/retry.
2. Split and fusion must preserve the authorized result/effect contract and any temporal, noncommutativity and budget constraints; a new plan revision is not a new authorization.
3. In-flight sharing requires qualified equivalence, current input snapshot, upstream sharing permission and separate output access. A semantic similarity score never authorizes sharing.
4. Subscriber cancellation detaches interest. Producer shutdown must linearize with late joins on its generation. Revocation of the sole valid grant may stop an otherwise wanted producer.
5. Hard reservations require atomic competing-domain admission and checked arithmetic. Parent envelope subdivisions are not independently new physical claims. Expiry is not reclamation.
6. LogicalEffect identity and exact payload survive retry and runtime replays. Unknown outcome requires target-side atomic idempotency, conclusive reconciliation or an explicitly authorized duplicate effect.
7. An accepted parent has a fixed required-child membership/version. Later work is a new revision, not a silent insertion.
8. The agent/human requestor, authorizer, executor and verifier are independently attributed.
9. No external effect may be called merely because a schema-valid event or optimistic plan exists; verify grants at the resource boundary and preserve enforced/observed distinctions.

### Implementation boundary in this branch

The exported pure Rust module at [transaction_contract.rs](../../crates/sharecli-core/src/transaction_contract.rs) adds a small, dependency-free decision layer with unit tests for grant attenuation, reservation conservation, late-join rejection, unknown-outcome retry and sealed parent membership. This is **not** yet a durable local authority database, distributed ledger, grant verifier, OS enforcement, or running process supervisor integration. Callers must serialize competing invocations.

ShareCLI session observations remain a different data source/projection; their compaction is not an immutable global commitment history. PhenoFabric owns remote placement, residency, authority epochs and cross-host handoff. ShareCLI must not double-count delegated capacity, silently accept stale placement proposals, or implement a second global namespace.

## Negative acceptance controls

- Many child agents cannot increase parent entitlement.
- Cancelled consumer A cannot terminate a producer still subscribed by B.
- Expired/stale lease does not free a physical reservation without observed reclamation.
- Same shell command with changed input or upstream no-share rule is not shareable.
- Unknown remote mutation is not retried under a fresh effect key.
- Required child cannot be added after parent acceptance.
- Human identity is not an implicit superuser exception.

## Validation / exit criteria

Run `cargo test -p sharecli-core transaction_contract`, `cargo fmt --all -- --check`, and existing relevant native tests. For product acceptance, integrate the guards into the actual local coordinator with a persistent authority transaction, an effect broker, and native adapter receipts. Run the two-consumer cancellation/restart/migration scenario; do not label the pure guard tests operational guarantees.

**Open:** where canonical scope/epoch identity is minted in standalone mode and how it is safely adopted; how authority revocation propagates offline; which adapter can provide a trustworthy reclamation receipt; whether reusable contract code should graduate into a shared package after two real consumers.
