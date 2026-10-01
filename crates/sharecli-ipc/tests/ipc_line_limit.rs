//! FR: FR-003
//!
//! Lane-3 HIGH (FINDINGS.md): `BufReader::lines()` has no maximum line length,
//! so a local client can grow the sidecar's heap without bound by streaming
//! bytes that never contain a newline. The specified fix is
//! `tokio_util::codec::LinesCodec::new_with_max_length` over the read half.
//!
//! These tests spawn the real `sharecli-ipc` binary so the evidence is the
//! wire behaviour an operator would actually observe, not an in-process
//! approximation.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The cap the server must enforce. Kept local so this file compiles — and can
/// therefore fail honestly — against the original server. It is also the
/// boundary for the "just under the cap" case below: comfortably above any
/// real request (`process.spawn` carries name/command/args/project/harness/
/// cwd only) and far below the 1 MiB frame the plan asks us to reject.
const MAX_REQUEST_LINE_BYTES: usize = 256 * 1024;

/// The plan's validation frame: a line well past the cap.
const OVERSIZED_FRAME_BYTES: usize = 1024 * 1024;

/// One sidecar instance on its own socket and its own state directories.
struct Sidecar {
    /// Killed before `_directory` drops, because dropping the child closes its
    /// socket while the directory still exists. Fields drop in declaration
    /// order, so the process must come first.
    child: Child,
    _directory: tempfile::TempDir,
    socket: PathBuf,
}

impl Sidecar {
    fn start() -> Self {
        let directory = tempfile::tempdir().expect("isolated sidecar directory");
        let socket = directory.path().join("ipc.sock");

        // Child-only environment: this process is shared by the rest of the
        // suite, so nothing here may be set with std::env::set_var.
        let mut command = Command::new(env!("CARGO_BIN_EXE_sharecli-ipc"));
        command
            .env("SHARECLI_IPC_SOCK", &socket)
            .env("SHARECLI_CONFIG_PATH", directory.path().join("config.toml"))
            .env("SHARECLI_SESSION_DB", directory.path().join("sessions.sqlite"))
            .env("RUST_LOG", "info")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command.spawn().expect("spawn sharecli-ipc");
        let this = Self { child, _directory: directory, socket };

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if UnixStream::connect(&this.socket).is_ok() {
                return this;
            }
            assert!(Instant::now() < deadline, "sidecar never bound {}", this.socket.display());
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn connect(&self) -> UnixStream {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match UnixStream::connect(&self.socket) {
                Ok(stream) => return stream,
                Err(e) => {
                    assert!(
                        Instant::now() < deadline,
                        "connect to {} failed: {e}",
                        self.socket.display()
                    );
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Read until a complete response frame arrives, the peer hangs up, or the
/// deadline passes. Returns the bytes seen and whether the connection ended.
fn read_frame(stream: &mut UnixStream, deadline: Duration) -> (Vec<u8>, bool) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let expires = Instant::now() + deadline;
    let mut seen = Vec::new();

    loop {
        if seen.contains(&b'\n') {
            return (seen, true);
        }
        if Instant::now() >= expires {
            return (seen, false);
        }
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => return (seen, true),
            Ok(n) => seen.extend_from_slice(&chunk[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::NotConnected
                ) =>
            {
                // The server already hung up on us.
                return (seen, true);
            }
            // Per-slice read timeout: keep polling until the deadline.
            Err(_) => {}
        }
    }
}

/// The finding itself: a frame past the cap must be refused — not buffered,
/// and not answered. The original server parses the whole line and replies
/// with a JSON-RPC parse-error envelope, so any response is the failure.
///
/// There are two correct refusal shapes and the test accepts both: the server
/// may close after the client has queued the entire frame (EOF on read), or it
/// may close while the client is still writing, which surfaces as
/// `BrokenPipe`. Either way nothing must come back.
#[test]
fn oversized_frame_is_rejected_without_a_response() {
    let sidecar = Sidecar::start();
    let mut stream = sidecar.connect();

    let mut frame = vec![b'x'; OVERSIZED_FRAME_BYTES];
    frame.push(b'\n');
    let write_outcome = stream.write_all(&frame).and_then(|()| stream.flush());
    let write_refused = matches!(
        &write_outcome,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            )
    );

    let (seen, hung_up) = read_frame(&mut stream, Duration::from_secs(10));
    assert!(
        seen.is_empty(),
        "a {} byte frame must be rejected without a reply, but the server answered with {} bytes: {}",
        OVERSIZED_FRAME_BYTES,
        seen.len(),
        String::from_utf8_lossy(&seen)
    );
    assert!(
        write_refused || hung_up,
        "the server must hang up on an oversized frame; write={write_outcome:?}, hung_up={hung_up}"
    );
}

/// A frame just under the cap must still be served. Without this the cap could
/// be "fixed" by shrinking it below legitimate traffic and the rejection test
/// would stay green.
#[test]
fn a_frame_just_under_the_cap_is_served() {
    let sidecar = Sidecar::start();
    let mut stream = sidecar.connect();

    // Pad a real request with params the handler ignores, so the frame lands
    // just below the cap while remaining a valid, answerable RPC.
    let overhead = "{\"id\":6,\"method\":\"health.status\",\"params\":{\"pad\":\"\"}}\n".len();
    let padding = MAX_REQUEST_LINE_BYTES - overhead - 1;
    let request = format!(
        "{{\"id\":6,\"method\":\"health.status\",\"params\":{{\"pad\":\"{}\"}}}}\n",
        "p".repeat(padding)
    );
    assert!(
        request.len() <= MAX_REQUEST_LINE_BYTES,
        "boundary frame must fit under the cap ({} > {})",
        request.len(),
        MAX_REQUEST_LINE_BYTES
    );
    assert!(
        request.len() >= MAX_REQUEST_LINE_BYTES - 64,
        "boundary frame must actually be near the cap"
    );

    stream.write_all(request.as_bytes()).expect("write boundary frame");
    stream.flush().expect("flush boundary frame");

    let (seen, _hung_up) = read_frame(&mut stream, Duration::from_secs(10));
    let text = String::from_utf8_lossy(&seen);
    assert!(text.contains("\"id\":6"), "a frame under the cap must be served, got: {text}");
}

/// The cap bounds a *frame*, not the number of clients: the tray, the CLI and
/// the dashboard all talk to the sidecar at once. This records that "single
/// connection" was deliberately not implemented server-side.
#[test]
fn a_second_concurrent_connection_is_still_served() {
    let sidecar = Sidecar::start();
    let _first = sidecar.connect();
    let mut second = sidecar.connect();

    second
        .write_all(b"{\"id\":8,\"method\":\"health.status\",\"params\":{}}\n")
        .expect("write on second connection");
    second.flush().expect("flush on second connection");

    let (seen, _hung_up) = read_frame(&mut second, Duration::from_secs(10));
    let text = String::from_utf8_lossy(&seen);
    assert!(text.contains("\"id\":8"), "second connection must be served, got: {text}");
}
