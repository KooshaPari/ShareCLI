// SPDX-License-Identifier: Apache-2.0 OR MIT
//! klipdot observability helpers — thin wrapper around pheno-tracing (ADR-012, ADR-036).
//!
//! Pheno-tracing is a port-driven tracing substrate. Consumers build a
//! TraceOperation and submit it to a TracePort (e.g. StdoutAdapter for
//! local dev, an OTLP collector adapter for production). The macros and
//! attribute macros are re-exported from `pheno_tracing::compat`, which
//! itself re-exports from the upstream `tracing` crate.

use pheno_tracing::{
    adapters::StdoutAdapter,
    port::{SpanId, SpanKind, TraceId, TraceOperation, TracePort},
};
use std::collections::HashMap;
use std::sync::Arc;

/// Service name used as the OpenTelemetry `service.name` resource attribute.
pub const SERVICE_NAME: &str = "klipdot";

/// Default OTLP gRPC endpoint if `OTEL_EXPORTER_OTLP_ENDPOINT` is unset.
pub const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4317";

/// Returns the configured OTLP endpoint (env override or local default).
pub fn otlp_endpoint() -> String {
    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_OTLP_ENDPOINT.to_string())
}

/// Build a new `TraceOperation` from a service-relative span name and
/// arbitrary string attributes.
pub fn build_span(
    trace_id: &str,
    span_id: &str,
    parent_span_id: Option<&str>,
    name: &str,
    kind: SpanKind,
    attributes: HashMap<String, String>,
) -> TraceOperation {
    TraceOperation {
        trace_id: TraceId(trace_id.to_string()),
        span_id: SpanId(span_id.to_string()),
        parent_span_id: parent_span_id.map(SpanId),
        kind,
        name: name.to_string(),
        attributes,
    }
}

/// Generate a deterministic-enough trace id (timestamp + process counter).
pub fn next_trace_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("klipdot-trace-{}-{}", ts, n)
}

/// Submit a span through the configured [`TracePort`].
pub async fn submit_span(op: TraceOperation) {
    let port: Arc<dyn TracePort> = Arc::new(StdoutAdapter);
    let _ = port.submit(op).await;
    let _ = port.flush().await;
}

/// Convenience: build + submit a span in one call.
pub async fn emit_span(name: &str, attributes: HashMap<String, String>) {
    let trace_id = next_trace_id();
    let op = build_span(
        &trace_id,
        &format!("{}-{}", name, trace_id),
        None,
        name,
        SpanKind::Internal,
        attributes,
    );
    submit_span(op).await;
}

// Re-export the `tracing` macro family via pheno-tracing's compat layer.
pub use pheno_tracing::compat::{debug, error, info, instrument, span, trace, warn};
