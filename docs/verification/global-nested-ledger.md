# ShareCLI transaction contract — verification plan

**Trace chain:** [verbatim user intent](../intent/2026-10-12-global-nested-ledger.md) → [ADR 0009](../adr/0009-global-nested-ledger-local-admission.md) → [FR-013 proposal](../specs/FR.md) → [Rust guards](../../crates/sharecli-core/src/transaction_contract.rs) → native/runtime evidence (not yet available).

| ID | Invariant | Existing unit/model evidence | Required product evidence | State |
|---|---|---|---|---|
| SC-TXN-01 | Human/SoftwareAgent share Actor authority rules; child grant attenuated | `grants_are_attenuated_independent_of_actor_kind` | actual AuthN+grant broker, owner/consent negative tests | Unit defined; native pending |
| SC-TXN-02 | No physical budget multiplication; reclaim observed | `reservation_never_multiplies_capacity_or_ignores_physical_reclamation` | durable reservation against concurrent subprocesses, crash/reclaim | Unit defined; native pending |
| SC-TXN-03 | Producer joining/cancelling generation-safe | `producer_late_join_cannot_revive_stopping_generation` | native cancellation/join race across supervisors | Unit defined; native pending |
| SC-TXN-04 | Unknown external effect never blindly repeated | `unknown_effect_is_not_automatically_retryable` | target idempotency/reconciliation after lost response | Unit defined; native pending |
| SC-TXN-05 | Parent required children sealed/versioned | `parent_acceptance_requires_current_complete_membership` | durable concurrent worktree/actor test | Unit defined; native pending |
| SC-TXN-06 | Split/share/migrate preserve original consumer obligations | Not tested in product | two-actor execution with migration and permission changes | Proposed |

Run `cargo test -p sharecli-core transaction_contract`, `cargo fmt --all -- --check` and native acceptance. **No CI success or product enforcement is asserted by this document.** A green unit suite would not satisfy the end-to-end contract.
