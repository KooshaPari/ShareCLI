// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shared proptest runner config (C07 L66 — shrink + file-backed replay).
//!
//! Committed seeds live under `proptest-regressions/` at the crate root.

use proptest::test_runner::Config as ProptestConfig;
use proptest::test_runner::FileFailurePersistence;

/// File-backed persistence for shrink/replay (`proptest-regressions/` at crate root).
pub fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::SourceParallel(
            "proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}
