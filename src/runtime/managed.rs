// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `ManagedProcess` — process info + substrate handle pair, one entry in the
//! `ProcessPool`'s `processes` map.

use substrate::ProcessHandle;

use super::process_info::ProcessInfo;

/// One slot in `ProcessPool::processes`. Pairs the dashboard metadata
/// (`info`) with the substrate `ProcessHandle` required to issue a
/// `kill_group` later. Both fields are public so the `pool` module can
/// read them without adding accessors.
#[derive(Debug)]
pub struct ManagedProcess {
    pub info: ProcessInfo,
    pub handle: ProcessHandle,
}
