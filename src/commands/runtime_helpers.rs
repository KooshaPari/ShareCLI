// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shared static singletons for command-family runtime accessors.
//!
//! The `SHARED_RUNTIME` `OnceLock<SharedRuntime>` is the canonical instance
//! that `pool` / `status` / `health` / `run_pool` read from; `PROJECT_RESOURCES`
//! backs `set_limits` / `check_limits`. Both are kept in their own file so the
//! `commands::` module root stays a thin re-export shell.

use crate::config;
use crate::runtime::{ProjectResources, SharedRuntime};

/// Shared runtime instance
static SHARED_RUNTIME: std::sync::OnceLock<SharedRuntime> = std::sync::OnceLock::new();

pub(crate) fn get_shared_runtime() -> &'static SharedRuntime {
    SHARED_RUNTIME.get_or_init(|| {
        let max = config::global().pool.max_per_type;
        SharedRuntime::new(max)
    })
}

/// Project resources instance
static PROJECT_RESOURCES: std::sync::OnceLock<ProjectResources> = std::sync::OnceLock::new();

pub(crate) fn get_project_resources() -> &'static ProjectResources {
    PROJECT_RESOURCES.get_or_init(ProjectResources::new)
}
