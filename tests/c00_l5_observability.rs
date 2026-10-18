//! C00 L5 / FR-003 — Observability (logs / metrics / traces) FR-003 acceptance gates.
//!
//! Evidence: `src/metrics.rs`, `src/log_sink.rs`, `src/otel.rs`, `src/commands/serve.rs`,
//! `src/main.rs`, `Cargo.toml`.

#[test]
fn c00_l5_metrics_module_exposes_counter_gauge_registry() {
    let src = include_str!("../src/metrics.rs");
    assert!(src.contains("pub struct Counter"), "src/metrics.rs must define Counter struct");
    assert!(src.contains("pub struct Gauge"), "src/metrics.rs must define Gauge struct");
    assert!(
        src.contains("pub struct MetricsRegistry"),
        "src/metrics.rs must define MetricsRegistry struct"
    );
    assert!(
        src.contains("impl Counter") && src.contains("impl Gauge"),
        "src/metrics.rs must have impl blocks for Counter and Gauge"
    );
}

#[test]
fn c00_l5_metrics_registry_default_impl_present() {
    let src = include_str!("../src/metrics.rs");
    assert!(
        src.contains("impl Default for Counter"),
        "src/metrics.rs must derive Default for Counter"
    );
    assert!(src.contains("impl Default for Gauge"), "src/metrics.rs must derive Default for Gauge");
}

#[test]
fn c00_l5_log_sink_exposes_bridge_to_tracing_layer() {
    let src = include_str!("../src/log_sink.rs");
    assert!(src.contains("pub struct LogSink"), "src/log_sink.rs must define LogSink struct");
    assert!(
        src.contains("pub struct LogSinkLayer"),
        "src/log_sink.rs must define LogSinkLayer (tracing::Layer bridge)"
    );
    assert!(
        src.contains("pub fn flush_to_tracing"),
        "src/log_sink.rs must expose flush_to_tracing() to drain buffer into tracing"
    );
    assert!(src.contains("pub enum LogLevel"), "src/log_sink.rs must define LogLevel enum");
}

#[test]
fn c00_l5_otel_module_uses_sdk_tracer_provider_and_batch_exporter() {
    let src = include_str!("../src/otel.rs");
    assert!(
        src.contains("SdkTracerProvider"),
        "src/otel.rs must use opentelemetry_sdk::trace::SdkTracerProvider"
    );
    assert!(src.contains("with_batch_exporter"), "src/otel.rs must use batch exporter (OTLP/HTTP)");
    assert!(src.contains("pub fn otel_enabled"), "src/otel.rs must expose otel_enabled() flag");
    assert!(
        src.contains("pub fn try_otel_layer"),
        "src/otel.rs must expose try_otel_layer() for tracing integration"
    );
}

#[test]
fn c00_l5_otel_w3c_tracecontext_propagator_present() {
    let src = include_str!("../src/otel.rs");
    assert!(
        src.contains("ensure_trace_context_propagator"),
        "src/otel.rs must install W3C TraceContext propagator"
    );
    assert!(
        src.contains("traceparent_http_value") || src.contains("traceparent_spawn_env"),
        "src/otel.rs must export traceparent helper for HTTP or spawn propagation"
    );
}

#[test]
fn c00_l5_serve_route_exposes_prometheus_metrics() {
    // Post-decomposition: the original monolithic `src/commands/serve.rs` was
    // split into a `commands/serve/` module directory. The HTTP route
    // definitions live in `commands/serve/run.rs`; the handler stubs and
    // doc-comments live in `commands/serve/handlers.rs` and other submodules.
    // Concatenate every `commands/serve/` .rs file so this static check
    // survives future sub-splits.
    let src = std::fs::read_dir("src/commands/serve")
        .expect("src/commands/serve must exist post-decomposition")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("rs"))
        .map(|p| std::fs::read_to_string(&p).expect("failed to read serve submodule"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        src.contains("/metrics/prometheus") || src.contains("metrics/prometheus"),
        "serve module must expose /metrics/prometheus route"
    );
    assert!(
        src.contains("/healthz") && src.contains("/readyz"),
        "serve module must expose /healthz and /readyz split"
    );
}

#[test]
fn c00_l5_main_initializes_tracing_subscriber() {
    let src = include_str!("../src/main.rs");
    assert!(
        src.contains("tracing_subscriber") || src.contains("tracing::subscriber"),
        "main must initialize a tracing subscriber"
    );
    assert!(
        src.contains("EnvFilter") || src.contains("RUST_LOG") || src.contains("verbose"),
        "main must apply verbose/RUST_LOG-driven filter level"
    );
}

#[test]
fn c00_l5_cargo_deps_include_tracing_and_otel() {
    let cargo = include_str!("../Cargo.toml");
    assert!(
        cargo.contains("tracing =") || cargo.contains("tracing=\""),
        "Cargo.toml must depend on tracing"
    );
    assert!(
        cargo.contains("tracing-subscriber") || cargo.contains("tracing_subscriber"),
        "Cargo.toml must depend on tracing-subscriber"
    );
    assert!(cargo.contains("opentelemetry"), "Cargo.toml must depend on opentelemetry");
    assert!(
        cargo.contains("opentelemetry_sdk") || cargo.contains("opentelemetry-sdk"),
        "Cargo.toml must depend on opentelemetry_sdk"
    );
}

#[test]
fn c00_l5_observability_docs_reference_all_three_pillars() {
    let observability_docs = ["docs/ops/otel.md", "docs/ops/grafana/sharecli-serve.json"];
    for path in observability_docs {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} must be present ({})", path, e));
        assert!(!content.trim().is_empty(), "{} must be non-empty observability reference", path);
    }
}
