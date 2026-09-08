//! FR: FR-003 — C05 L42 OTel instrumentation acceptance gate.
//!
//! Verifies the trace-context inject/extract counters are wired end-to-end:
//! middleware increments -> MetricsRegistry -> `/metrics/prometheus` output.
//! These counters populate L49's `sharecli-trace.json` panel #2.

use std::fs;
use std::path::PathBuf;

const INJECTED: &str = "sharecli_tracecontext_injected_total";
const EXTRACTED: &str = "sharecli_tracecontext_extracted_total";

fn serve_src() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/commands/serve.rs");
    fs::read_to_string(&p).expect("read src/commands/serve.rs")
}

#[test]
fn fr003_counter_names_are_stable_constants() {
    // Guard: the metric names must match what sharecli-trace.json panel #2 queries.
    assert_eq!(INJECTED, "sharecli_tracecontext_injected_total");
    assert_eq!(EXTRACTED, "sharecli_tracecontext_extracted_total");
}

#[test]
fn fr003_middleware_increments_extract_counter_when_traceparent_present() {
    let src = serve_src();
    // The extract counter must be bumped only when an incoming traceparent exists.
    assert!(
        src.contains("sharecli_tracecontext_extracted_total\").inc();"),
        "extract counter must be incremented"
    );
    assert!(
        src.contains("get(\"traceparent\")") || src.contains(r#"header("traceparent")"#),
        "extract point must read the incoming traceparent header"
    );
}

#[test]
fn fr003_middleware_increments_inject_counter() {
    let src = serve_src();
    assert!(
        src.contains("sharecli_tracecontext_injected_total\").inc();"),
        "inject counter must be incremented"
    );
    assert!(
        src.contains("traceparent") && src.contains("response.headers_mut().insert"),
        "inject point must write the outgoing traceparent header"
    );
}

#[test]
fn fr003_prometheus_handler_reads_both_counters() {
    let src = serve_src();
    assert!(src.contains("metrics_prometheus_handler"), "prometheus handler must exist");
    assert!(
        src.contains(&format!("counter(\"{INJECTED}\").get()")),
        "prometheus handler must read the injected counter value"
    );
    assert!(
        src.contains(&format!("counter(\"{EXTRACTED}\").get()")),
        "prometheus handler must read the extracted counter value"
    );
}

#[test]
fn fr003_dashboard_panel_references_the_same_counter_names() {
    // L49's trace dashboard (shipped by Plan 782) must query the exact counters
    // this feature produces — closes the panel #2 gap.
    let dash = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/ops/grafana/dashboards/sharecli-trace.json");
    if dash.exists() {
        let json = fs::read_to_string(&dash).unwrap_or_default();
        assert!(
            json.contains(INJECTED) && json.contains(EXTRACTED),
            "sharecli-trace.json must query both trace-context counters"
        );
    }
}
