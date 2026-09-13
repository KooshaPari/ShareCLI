//! sharecli - Shared CLI process manager
//!
//! Thin CLI wrapper around local process runtime.
//!
//! Features:
//! - Process management via local runtime types
//! - Multi-project orchestration

// --- Tier A/B: product + ops (root pub surface) ---
pub mod agent_call_policy;
pub mod audit_log;
pub mod cast;
pub mod commands;
pub mod config;
pub mod config_loader;
pub mod config_merger;
pub mod config_validator;
pub mod config_watcher;
pub mod coordination;
pub mod dashboard_assets;
pub mod env_manager;
pub mod error;
pub mod error_envelope;
pub mod health;
pub mod health_check;
pub mod http_red;
pub mod log_sink;
pub mod metrics;
pub mod monitoring;
pub mod notifier;
pub mod otel;
pub mod pool_index {
    pub use sharecli_sync::PoolIndex;
}
pub mod paths;
pub mod pprof_http;
pub mod proc_table;
pub mod progress;
pub mod pyroscope_stub;
pub mod runtime;
pub mod scheduler;
pub mod serve_auth;
pub mod serve_lock;
pub mod serve_rate_limit;
pub mod session;
mod shutdown;
pub mod signals;
pub mod spawn_policy;
pub mod tray_http;
pub mod watchdog;

/// Parity / expansion utilities. C00 L1 Phase 1 — `docs/ops/lib-sprawl-plan.md`.
pub mod util;

pub use anyhow::Result;
pub use runtime::{
    ManagedProcess, ProcessFilter, ProcessInfo, ProcessPool, ProjectLimits, ProjectResources,
    SharedRuntime,
};
// Compatibility re-exports (Phase 1 non-breaking): legacy `sharecli::<mod>` paths.
pub use util::ansi;
pub use util::apfs_uuid;
pub use util::api;
pub use util::argparse;
pub use util::astar;
pub use util::backoff;
pub use util::base64_util;
pub use util::base64url;
pub use util::base85;
pub use util::binary_search;
pub use util::binary_search_ex;
pub use util::bip39_mnemonic;
pub use util::bip39_wordlist;
pub use util::bloom;
pub use util::bloom_filter;
pub use util::bucks;
pub use util::cache;
pub use util::catalan_number;
pub use util::cdp_meraki_discovery;
pub use util::chacha20;
pub use util::color;
pub use util::crc64;
pub use util::credit_card;
pub use util::cron_parser;
pub use util::csv_util;
pub use util::csv_writer;
pub use util::cyclic_check;
pub use util::deque;
pub use util::dhcpv6_msg;
pub use util::disjoint_set;
pub use util::distance;
pub use util::dnssec_chain;
pub use util::feature_flags;
pub use util::flatbuffers_lite;
pub use util::glob_pattern;
pub use util::graph;
pub use util::hash_util;
pub use util::hkdf;
pub use util::html_escape;
pub use util::ini_parser;
pub use util::ipaddr_validation;
pub use util::ipv4_util;
pub use util::itoa;
pub use util::json_pointer;
pub use util::jsonpath_lite;
pub use util::jsonschema_subset;
pub use util::keccak;
pub use util::kmp_search;
pub use util::lazy;
pub use util::levenshtein;
pub use util::lru;
pub use util::lru_cache_ext;
pub use util::macho_parse;
pub use util::mapi_props;
pub use util::mapi_props_parity;
pub use util::markdown_inline;
pub use util::matrix;
pub use util::matrix_ops;
pub use util::md_table;
pub use util::mime_qp;
pub use util::msgpack;
pub use util::natural_sort;
pub use util::ntp_timestamp;
pub use util::object_pool;
pub use util::pem_decode;
pub use util::perm;
pub use util::pin;
pub use util::priority_queue;
pub use util::queue;
pub use util::queue2;
pub use util::radix_trie;
pub use util::rate_limiter;
pub use util::rational;
pub use util::retry;
pub use util::ring_buffer;
pub use util::segment_tree_basic;
pub use util::sha3_keccak;
pub use util::skiplist;
pub use util::slice_ext;
pub use util::sliding_window;
pub use util::sorted_vec;
pub use util::sortedset;
pub use util::sqrt_integer;
pub use util::stack;
pub use util::stats;
pub use util::stopwatch;
pub use util::stream;
pub use util::tar_util;
pub use util::template;
pub use util::text_slab;
pub use util::toml_lite;
pub use util::topological_sort;
pub use util::trie_compressed;
pub use util::trim;
pub use util::typed_id;
pub use util::url_safe_base64;
pub use util::utf8v;
pub use util::uuid;
pub use util::uuid_v7;
pub use util::vlq;
pub use util::word_count;
pub use util::xml_escape;
pub use util::xxhash3;
pub use util::xxtea;
pub use util::zip_crc32;

#[cfg(test)]
pub mod proptest_util;
