// FR: FR-008 / SC-WP-B03. Process death, not only an unwind in one process.
#![cfg(unix)]
use std::fs;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use fs2::FileExt;
use sharecli_ipc::{QueuePriority, SlotQueue};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
fn lease_crash_child() {
    let Some(root) = std::env::var_os("SHARECLI_LEASE_CRASH_FIXTURE") else { return; };
    SlotQueue::with_options(root, 1, Duration::from_secs(30), Duration::from_millis(5))
        .with_slot("crash", QueuePriority::Critical, || Ok(())).unwrap();
}

#[test]
fn killed_waiter_releases_ownership_without_ticket_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let slot = fs::OpenOptions::new().create(true).write(true).truncate(false)
        .open(dir.path().join("crash.slot0.lock")).unwrap();
    FileExt::lock_exclusive(&slot).unwrap();
    let mut child = ChildGuard(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lease_crash_child", "--nocapture"])
        .env("SHARECLI_LEASE_CRASH_FIXTURE", dir.path())
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let waiting = dir.path().join("crash.waiting");
    let deadline = Instant::now() + Duration::from_secs(5);
    let leaked = loop {
        assert!(child.0.try_wait().unwrap().is_none(), "child exited before publishing waiter");
        if let Ok(entries) = fs::read_dir(&waiting) {
            let mut leased = None;
            for entry in entries.filter_map(Result::ok) {
                if entry.file_name().to_string_lossy().starts_with('.') { continue; }
                let file = fs::OpenOptions::new().read(true).write(true).open(entry.path()).unwrap();
                if let Err(error) = FileExt::try_lock_shared(&file) {
                    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
                    leased = Some(entry.path());
                    break;
                }
            }
            if let Some(path) = leased { break path; }
        }
        assert!(Instant::now() < deadline, "child did not publish a held waiter lease");
        std::thread::sleep(Duration::from_millis(2));
    };
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success(), "fixture must terminate the process");
    assert!(leaked.exists(), "process death should leave the ticket file behind");
    drop(slot);
    SlotQueue::with_options(dir.path(), 1, Duration::from_millis(500), Duration::from_millis(5))
        .with_slot("crash", QueuePriority::Normal, || Ok(())).unwrap();
    assert!(leaked.exists(), "peer must not unlink an old claim merely from PID state");
}
