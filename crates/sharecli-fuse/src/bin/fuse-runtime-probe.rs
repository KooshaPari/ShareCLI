// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Read-only runtime evidence for the optional FUSE interception tier.
//!
//! This probe never loads a kext, changes approval state, mounts a volume, or
//! prompts for privilege. It reports the deterministic KEXT -> FSKit ->
//! non-FUSE selection used by ShareCLI.

use std::path::PathBuf;

use sharecli_fuse::probe_runtime;

fn main() {
    let mountpoint = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/sharecli-fuse-runtime-probe"));
    let evidence = probe_runtime(&mountpoint);
    let report = serde_json::json!({
        "platform": evidence.platform,
        "mountpoint": evidence.mountpoint,
        "kernel_loaded": evidence.kernel_loaded,
        "fskit_framework": evidence.fskit_framework,
        "fskit_approved": evidence.fskit_approved,
        "selected_backend": evidence.selection.backend.as_str(),
        "diagnostic": evidence.selection.diagnostic.map(|diagnostic| diagnostic.message()),
        "non_fuse_fallback": evidence.non_fuse_fallback,
    });
    println!("{}", serde_json::to_string_pretty(&report).expect("serialize probe report"));
}
