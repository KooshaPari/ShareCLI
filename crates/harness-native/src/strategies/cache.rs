// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::path::{Path, PathBuf};

pub fn l1_path(harness_home: &Path, key: &str) -> PathBuf {
    harness_home
        .join("var")
        .join("cache")
        .join("l1")
        .join(format!("{}.json", key))
}

pub fn l2_path(harness_home: &Path, key: &str) -> PathBuf {
    harness_home
        .join("var")
        .join("cache")
        .join("l2")
        .join(format!("{}.json", key))
}

pub fn lock_shared(path: &Path, timeout_secs: u64) -> Option<std::fs::File> {
    use fs2::FileExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .ok()?;
    file.lock_shared().ok()?;
    Some(file)
}
