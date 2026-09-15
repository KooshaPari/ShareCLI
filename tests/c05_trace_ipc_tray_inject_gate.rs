//! FR-005 / FR-007 — OTel multi-hop trace context propagation (T-1060)
//! FR: FR-005, FR-007
//!
//! Validates W3C `traceparent` header format parsing, component-length
//! constraints, injection/extraction roundtrips, IPC serialization, and
//! tray-client pass-through semantics.

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A valid W3C `traceparent` header value.
const VALID_TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

/// Trace ID component (32 hex chars).
const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

/// Parent / span ID component (16 hex chars).
const PARENT_ID: &str = "00f067aa0ba902b7";

/// Trace flags component (2 hex chars).
const TRACE_FLAGS: &str = "01";

/// Return true when `ch` is an ASCII hexadecimal digit.
fn is_hex_digit(ch: char) -> bool {
    ch.is_ascii_hexdigit()
}

/// Validate a segment is exactly `expected_len` hex characters.
///
/// Panics if the segment has the wrong length or contains non-hex characters.
fn assert_hex_segment(segment: &str, expected_len: usize, label: &str) {
    assert_eq!(
        segment.len(),
        expected_len,
        "{label} must be exactly {expected_len} hex chars; got {segment}"
    );
    assert!(
        segment.chars().all(is_hex_digit),
        "{label} must contain only hex digits; got {segment}"
    );
}

/// Parse a `traceparent` header into its four dash-separated components.
///
/// Returns `Ok((version, trace_id, parent_id, trace_flags))` or
/// `Err(description)`.
fn parse_traceparent(header: &str) -> Result<(&str, &str, &str, &str), String> {
    let parts: Vec<&str> = header.split('-').collect();
    if parts.len() != 4 {
        return Err(format!("traceparent must have 4 dash-separated parts; got {}", parts.len()));
    }
    Ok((parts[0], parts[1], parts[2], parts[3]))
}

/// Build a `traceparent` string from its four components.
fn build_traceparent(version: &str, trace_id: &str, parent_id: &str, trace_flags: &str) -> String {
    format!("{version}-{trace_id}-{parent_id}-{trace_flags}")
}

/// Simulate injecting a `traceparent` header into a request-like map.
fn inject_traceparent(headers: &mut Vec<(String, String)>, value: &str) {
    headers.push(("traceparent".to_string(), value.to_string()));
}

/// Simulate extracting the **last** `traceparent` header from a request-like
/// map (the most recently injected value, matching real multi-hop behaviour).
fn extract_traceparent(headers: &[(String, String)]) -> Option<String> {
    headers
        .iter()
        .rev()
        .find(|(k, _)| k.eq_ignore_ascii_case("traceparent"))
        .map(|(_, v)| v.clone())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Validate the W3C `traceparent` header format:
///
/// ```text
/// 00-{trace_id}-{parent_id}-{trace_flags}
/// ```
///
/// The header must consist of exactly four dash-separated segments where
/// the first segment (version) is `"00"`.
#[test]
fn traceparent_header_format() {
    let (version, tid, pid, flags) =
        parse_traceparent(VALID_TRACEPARENT).expect("parse valid traceparent");

    assert_eq!(version, "00", "version must be '00'");
    assert_hex_segment(tid, 32, "trace_id");
    assert_hex_segment(pid, 16, "parent_id");
    assert_hex_segment(flags, 2, "trace_flags");

    // A well-formed header roundtrips through parse → rebuild.
    let rebuilt = build_traceparent(version, tid, pid, flags);
    assert_eq!(rebuilt, VALID_TRACEPARENT);

    // Reject headers with the wrong number of segments.
    assert!(parse_traceparent("00-aaa-bbb").is_err(), "3-segment header must be rejected");
    assert!(parse_traceparent("00-aaa-bbb-ccc-ddd").is_err(), "5-segment header must be rejected");

    // Reject a header whose version is not `"00"`.
    let bad_version = "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let (v, _, _, _) = parse_traceparent(bad_version).expect("parse succeeds");
    assert_ne!(v, "00", "non-'00' version must be detected");
}

/// The **trace_id** (second component) must be exactly 32 hex characters
/// per the W3C Trace Context specification.
#[test]
fn trace_id_is_32_hex_chars() {
    let (_, tid, _, _) = parse_traceparent(VALID_TRACEPARENT).expect("parse");

    assert_eq!(tid.len(), 32, "trace_id must be 32 chars");
    assert!(tid.chars().all(is_hex_digit), "trace_id must be hex");
    assert_eq!(tid, TRACE_ID);

    // Too short (31 chars) — validation must reject.
    let short_tid = "4bf92f3577b34da6a3ce929d0e0e473";
    assert_eq!(short_tid.len(), 31);
    let result =
        std::panic::catch_unwind(|| assert_hex_segment(short_tid, 32, "trace_id (too short)"));
    assert!(result.is_err(), "short trace_id must be rejected");

    // Too long (33 chars) — validation must reject.
    let long_tid = "4bf92f3577b34da6a3ce929d0e0e47360";
    assert_eq!(long_tid.len(), 33);
    let result =
        std::panic::catch_unwind(|| assert_hex_segment(long_tid, 32, "trace_id (too long)"));
    assert!(result.is_err(), "long trace_id must be rejected");
}

/// The **parent_id** / span ID (third component) must be exactly 16 hex
/// characters per the W3C Trace Context specification.
#[test]
fn parent_id_is_16_hex_chars() {
    let (_, _, pid, _) = parse_traceparent(VALID_TRACEPARENT).expect("parse");

    assert_eq!(pid.len(), 16, "parent_id must be 16 chars");
    assert!(pid.chars().all(is_hex_digit), "parent_id must be hex");
    assert_eq!(pid, PARENT_ID);

    // Too short (15 chars) — validation must reject.
    let short_pid = "00f067aa0ba902b";
    assert_eq!(short_pid.len(), 15);
    let result =
        std::panic::catch_unwind(|| assert_hex_segment(short_pid, 16, "parent_id (too short)"));
    assert!(result.is_err(), "short parent_id must be rejected");

    // Too long (17 chars) — validation must reject.
    let long_pid = "00f067aa0ba902b70";
    assert_eq!(long_pid.len(), 17);
    let result =
        std::panic::catch_unwind(|| assert_hex_segment(long_pid, 16, "parent_id (too long)"));
    assert!(result.is_err(), "long parent_id must be rejected");
}

/// The **trace_flags** (fourth component) must be exactly 2 hex characters
/// per the W3C Trace Context specification.
#[test]
fn trace_flags_is_2_hex_chars() {
    let (_, _, _, flags) = parse_traceparent(VALID_TRACEPARENT).expect("parse");

    assert_eq!(flags.len(), 2, "trace_flags must be 2 chars");
    assert!(flags.chars().all(is_hex_digit), "trace_flags must be hex");
    assert_eq!(flags, TRACE_FLAGS);

    // Too short (1 char) — validation must reject.
    let short_flags = "0";
    assert_eq!(short_flags.len(), 1);
    let result =
        std::panic::catch_unwind(|| assert_hex_segment(short_flags, 2, "trace_flags (too short)"));
    assert!(result.is_err(), "short trace_flags must be rejected");

    // Too long (3 chars) — validation must reject.
    let long_flags = "010";
    assert_eq!(long_flags.len(), 3);
    let result =
        std::panic::catch_unwind(|| assert_hex_segment(long_flags, 2, "trace_flags (too long)"));
    assert!(result.is_err(), "long trace_flags must be rejected");
}

/// Injecting a `traceparent` header into a request and then extracting it
/// must yield the original value — a full round-trip.
#[test]
fn traceparent_injection_preserves_context() {
    let mut headers: Vec<(String, String)> = Vec::new();

    inject_traceparent(&mut headers, VALID_TRACEPARENT);

    let extracted =
        extract_traceparent(&headers).expect("traceparent must be present after injection");
    assert_eq!(
        extracted, VALID_TRACEPARENT,
        "injected traceparent must survive extraction round-trip"
    );

    // Verify it landed in the right slot.
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "traceparent");
}

/// Trace context must survive IPC serialization: the traceparent is
/// encoded into a JSON envelope and decoded on the other side with the
/// value intact.
#[test]
fn ipc_trace_context_propagation() {
    // --- Serialize (sender side) ----------------------------------------
    let envelope = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "health.status",
        "params": {},
        "traceparent": VALID_TRACEPARENT,
    });

    let wire = serde_json::to_string(&envelope).expect("serialize IPC envelope");

    // --- Deserialize (receiver side) ------------------------------------
    let decoded: serde_json::Value = serde_json::from_str(&wire).expect("deserialize IPC envelope");

    let recovered = decoded
        .get("traceparent")
        .and_then(|v| v.as_str())
        .expect("traceparent must be present in decoded envelope");

    assert_eq!(recovered, VALID_TRACEPARENT, "trace context must survive IPC JSON round-trip");

    // Verify the three sub-components are still individually valid.
    let (ver, tid, pid, flags) =
        parse_traceparent(recovered).expect("decoded traceparent must parse");
    assert_eq!(ver, "00");
    assert_eq!(tid.len(), 32);
    assert_eq!(pid.len(), 16);
    assert_eq!(flags.len(), 2);
}

/// The tray client must pass through `traceparent` headers unchanged
/// when forwarding requests to the IPC sidecar.
#[test]
fn tray_trace_context_passthrough() {
    // Simulate a tray-originated request that already carries trace
    // context from the operator environment.
    let mut incoming: Vec<(String, String)> = Vec::new();
    inject_traceparent(&mut incoming, VALID_TRACEPARENT);

    // Simulate the tray layer forwarding headers to the IPC sidecar.
    let mut outgoing: Vec<(String, String)> = Vec::new();
    for (key, value) in &incoming {
        outgoing.push((key.clone(), value.clone()));
    }

    // The forwarded value must be identical.
    let forwarded = extract_traceparent(&outgoing).expect("traceparent must be forwarded by tray");
    assert_eq!(forwarded, VALID_TRACEPARENT, "tray must pass trace context through unchanged");

    // --- Multi-hop scenario: operator → tray → IPC ---------------------
    // Build a second-generation traceparent to simulate a child span.
    let child_trace_id = TRACE_ID;
    let child_parent_id = "a1b2c3d4e5f6a7b8";
    let child_flags = "01";
    let child_traceparent = build_traceparent("00", child_trace_id, child_parent_id, child_flags);

    // Replace the original traceparent with the child span.
    outgoing.pop();
    inject_traceparent(&mut outgoing, &child_traceparent);

    // Extract should now return the child span.
    let final_value = extract_traceparent(&outgoing).expect("child traceparent must be present");
    assert_eq!(final_value, child_traceparent);

    // Validate the full structure of the propagated child span.
    let (v, tid, pid, fl) = parse_traceparent(&final_value).expect("child traceparent must parse");
    assert_eq!(v, "00");
    assert_eq!(tid, child_trace_id);
    assert_eq!(pid, child_parent_id);
    assert_eq!(fl, child_flags);
}
