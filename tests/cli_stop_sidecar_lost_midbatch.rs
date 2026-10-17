//! FR: FR-006
//!
//! `sharecli stop --project` builds its target list from a single
//! `process.list` snapshot, then issues one `process.kill` per target. If the
//! sidecar transport disappears *between* the snapshot and the kills (the
//! sidecar vanished after a successful start), the current code falls into
//! `None => false` and the loop finishes silently: the affected processes are
//! left running with no operator signal.
//!
//! This test drives the real `sharecli` binary against a minimal fake sidecar
//! that answers exactly one `process.list` and then makes the socket
//! unreachable (the path is unlinked *before* the reply is written, so the
//! next connect deterministically sees ENOENT — no timing race). The fix must
//! emit a stderr warning naming how many processes could not be verified,
//! while leaving the no-sidecar-at-START taxonomy (exit 2 / not_found)
//! untouched.
//!
//! Unix-only: the sidecar transport is a unix socket.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

/// Answer exactly one `process.list` with a single project-scoped target, then
/// unlink the socket path before replying so the CLI's subsequent
/// `process.kill` connect fails with ENOENT (transport gone mid-batch).
fn serve_one_list_then_vanish(sock: &Path) -> std::thread::JoinHandle<()> {
    let sock = sock.to_path_buf();
    let listener = UnixListener::bind(&sock).expect("fixture: bind fake sidecar");
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("fixture: accept list");
        let mut line = String::new();
        BufReader::new(stream.try_clone().expect("fixture: clone stream"))
            .read_line(&mut line)
            .expect("fixture: read list request");
        let id = serde_json::from_str::<Value>(&line)
            .expect("fixture: decode request")
            .get("id")
            .cloned()
            .unwrap_or(Value::Null);

        // Remove the path BEFORE replying: the CLI cannot advance past reading
        // the reply, so its next connect is guaranteed to find no socket.
        std::fs::remove_file(&sock).expect("fixture: unlink socket");

        let reply = json!({
            "id": id,
            "result": [{"pid": 999, "name": "phantom", "project": "midbatch"}],
            "error": null,
        });
        let mut stream = stream;
        writeln!(stream, "{reply}").expect("fixture: write reply");
        stream.flush().expect("fixture: flush reply");
    })
}

/// Mid-batch sidecar loss must be reported, never silently swallowed.
#[test]
fn stop_project_warns_when_sidecar_vanishes_mid_batch() {
    let dir = tempfile::tempdir().expect("isolated fixture dir");
    let sock = dir.path().join("midbatch.sock");
    let server = serve_one_list_then_vanish(&sock);

    let out = Command::new(env!("CARGO_BIN_EXE_sharecli"))
        .args(["stop", "--project", "midbatch"])
        .env("SHARECLI_IPC_SOCK", &sock)
        .env("SHARECLI_CONFIG_PATH", dir.path().join("config.toml"))
        .env("HOME", dir.path())
        .output()
        .expect("run sharecli stop --project");
    server.join().expect("fixture server thread");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("sidecar connection lost mid-batch"),
        "a mid-batch sidecar loss must be reported on stderr, not swallowed silently; \
         exit={:?}\nstdout:{}\nstderr:{stderr}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
    );
    assert!(
        stderr.contains('1'),
        "the warning must name how many processes could not be verified; stderr:{stderr}"
    );
}
