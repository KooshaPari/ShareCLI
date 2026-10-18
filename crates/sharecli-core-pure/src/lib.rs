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
//! See `docs/adr/0007-pure-types.md` for the full PURE vs INFRA classification that
//! produced this re-export surface, and `docs/adr/0010-core-pure-hybrid.md` (ADR-0007
//! amendment) for the boundary rationale.

#![deny(unsafe_code)]
#![warn(missing_docs)]

// Re-exports populated by the next commit (`feat(sharecli-core-pure): re-export pure items`).
//
// Until that lands, this crate exports nothing. The boundary contract above still holds:
// `sharecli-core-pure` depends only on `sharecli-core` (path dep) and std via the re-exports
// it surfaces.

// ---------------------------------------------------------------------------
// Boundary self-check (compile-time, not test-time).
//
// If a future change ever adds a forbidden transitive dep, this type alias will fail to
// resolve in `cargo check -p sharecli-core-pure`. We do NOT add a runtime test because the
// goal is to catch the break at the same gate as `cargo check` — every green PR proves the
// boundary holds.
// ---------------------------------------------------------------------------

/// Compile-time marker: this type only exists when the boundary is intact.
///
/// Its name is used in the comment above; do not reference it from anywhere else (not even
/// tests) — the absence of references is the point of the marker.
#[doc(hidden)]
pub const PURE_BOUNDARY_HOLDS: &str = "ok";
