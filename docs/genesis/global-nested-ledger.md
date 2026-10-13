# Genesis — why a nested global transaction contract?

**Origin:** The user's 2026-10-12 system-level autonomy problem, corrected to **one global recursively nested ledger** and **one Actor parent class** for humans and software agents.

The immediate example is a system-level agent making a context-dependent repair: editing config could be helpful or harmful. Task classification cannot alone settle safety. An effect must be evaluated against enclosing authority, shared resource/side-effect constraints, and current competing commitments. This applies equally to human participants.

**Pre-existing ShareCLI thesis:** Optimize fragmented high-concurrency work by changing the execution shape (sharing, scheduling, coalescing, caching, speculation, pressure response), preserving original intent and accepted results. The global ledger research is an extension of that thesis, not a rebranding into process/session logging.

## Design contradictions to resolve

1. One global logical history must coexist with standalone ShareCLI and multi-domain storage.
2. One tree of containment must represent shared producers and resources whose relationships form cross-branch graphs.
3. Non-rewritable event history must coexist with mutable conclusions, privacy, corrections and recovery.
4. Parent-child rollback semantics cannot pretend externally committed effects are reversible.
5. Human and software agent are the same Actor superclass, yet explicit consent and owner-specific policies remain valid.

**Alternatives retained:** ordinary transactional SQL + event/outbox + enforcing adapters, workflow engines as orchestration only, distributed consensus if justified by fault model, and the weaker after-the-fact queue/event log baseline.

**Source evidence:** [human intent](../intent/2026-10-12-global-nested-ledger.md), [ADR](../adr/0009-global-nested-ledger-local-admission.md), [Fabric counterpart](https://github.com/KooshaPari/PhenoFabric/blob/feat/global-nested-ledger-20261012/docs/adr/0036-global-nested-transaction-ledger.md).
