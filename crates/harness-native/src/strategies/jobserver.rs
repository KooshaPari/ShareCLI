// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::path::Path;

use super::process;
use super::RuleOpts;

/// Delegate to Hypervisor-backed process lane (FR-008 AC-008.19).
pub fn run(
    harness_home: &Path,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    process::run_status(harness_home, real_cmd, args, opts)
}
