// FR: FR-008 mature-recovery inherited jobserver admission
#![cfg(unix)]

use std::process::Command;

#[test]
fn inherited_make_jobserver_is_discovered_and_reused_by_nested_child() {
    if std::env::var_os("SHARECLI_INHERITED_JOBSERVER_CHILD").is_some() {
        let client = unsafe {
            sharecli_ipc::NativeJobserverClient::from_environment()
                .expect("inherited jobserver environment must be usable")
                .expect("inherited jobserver must be discovered")
        };

        // The recursive make recipe already owns its implicit slot. Borrow
        // exactly one extra token, return it, and verify it is reusable.
        let before = client.available_tokens().expect("inherited token count");
        let lease = client
            .acquire_extra_tokens(sharecli_ipc::extra_tokens_for_total_parallelism(2))
            .expect("acquire inherited extra slot");
        assert_eq!(lease.token_count(), 1);
        assert_eq!(client.available_tokens().expect("leased token count") + 1, before);

        let mut nested = Command::new("sh");
        nested.args(["-c", r#"test -n "$MAKEFLAGS$CARGO_MAKEFLAGS""#]);
        client.configure_make_child(&mut nested);
        let status = nested.status().expect("nested make-compatible child");
        assert!(status.success(), "nested child did not inherit jobserver state");
        drop(lease);
        assert_eq!(client.available_tokens().expect("returned token count"), before);
        let returned = client.acquire_extra_tokens(1).expect("returned token reusable");
        assert_eq!(returned.token_count(), 1);
        return;
    }

    let exe = std::env::current_exe().expect("current test executable");
    // '+' marks this recipe recursive, so GNU make preserves pipe jobserver
    // descriptors. Without it MAKEFLAGS can advertise closed descriptors.
    let makefile = format!(
        "all:\n\t+SHARECLI_INHERITED_JOBSERVER_CHILD=1 '{}' --exact inherited_make_jobserver_is_discovered_and_reused_by_nested_child --nocapture\n",
        exe.display()
    );
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("Makefile");
    std::fs::write(&path, makefile).expect("write Makefile");

    let output = Command::new("make")
        // The outer cargo test harness may advertise its own unavailable FDs.
        // This fixture must test the NEW make-owned provider, not those FDs.
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("MAKELEVEL")
        .arg("-j2")
        .arg("-f")
        .arg(&path)
        .current_dir(dir.path())
        .output()
        .expect("GNU make must be available for inherited jobserver oracle");

    assert!(
        output.status.success(),
        "inherited jobserver fixture failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
