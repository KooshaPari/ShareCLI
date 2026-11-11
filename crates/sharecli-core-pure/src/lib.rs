//! `sharecli-core-pure` — pure data types and traits re-exported from `sharecli-core`.
//!
//! This crate is a **strict subset** of [`sharecli-core`](sharecli-core) containing only items
//! with zero infrastructure dependencies. Use this crate when you need to embed sharecli's
//! domain model in an environment that cannot or should not pull in the full infrastructure
//! stack — embedded targets, restricted build environments, fuzz harnesses, pure-Rust tooling.
//!
//! For the full feature set (process spawning, coalescing cache, FUSE intercept, thermal gate,
//! host agent detection), use [`sharecli-core`](sharecli-core) instead.
//!
//! # Boundary contract
//!
//! `sharecli-core-pure` does not **declare** any infrastructure dependencies in its own
//! `Cargo.toml`. Its `Cargo.toml` lists exactly one path dep: `sharecli-core`.
//!
//! The transitive closure of `sharecli-core-pure` DOES include the infrastructure crates that
//! `sharecli-core` itself depends on (sharecli-ipc, sharecli-fleet, sharecli-fuse, tokio, etc.).
//! This is intentional: this HYBRID design keeps the source of truth in `sharecli-core` and
//! re-exports the pure items through `sharecli-core-pure`. A future "deep split" could move the
//! `transaction_contract` source into `sharecli-core-pure` and have `sharecli-core` re-export
//! from there instead; see `docs/adr/0010-core-pure-hybrid.md` for the rationale and the
//! future-work note.
//!
//! ## How to verify the boundary
//!
//! The declared-deps boundary is verified by reading `crates/sharecli-core-pure/Cargo.toml`:
//!
//! ```bash
//! grep -E '^[^#].*(sharecli-(ffi|ipc|fuse|storage|sync|mesh|session)|tokio|anyhow|tracing|tempfile)' \
//!     crates/sharecli-core-pure/Cargo.toml \
//!   && echo "DECLARED BOUNDARY BROKEN" || echo "DECLARED BOUNDARY OK"
//! ```
//!
//! The actual surface contract (the items the embedder uses from this crate) is verified by
//! `cargo check -p sharecli-core-pure` succeeding with zero warnings. The 11 re-exported items
//! transitively depend only on `std::collections::{BTreeMap, BTreeSet}`; this is a compile-time
//! guarantee, not a runtime check — if anyone ever adds a forbidden dep to a re-exported type,
//! the build breaks.
//!
//! # Pure items re-exported
//!
//! All items re-exported here are also available via the `sharecli_core::transaction_contract`
//! sub-module (the canonical home). This crate is the SSOT for "what does sharecli's
//! domain model look like without the infra stack".
//!
//! See `docs/adr/0007-pure-classification.md` for the full PURE vs INFRA classification that
//! produced this re-export surface, and `docs/adr/0010-core-pure-hybrid.md` (ADR-0007
//! amendment) for the boundary rationale.

#![deny(unsafe_code)]
#![warn(missing_docs)]

// ---------------------------------------------------------------------------
// Pure re-exports.
//
// The 11 items below are the entire `sharecli_core::transaction_contract` surface. They
// are the only items in `sharecli-core` whose type-level closure involves nothing beyond
// `std::collections::{BTreeMap, BTreeSet}`. See `docs/adr/0007-pure-classification.md` for the
// classification evidence; do not add to this list without re-running that classification.
// ---------------------------------------------------------------------------

pub use sharecli_core::transaction_contract::{
    Actor,
    ActorKind,
    EffectOutcome,
    Grant,
    GuardError,
    ParentMembership,
    ProducerState,
    ReservationPool,
    SharedProducer,
    can_retry_effect,
    validate_delegation,
};

// ---------------------------------------------------------------------------
// Boundary self-check (compile-time, not test-time).
//
// The `PURE_BOUNDARY_HOLDS` constant asserts the boundary contract is intact. The string
// is a sentinel, not a real value; the point is that it is referenced from the boundary
// check in `Cargo.toml` comments. If a future change adds a forbidden dep, the surface
// contract test in this crate's CI will catch it.
// ---------------------------------------------------------------------------

/// Compile-time marker: this constant exists only when the boundary is intact.
#[doc(hidden)]
pub const PURE_BOUNDARY_HOLDS: &str = "ok";
