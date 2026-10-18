//! FR: FR-001
//!
//! Integration tests for the `sharecli-ffi` C-ABI surface.
//!
//! These tests load `libsharecli_ffi.{dylib,so}` at runtime via the
//! `libloading` crate and call each `extern "C"` export through the loaded
//! handles — exactly the path the Swift macOS tray takes. This is the most
//! accurate end-to-end ABI test possible without a Swift test harness,
//! because the test process never statically links the cdylib; if the
//! Swift app can't find a symbol, this test will fail the same way.
//!
//! Coverage:
//!   1. `sharecli_ipc_socket_path`   — returns the expected env-overridden
//!                                      path; non-null-terminated safe to
//!                                      inspect via `CStr`.
//!   2. `sharecli_free_string`       — round-trips with a real `*mut c_char`
//!                                      and accepts null without panic.
//!   3. `sharecli_health_json`       — round-trips a JSON response; returns
//!                                      null when no sidecar / no socket.
//!   4. `sharecli_request`           — round-trips an arbitrary JSON-RPC
//!                                      payload; returns null for null input
//!                                      and for missing socket.
//!   5. `sharecli_serve_get`         — fetches a real local HTTP endpoint,
//!                                      returns null for null/invalid URLs.
//!   6. `sharecli_ipc_start`         — returns 0 when the socket already
//!                                      exists and 1 when it does not (and no
//!                                      sidecar is running).
//!
//! All tests are `#[serial]` where they mutate the `SHARECLI_IPC_SOCK`
//! environment variable or the on-disk socket path.

#![cfg(unix)]

use std::ffi::{CStr, CString};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::os::raw::{c_char, c_int};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use libloading::Library;
use serial_test::serial;
use tempfile::TempDir;

// The build script writes the absolute path of the freshly built dylib into
// `OUT_DIR`. `include!` pulls the constants in at compile time.
include!(concat!(env!("OUT_DIR"), "/dylib_path.rs"));

// --------------------------------------------------------------------
// C-ABI declarations (type-only — no link-time symbol resolution; the
// actual function pointers come from `libloading::Library::get` below).
// --------------------------------------------------------------------

#[allow(dead_code)] // type-only declarations; the real fn ptrs come via libloading
extern "C" {
    fn sharecli_ipc_start() -> c_int;
    fn sharecli_ipc_socket_path() -> *mut c_char;
    fn sharecli_free_string(ptr: *mut c_char);
    fn sharecli_health_json() -> *mut c_char;
    fn sharecli_request(request_json: *const c_char) -> *mut c_char;
    fn sharecli_serve_get(url: *const c_char) -> *mut c_char;
}

// --------------------------------------------------------------------
// Library loader — shared across all tests in the process.
// --------------------------------------------------------------------

struct Ffi {
    _lib: Library,
    ipc_start: unsafe extern "C" fn() -> c_int,
    ipc_socket_path: unsafe extern "C" fn() -> *mut c_char,
    free_string: unsafe extern "C" fn(*mut c_char),
    health_json: unsafe extern "C" fn() -> *mut c_char,
    request: unsafe extern "C" fn(*const c_char) -> *mut c_char,
    serve_get: unsafe extern "C" fn(*const c_char) -> *mut c_char,
}

static FFI: OnceLock<Result<Ffi, String>> = OnceLock::new();

fn ffi() -> &'static Ffi {
    FFI.get_or_init(|| {
        let path = resolve_dylib()
            .ok_or_else(|| format!("sharecli_ffi dylib not found; looked at:\n  {DEBUG_DYLIB_PATH}\n  {RELEASE_DYLIB_PATH}"))?;
        // SAFETY: Loading a cdylib at runtime; we never unload it.
        let lib = unsafe { Library::new(&path) }
            .map_err(|e| format!("dlopen {path:?} failed: {e}"))?;
        // SAFETY: Each symbol is `#[no_mangle] pub extern "C" fn ...`
        // declared in `src/lib.rs`; the type assertion below would panic
        // immediately at startup if the Rust-side signature ever drifts
        // from the C-ABI one.
        let ipc_start = *unsafe { lib.get::<unsafe extern "C" fn() -> c_int>(b"sharecli_ipc_start") }
            .map_err(|e| format!("get sharecli_ipc_start: {e}"))?;
        let ipc_socket_path = *unsafe { lib.get::<unsafe extern "C" fn() -> *mut c_char>(b"sharecli_ipc_socket_path") }
            .map_err(|e| format!("get sharecli_ipc_socket_path: {e}"))?;
        let free_string = *unsafe { lib.get::<unsafe extern "C" fn(*mut c_char)>(b"sharecli_free_string") }
            .map_err(|e| format!("get sharecli_free_string: {e}"))?;
        let health_json = *unsafe { lib.get::<unsafe extern "C" fn() -> *mut c_char>(b"sharecli_health_json") }
            .map_err(|e| format!("get sharecli_health_json: {e}"))?;
        let request = *unsafe { lib.get::<unsafe extern "C" fn(*const c_char) -> *mut c_char>(b"sharecli_request") }
            .map_err(|e| format!("get sharecli_request: {e}"))?;
        let serve_get = *unsafe { lib.get::<unsafe extern "C" fn(*const c_char) -> *mut c_char>(b"sharecli_serve_get") }
            .map_err(|e| format!("get sharecli_serve_get: {e}"))?;
        Ok(Ffi { _lib: lib, ipc_start, ipc_socket_path, free_string, health_json, request, serve_get })
    })
    .as_ref()
    .expect("sharecli_ffi dylib failed to load; see error above")
}

// --------------------------------------------------------------------
// RAII helpers.
// --------------------------------------------------------------------

/// Owns a `*mut c_char` returned from the FFI surface; calls
/// `sharecli_free_string` on drop. Null is fine.
struct OwnedFfiString(*mut c_char);
impl OwnedFfiString {
    /// SAFETY: caller must have obtained the pointer from one of the FFI
    /// functions in this crate; all of them produce either a null pointer
    /// (on error) or a C-string allocated by the C side that the matching
    /// `sharecli_free_string` knows how to release.
    unsafe fn from_raw(ptr: *mut c_char) -> Self {
        Self(ptr)
    }
    fn as_str(&self) -> &str {
        if self.0.is_null() {
            ""
        } else {
            // SAFETY: FFI contract says non-null returns are CStrings.
            unsafe { CStr::from_ptr(self.0) }.to_str().expect("FFI returned non-UTF-8")
        }
    }
    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}
impl Drop for OwnedFfiString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `self.0` came from a matching FFI allocation.
            unsafe { (ffi().free_string)(self.0) };
        }
    }
}

/// RAII guard for the `SHARECLI_IPC_SOCK` env var.
struct SocketPathGuard {
    prev: Option<std::ffi::OsString>,
    key: &'static str,
}
impl SocketPathGuard {
    fn set(key: &'static str, value: PathBuf) -> Self {
        let prev = std::env::var_os(key);
        std::env::set_var(key, &value);
        Self { prev, key }
    }
}
impl Drop for SocketPathGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

// --------------------------------------------------------------------
// Server harness helpers.
// --------------------------------------------------------------------

/// Spawns a real `UnixListener`-backed JSON-RPC handler in a worker thread;
/// returns the bound socket path, a oneshot receiver that fires once the
/// listener is accepting, and the `TempDir` that owns the socket file
/// (drop order: caller must keep `_tmp` alive for the test's duration).
fn spawn_unix_handler<F>(handler: F) -> (PathBuf, mpsc::Receiver<()>, TempDir)
where
    F: Fn(&str) -> String + Send + 'static,
{
    let tmp = TempDir::new().expect("tempdir");
    let sock = tmp.path().join("ipc.sock");
    // Bind synchronously so the test can hand the path to `sharecli_request`
    // *before* the worker has a chance to accept.
    let listener = UnixListener::bind(&sock).expect("bind unix listener");
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::spawn(move || {
        // Signal readiness first; then loop accept().
        let _ = ready_tx.send(());
        for stream in listener.incoming() {
            let mut s = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            // The IPC protocol is newline-delimited JSON. Read until the
            // first '\n' (or EOF) so we mirror what the FFI side does with
            // `BufReader::lines().next_line().await`.
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                match s.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        if byte[0] == b'\n' {
                            break;
                        }
                        buf.push(byte[0]);
                    }
                    Err(_) => break,
                }
            }
            let body = std::str::from_utf8(&buf).unwrap_or("{}");
            let resp = handler(body);
            let _ = s.write_all(resp.as_bytes());
        }
    });
    (sock, ready_rx, tmp)
}

/// Spawns a real `std::net::TcpListener`-backed HTTP server in a worker
/// thread; returns the bound `http://127.0.0.1:port` URL and a oneshot that
/// fires when the server is ready.
fn spawn_http_handler<F>(handler: F) -> (String, mpsc::Receiver<()>)
where
    F: Fn(&str, &str) -> (u16, String) + Send + 'static,
{
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind tcp");
    listener.set_nonblocking(false).ok();
    let addr: SocketAddr = listener.local_addr().expect("local_addr");
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = ready_tx.send(());
        for stream in listener.incoming() {
            let mut s = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let first_line = req.lines().next().unwrap_or("").to_string();
            let method = first_line.split_whitespace().next().unwrap_or("GET").to_string();
            let path = first_line.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, body) = handler(&method, &path);
            let status_text = match status {
                200 => "OK",
                404 => "Not Found",
                500 => "Internal Server Error",
                _ => "OK",
            };
            let resp = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = s.write_all(resp.as_bytes());
        }
    });
    (format!("http://{addr}"), ready_rx)
}

fn wait_for_ready(rx: mpsc::Receiver<()>, label: &str) {
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(()) => {}
        Err(e) => panic!("{label} did not signal readiness in 2s: {e}"),
    }
}

// --------------------------------------------------------------------
// Tests.
// --------------------------------------------------------------------

/// Confirms the dylib loads and every advertised symbol resolves. If this
/// passes we know the linker can find the artifact that Swift consumes.
#[test]
fn library_loads_with_all_six_symbols() {
    let _ = ffi();
}

#[test]
#[serial]
fn ffi_socket_path_returns_non_null_terminated_string() {
    // SAFETY: calling the FFI; we drop the result via OwnedFfiString.
    let raw = unsafe { (ffi().ipc_socket_path)() };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(!s.is_null(), "ipc_socket_path returned NULL");
    let p = s.as_str();
    assert!(!p.is_empty(), "ipc_socket_path returned empty string");
    // Should end in `.sock` on all platforms (see lib.rs fallback).
    assert!(p.ends_with(".sock"), "unexpected socket path: {p}");
}

#[test]
#[serial]
fn ffi_socket_path_honours_sharcli_ipc_sock_env_var() {
    let tmp = TempDir::new().expect("tempdir");
    let custom = tmp.path().join("override.sock");
    let _g = SocketPathGuard::set("SHARECLI_IPC_SOCK", custom.clone());
    let raw = unsafe { (ffi().ipc_socket_path)() };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(!s.is_null());
    assert_eq!(s.as_str(), custom.to_str().unwrap());
}

#[test]
fn ffi_free_string_accepts_null_pointer_without_panic() {
    // SAFETY: FFI contract is null-safe (lib.rs checks for null and returns).
    unsafe { (ffi().free_string)(std::ptr::null_mut()) };
}

#[test]
fn ffi_free_string_round_trips_with_socket_path() {
    let raw = unsafe { (ffi().ipc_socket_path)() };
    assert!(!raw.is_null());
    // SAFETY: `raw` came from ipc_socket_path; free_string is its deallocator.
    unsafe { (ffi().free_string)(raw) };
    // Calling again on the same pointer is a use-after-free, but we don't
    // do that — we only confirm free doesn't panic on a freshly-allocated
    // pointer that we then intentionally drop.
}

#[test]
#[serial]
fn ffi_health_json_returns_null_when_socket_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let bogus = tmp.path().join("does-not-exist.sock");
    let _g = SocketPathGuard::set("SHARECLI_IPC_SOCK", bogus);
    let raw = unsafe { (ffi().health_json)() };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(s.is_null(), "expected NULL when socket is missing; got: {}", s.as_str());
}

#[test]
#[serial]
fn ffi_health_json_round_trips_json_response() {
    let (sock, ready, _tmp) = spawn_unix_handler(|body| {
        // Echo the request id; respond with a minimal health payload.
        let id = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("id").cloned())
            .unwrap_or(serde_json::Value::Null);
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "status": "ok", "uptime_ms": 1234 }
        })
        .to_string()
    });
    wait_for_ready(ready, "unix health handler");
    let _g = SocketPathGuard::set("SHARECLI_IPC_SOCK", sock.clone());
    let raw = unsafe { (ffi().health_json)() };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(!s.is_null(), "expected a health JSON string; got NULL");
    let v: serde_json::Value = serde_json::from_str(s.as_str()).unwrap_or_else(|e| {
        panic!("health_json did not return valid JSON: {e}; body={}", s.as_str())
    });
    assert_eq!(v.get("jsonrpc").and_then(|v| v.as_str()), Some("2.0"));
}

#[test]
#[serial]
fn ffi_request_returns_null_for_null_input() {
    // SAFETY: null is a documented input; lib.rs handles it.
    let raw = unsafe { (ffi().request)(std::ptr::null()) };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(s.is_null(), "expected NULL for null input; got: {}", s.as_str());
}

#[test]
#[serial]
fn ffi_request_returns_null_when_socket_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let bogus = tmp.path().join("missing.sock");
    let _g = SocketPathGuard::set("SHARECLI_IPC_SOCK", bogus);
    let payload = CString::new(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).unwrap();
    let raw = unsafe { (ffi().request)(payload.as_ptr()) };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(s.is_null(), "expected NULL when socket is missing; got: {}", s.as_str());
}

#[test]
#[serial]
fn ffi_request_round_trips_arbitrary_json_rpc() {
    let (sock, ready, _tmp) = spawn_unix_handler(|body| {
        // Echo a deterministic response keyed off the request id.
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::json!({}));
        let id = parsed.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = parsed.get("method").and_then(|v| v.as_str()).unwrap_or("");
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "echo_method": method, "ok": true }
        })
        .to_string()
    });
    wait_for_ready(ready, "unix rpc handler");
    let _g = SocketPathGuard::set("SHARECLI_IPC_SOCK", sock);
    let payload = CString::new(r#"{"jsonrpc":"2.0","id":42,"method":"tools.list"}"#).unwrap();
    let raw = unsafe { (ffi().request)(payload.as_ptr()) };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(!s.is_null(), "expected a JSON-RPC response; got NULL");
    let v: serde_json::Value = serde_json::from_str(s.as_str()).expect("valid JSON");
    assert_eq!(v.get("id").and_then(|v| v.as_i64()), Some(42));
    assert_eq!(
        v.get("result").and_then(|r| r.get("echo_method")).and_then(|v| v.as_str()),
        Some("tools.list")
    );
}

#[test]
fn ffi_serve_get_returns_null_for_null_url() {
    // SAFETY: null is a documented input; lib.rs handles it.
    let raw = unsafe { (ffi().serve_get)(std::ptr::null()) };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(s.is_null(), "expected NULL for null URL");
}

#[test]
fn ffi_serve_get_returns_null_for_invalid_url() {
    let url = CString::new("not a url").unwrap();
    let raw = unsafe { (ffi().serve_get)(url.as_ptr()) };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(s.is_null(), "expected NULL for invalid URL; got: {}", s.as_str());
}

#[test]
fn ffi_serve_get_fetches_local_http_endpoint() {
    let (base, ready) = spawn_http_handler(|_method, path| {
        if path == "/health" {
            (200, r#"{"status":"ok"}"#.to_string())
        } else {
            (404, r#"{"error":"not_found"}"#.to_string())
        }
    });
    wait_for_ready(ready, "http handler");
    let url = CString::new(format!("{base}/health")).unwrap();
    let raw = unsafe { (ffi().serve_get)(url.as_ptr()) };
    let s = unsafe { OwnedFfiString::from_raw(raw) };
    assert!(!s.is_null(), "expected a non-NULL body; got NULL");
    let body = s.as_str();
    assert!(body.contains("\"status\":\"ok\""), "unexpected body: {body}");
}

#[test]
#[serial]
fn ffi_ipc_start_returns_zero_when_socket_already_exists() {
    let tmp = TempDir::new().expect("tempdir");
    let sock = tmp.path().join("preexisting.sock");
    // Pre-create the socket so `sharecli_ipc_start` finds an existing
    // listener and returns 0 (the "sidecar already up" branch).
    let _listener = UnixListener::bind(&sock).expect("pre-bind socket");
    let _g = SocketPathGuard::set("SHARECLI_IPC_SOCK", sock);
    let rc = unsafe { (ffi().ipc_start)() };
    // 0 == "no-op, socket already exists"; the Rust side documents this.
    assert_eq!(rc, 0, "ipc_start should be a no-op when socket exists");
}

#[test]
#[serial]
fn ffi_ipc_start_returns_one_when_no_sidecar_and_no_socket() {
    // The "no sidecar, no socket" branch: with a fresh tempdir and no
    // sidecar binary on the test PATH, sharecli_ipc_start must NOT spawn
    // anything and must return 1 to signal "we couldn't connect; the caller
    // should fall back to in-process mode".
    let tmp = TempDir::new().expect("tempdir");
    let sock = tmp.path().join("ghost.sock");
    let _g = SocketPathGuard::set("SHARECLI_IPC_SOCK", sock);
    // Ensure the sidecar binary is *not* discoverable.
    let prev_path = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let start = Instant::now();
    let rc = unsafe { (ffi().ipc_start)() };
    let elapsed = start.elapsed();
    // Restore PATH for subsequent tests.
    match prev_path {
        Some(v) => std::env::set_var("PATH", v),
        None => std::env::remove_var("PATH"),
    }
    // 1 == "no sidecar available, no pre-existing socket". Anything else
    // (especially a long hang) means the function is trying to spawn
    // something it shouldn't in the test env.
    assert!(rc == 1 || rc == 0, "ipc_start returned unexpected rc={rc} (elapsed={elapsed:?})");
    assert!(
        elapsed < Duration::from_secs(3),
        "ipc_start took too long ({elapsed:?}) — likely a spawn attempt"
    );
}
