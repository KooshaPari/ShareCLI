# ShareCLI — pass 2B: trace pollution and acceptance authority

Date: 2026-09-29. Supplements PASS-2-VALIDATION.

## Commit-history evidence

Search of FR-008 commit history shows two materially different classes:

1. **Semantically relevant implementation lineage**, including:
   - `68516581243fe34de427cd21166e4ab5d2db9a8f` TTL/debounce;
   - `39fe5bf1262e069b6bd74c0a304f8588b8f4253f` cwd/env key-isolation test;
   - `a8e0d4ba674c58a9548bd1f38178b3cd8d18ebd3` queue priority;
   - `db82f1b18705bd28370626b76e4ce5606b69f0a9` cache-key modes/semantic/nocache port;
   - `19e5ecaee39ebda307755d5c55f1d41090df29a2` speculative execution.

2. **Unrelated changes carrying FR-008 labels**, including later CI/docs/codesign/HTTPS changes. This demonstrates that a textual FR tag in a commit is not sufficient proof of semantic trace membership.

Scorecard history also contains repeated Forge Bot co-authored governance synchronization commits. These are useful historical records but not independent acceptance observations.

## Trace policy correction

For this program an implementation/test/commit edge to FR-008 is valid only if its subject relation is explicit and reviewed:
- implements behavior;
- verifies behavior;
- documents accepted behavior;
- migrates behavior;
- supersedes behavior;
- or supplies evidence for a named criterion.

A commit message, comment or test annotation containing `FR-008` without a defensible subject relation is **tag coincidence / auxiliary metadata**, not bidirectional traceability.

This means the prior repository statement “all 12 FRs have source location mapped, test files present, doc references linked, therefore full bidirectional traceability is established” is not accepted as a mature-recovery conclusion. Structural links are necessary but insufficient.

## Acceptance-authority question

The July cache-key evolution is implementation/design history. The mature program still needs to recover who authorized the stronger product claim that arbitrary/recovered harness invocations may be replayed from a durable cache, and under what equivalence assumptions. Historical harness behavior is not self-authorizing if its semantics produce incorrect output.

Until that authority and semantic domain are recovered, the safe projection is adapter-bounded sharing with bypass for unknown equivalence—not “repair Git mode until every command can be cached.”

## Gate delta

Traceability structural-validity gate remains NOT_VALIDATED. A new trace validator should eventually reject relation-less FR tags and distinguish author assertion from executed evidence, but implementation of that validator is deferred until the ontology/authority model is accepted.
