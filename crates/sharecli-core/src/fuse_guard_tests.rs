//! FR: FR-008 / FR-009 — failed unmount must never trigger recursive mount cleanup.

use super::FuseGuard;

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn failed_unmount_preserves_mount_directory_contents() {
    let parent = tempfile::tempdir().expect("isolated parent");
    let mount_dir = tempfile::tempdir_in(parent.path()).expect("owned mount directory");
    let mountpoint = mount_dir.path().to_path_buf();
    let sentinel = mountpoint.join("backing-data.txt");
    std::fs::write(&sentinel, b"must survive failed unmount").expect("sentinel");

    // An ordinary directory is deliberately not a live mount. Unmount fails,
    // simulating the dangerous cleanup path without exposing a backing tree.
    drop(FuseGuard {
        mountpoint: Some(mountpoint),
        _tmpdir: Some(mount_dir),
        readiness_marker: None,
    });

    assert_eq!(
        std::fs::read(&sentinel).expect("failed unmount must preserve directory contents"),
        b"must survive failed unmount"
    );
}

#[test]
fn unmounted_guard_still_cleans_owned_temporary_directory() {
    let mount_dir = tempfile::tempdir().expect("owned directory");
    let path = mount_dir.path().to_path_buf();
    drop(FuseGuard { mountpoint: None, _tmpdir: Some(mount_dir), readiness_marker: None });
    assert!(!path.exists(), "ordinary temporary directory must still be cleaned");
}
