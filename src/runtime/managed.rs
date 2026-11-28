//! `ManagedProcess` — process info + substrate handle pair, one entry in the
//! `ProcessPool`'s `processes` map.

use substrate::ProcessHandle;

use crate::spawn_policy::BuildPermit;

use super::admission::SlotLease;
use super::process_info::ProcessInfo;

/// One slot in `ProcessPool::processes`. Pairs the dashboard metadata
/// (`info`) with the substrate `ProcessHandle` required to issue a
/// `kill_group` later. Both fields are public so the `pool` module can
/// read them without adding accessors.
pub struct ManagedProcess {
    pub info: ProcessInfo,
    pub handle: ProcessHandle,
    /// Device-scoped build slot, held for the whole child lifetime.
    ///
    /// Preferred over `permit`: an in-process semaphore cannot see a sibling
    /// `sharecli` invocation, so only a device slot makes
    /// `max_concurrent_builds` mean what it says. `None` for non-build harnesses
    /// and when the device slot pool is unavailable (then `permit` is used).
    pub lease: Option<SlotLease>,
    /// In-process build-slot permit, used only when the device slot pool is
    /// unavailable.
    ///
    /// Dropping it releases the semaphore, so it travels with the process entry
    /// instead of living as a local in `spawn`. `None` for non-build harnesses
    /// and when no `SpawnPolicy` is configured.
    pub permit: Option<BuildPermit>,
}

// Hand-written because `BuildPermit` owns a Zig semaphore handle and is not
// `Debug`; a permit's presence is the only thing worth printing.
impl std::fmt::Debug for ManagedProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedProcess")
            .field("info", &self.info)
            .field("handle", &self.handle)
            .field("build_slot_held", &(self.lease.is_some() || self.permit.is_some()))
            .field("build_slot_scope", &if self.lease.is_some() { "device" } else { "process" })
            .finish()
    }
}
