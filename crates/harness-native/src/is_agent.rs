// SPDX-License-Identifier: Apache-2.0 OR MIT
//! harness-is-agent binary: detect if current process is an agent

use std::env;
use std::process;

fn main() {
    let _ppid = env::var("PPID").unwrap_or_else(|_| "1".to_string());
    // TODO: Check agent detection logic
    // For now, exit with code 1 (not agent)
    process::exit(1);
}
