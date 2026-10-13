# ShareCLI traceability

The maintained canonical mapping is [FR to code and tests](../specs/TRACEABILITY.md), with [functional requirements](../specs/FR.md) and [test coverage history](../../TEST_COVERAGE_MATRIX.md).

## Proposed FR-013: global nested transactions

[Direct user intent](../intent/2026-10-12-global-nested-ledger.md) → [ADR 0009](../adr/0009-global-nested-ledger-local-admission.md) → [FR-013](../specs/FR.md) → [Rust guard module](../../crates/sharecli-core/src/transaction_contract.rs) → [verification plan](../verification/global-nested-ledger.md).

**Evidence status:** Pure guard implementation is committed, but the native supervisor, effect broker, physical reclaim and cross-host acceptance have not been verified. Historical coverage figures must not be represented as new branch results.
