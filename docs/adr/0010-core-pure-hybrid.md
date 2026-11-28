# ADR 0010 — `sharecli-core-pure` HYBRID: keep `sharecli-core` infra, add a pure re-export crate

**Status:** Accepted (2026-10-18)
**Date:** 2026-10-18
**Deciders:** sharecli maintainers
**Supersedes:** —
**Amends:** [0007-ports-adapters-hexagonal.md](0007-ports-adapters-hexagonal.md)
**Related:**
- [0007-pure-classification.md](0007-pure-classification.md) — the PURE/INFRA item-level evidence
- [../ops/lib-sprawl-plan.md](../ops/lib-sprawl-plan.md) — Phase 4 crate extraction
- [COMPREHENSIVE_AUDIT_SCORECARD.md](../../COMPREHENSIVE_AUDIT_SCORECARD.md) — C00 L2 row, evidence column

---

## Context

ADR-0007 (2026-08-28) declares:

> **1. Domain core is dependency-free**
> `sharecli-core` (Hypervisor, ThermalGate) must **never** depend on Tokio async
> runtime, Axum HTTP framework, Filesystem or network I/O, Any `sharecli-*`
> infrastructure crate. Domain types use plain Rust enums and structs. Async
> wrappers live in application services.

The C00 L2 audit row ("Dependency Direction") scores this invariant **3** and the
evidence column is the contradiction: the actual `crates/sharecli-core/Cargo.toml`
declares

| dep | version | classification |
|-----|---------|----------------|
| `sharecli-ipc`   | path | TIER-C infrastructure (CoalesceCache, slot queue) |
| `sharecli-fleet` | path | TIER-C infrastructure (device registry) |
| `sharecli-fuse`  | path | TIER-C infrastructure (FUSE mount) |
| `tokio`          | 1.53 | TIER-C infrastructure (async runtime) |
| `anyhow`         | 1    | utility |
| `tracing`        | 0.1  | utility |
| `tempfile`       | 3    | utility |

So the ADR says one thing and the code does another. The 2026-10-17 reaudit
flagged this in `docs/audit/sharecli-reaudit-2026-10-17.md:71` as a
"score surface-level, must fix contradiction in C00-L2" finding.

We have three resolution options on the table:

### Option A — Full split (move source of truth)

Move every pure item from `sharecli-core::transaction_contract` into a new
`sharecli-core-pure` crate as the source of truth. `sharecli-core` then adds
`pub use sharecli_core_pure::*` for backwards compat. The new crate would have
genuinely zero transitive infra deps. **Cost:** ~200 lines of `mod.rs` +
`Cargo.toml` churn, plus a coordinated test rerun on every crate that imports
`sharecli_core::transaction_contract`. **Risk:** if the 11 items ever diverge
between the two crates, the embedder of `sharecli-core-pure` sees a different
binary layout than the embedder of `sharecli-core` (a class of bug that Rust's
type system cannot catch because the type names are the same).

### Option B — Pure re-exports, source stays (HYBRID)

Keep all item source code in `sharecli-core::transaction_contract`. Add a new
`sharecli-core-pure` crate whose `Cargo.toml` declares **only** the path
dependency `sharecli-core`, and whose `src/lib.rs` does
`pub use sharecli_core::transaction_contract::{…11 items…}`. The embedder
gets a single, versioned entry point into the pure surface; the cargo-tree
transitive closure still includes sharecli-ipc/fleet/fuse/tokio (because
sharecli-core has them), but the **declared** deps of the new crate are clean.
**Cost:** ~3 small commits, no `sharecli-core` source churn, no
backwards-compat risk. **Risk:** the new crate is only "pure" at the declared
level — a transitive dep analysis would still see infrastructure crates. This
limits the value to embedders who want a clear Cargo boundary, not those who
want to compile sharecli types with `--no-default-features` or similar
trimming.

### Option C — Deny the contradiction (status quo)

Edit ADR-0007 to delete the "domain core is dependency-free" claim and replace
it with a description of the actual state. Bump the scorecard evidence
column. **Cost:** no code changes. **Risk:** the L0–L4 audit framework
explicitly rewards hexagonal layering; lowering the aspiration in writing
without changing code regresses the rubric.

## Decision

**Option B — HYBRID.** This is the "boring, useful, defers the question" choice.
It costs three small commits, gives embedders a single explicit entry point
to the pure surface, and leaves the door open to a future Option A "deep
split" without breaking anyone. The libstd/libcore analogy applies:
`sharecli-core` is like `libstd` (full-featured with infra), `sharecli-core-pure`
is like `libcore` (a curated subset). A future Option A would promote
`sharecli-core-pure` to a fully standalone crate; until then, it is a view
into `sharecli-core`.

### What lands in this amendment

1. `crates/sharecli-core-pure/Cargo.toml` — declares one path dep:
   `sharecli-core`. No tokio, no `sharecli-ffi`, no `sharecli-ipc`,
   no `sharecli-fuse`, no `sharecli-storage`, no `sharecli-sync`,
   no `sharecli-mesh`, no `sharecli-session`, no `anyhow`/`tracing`/`tempfile`.
2. `crates/sharecli-core-pure/src/lib.rs` — re-exports the 11 pure items
   (the full `sharecli_core::transaction_contract` surface).
3. `crates/sharecli-core-pure/examples/re_export_link_check.rs` — compile-time
   type-equality witness. If anyone ever renames a `transaction_contract`
   item, the example breaks at `cargo check -p sharecli-core-pure --examples`.
4. `[workspace.members]` in the root `Cargo.toml` gets
   `"crates/sharecli-core-pure"`.
5. `docs/adr/0007-pure-classification.md` — the PURE/INFRA evidence.
6. `docs/adr/0007-ports-adapters-hexagonal.md` — Status header changes to
   "Accepted (Amended 2026-10-18)" with a short pointer to this ADR.
7. `COMPREHENSIVE_AUDIT_SCORECARD.md` — C00 L2 row evidence column expands
   to cite `sharecli-core-pure` + ADR-0010.

### What does NOT land (deferred)

- Moving the source of truth to `sharecli-core-pure` (Option A). The HYBRID
  makes this possible without urgency.
- Splitting `sharecli-core` into `sharecli-core` (infra) +
  `sharecli-core-types` (just the 11 types) + `sharecli-core-runtime` (the
  Hypervisor, etc.). A future 3-way split could happen if Option A ever lands.
- Adding a CI check that fails when `sharecli-core-pure`'s `Cargo.toml` adds
  an infra dep. The boundary contract is currently manual; codifying it is
  a follow-up.

## Consequences

### Positive

- Embedders who want a Cargo-level marker for "pure surface" can
  `use sharecli_core_pure;` instead of reaching into
  `sharecli_core::transaction_contract`. The new crate is self-documenting.
- The C00 L2 audit row's evidence column gets concrete (a real crate +
  example) rather than aspirational.
- The path to a future Option A is open: lift the 11 items out of
  `sharecli-core::transaction_contract` and into
  `sharecli-core-pure/src/transaction_contract.rs`, change
  `sharecli-core`'s `mod transaction_contract` to
  `pub use sharecli_core_pure::transaction_contract::*;`, done.

### Negative

- The transitive closure of `sharecli-core-pure` is NOT clean. A user who
  audits `cargo tree -p sharecli-core-pure` still sees `sharecli-ipc`,
  `sharecli-fleet`, `sharecli-fuse`, `tokio`, etc. The boundary is at the
  declared-deps level, not the transitive level. This is a known and
  documented limitation; see the boundary check in the crate's `lib.rs`
  doc-comment.
- We now have two places to think about when adding a pure item: the
  source in `sharecli-core`, and the re-export list in
  `sharecli-core-pure`. A CI check that scans both is recommended but out
  of scope here.

### Neutral

- The binary size of anything that depends on `sharecli-core-pure` is
  unchanged (because it transitively depends on `sharecli-core`).
- Existing callers of `sharecli_core::transaction_contract::*` see no
  diff. The amendment is additive.

## References

- ADR 0007 (the original, being amended): [0007-ports-adapters-hexagonal.md](0007-ports-adapters-hexagonal.md)
- Classification evidence: [0007-pure-classification.md](0007-pure-classification.md)
- 2026-10-17 reaudit: [../../audit/sharecli-reaudit-2026-10-17.md](../../audit/sharecli-reaudit-2026-10-17.md)
- Scorecard: [../../COMPREHENSIVE_AUDIT_SCORECARD.md](../../COMPREHENSIVE_AUDIT_SCORECARD.md)
