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

        // One implicit GNU-make slot exists already. Acquire exactly one extra
        // slot for total parallelism two, then prove nested propagation.
        let lease = client
            .acquire_extra_tokens(sharecli_ipc::extra_tokens_for_total_parallelism(2))
            .expect("acquire inherited extra slot");
        assert_eq!(lease.token_count(), 1);

        let mut nested = Command::new("sh");
        nested.args([
            "-c",
            r#"test -n "$MAKEFLAGS$CARGO_MAKEFLAGS""#,
        ]);
        client.configure_make_child(&mut nested);
        let status = nested.status().expect("nested make-compatible child");
        assert!(status.success(), "nested child did not inherit jobserver state");
        return;
    }

    // GNU make owns the real jobserver. The recipe launches this same test
    // binary as a child so NativeJobserverClient::from_environment observes an
    // externally supplied provider rather than a ShareCLI-owned pool.
    let exe = std::env::current_exe().expect("current test executable");
    let makefile = format!(
        "all:\n\tSHARECLI_INHERITED_JOBSERVER_CHILD=1 '{}' --exact inherited_make_jobserver_is_discovered_and_reused_by_nested_child --nocapture\n",
        exe.display()
    );
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("Makefile");
    std::fs::write(&path, makefile).expect("write Makefile");

    let output = Command::new("make")
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
