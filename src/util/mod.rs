//! Parity / expansion utilities (`sharecli::util` + library helpers).
//!
//! C00 L1 Phase 1 facade — see `docs/ops/lib-sprawl-plan.md`.
//! Historical file paths under `src/` preserved via `#[path = "../…"]`.

#[path = "../ansi.rs"]
pub mod ansi;
#[path = "../api.rs"]
pub mod api;
#[path = "../backoff.rs"]
pub mod backoff;
#[path = "../binary_search.rs"]
pub mod binary_search;
#[path = "../cache.rs"]
pub mod cache;
#[path = "../color.rs"]
pub mod color;
#[path = "../deque.rs"]
pub mod deque;
#[path = "../graph.rs"]
pub mod graph;
#[path = "../hash_util.rs"]
pub mod hash_util;
#[path = "../lazy.rs"]
pub mod lazy;
#[path = "../retry.rs"]
pub mod retry;
#[path = "../slice_ext.rs"]
pub mod slice_ext;
#[path = "../uuid.rs"]
pub mod uuid;
#[path = "../rate_limiter.rs"]
pub mod rate_limiter;
