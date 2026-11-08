// SPDX-License-Identifier: Apache-2.0 OR MIT
//! fuse-smoke-runner library — matrix types + executors.

#![warn(missing_docs)]

pub mod exec;
pub mod matrix;

pub use exec::run_matrix;
pub use matrix::{
    default_cells_for_host, find_repo_root, CellId, CellResult, FailReason, MatrixReport,
};
