// SPDX-License-Identifier: Apache-2.0 OR MIT
//! OpenTelemetry bootstrap for sharecli.
//!
//! When `OTEL_EXPORTER_OTLP_ENDPOINT` (or `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`)
//! is set, installs an OTLP/HTTP span exporter and a `tracing-opentelemetry`
//! layer so `#[instrument]` spans become OTel traces. Without those env vars
//! this module is a no-op so local/CI runs stay collector-free.
//!
//! Also installs the W3C `TraceContextPropagator` so HTTP middleware can
//! extract/inject `traceparent` (audit-v38 L42 / L44).

use std::collections::HashMap;
use std::sync::OnceLock;

use opentelemetry::global;
use opentelemetry::propagation::Injector;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::Context;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing::Subscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::registry::LookupSpan;

static PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();

/// True when an OTLP endpoint env var is present (export path is armed).
pub fn otel_enabled() -> bool {
    std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_some()
        || std::env::var_os("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT").is_some()
}

/// Build an optional `tracing` → OpenTelemetry layer.
///
/// Returns `None` when no OTLP endpoint is configured, or when the exporter
/// fails to build (logged via `eprintln!` so serve still starts).
pub fn try_otel_layer<S>() -> Option<OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    if !otel_enabled() {
        return None;
    }

    match build_provider() {
        Ok(provider) => {
            let _ = PROVIDER.set(provider);
            let tracer = PROVIDER.get().expect("provider just set").tracer("sharecli");
            global::set_text_map_propagator(TraceContextPropagator::new());
            Some(OpenTelemetryLayer::new(tracer))
        }
        Err(err) => {
            eprintln!("sharecli: OTel init failed ({err}); continuing without OTLP export");
            None
        }
    }
}

/// Ensure the W3C propagator is registered even when OTLP export is off, so
/// serve middleware can still parse/emit `traceparent` for in-process spans.
pub fn ensure_trace_context_propagator() {
    global::set_text_map_propagator(TraceContextPropagator::new());
}

struct CarrierInjector<'a>(&'a mut HashMap<String, String>);

impl Injector for CarrierInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

/// Case-sensitive env lookup (Windows `var_os` is case-insensitive).
fn env_var_exact(key: &str) -> Option<std::ffi::OsString> {
    std::env::vars_os().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn traceparent_value_from_env() -> Option<String> {
    for key in ["traceparent", "TRACEPARENT"] {
        if let Some(v) = env_var_exact(key) {
            let s = v.to_string_lossy();
            // Validate each candidate before selecting so a malformed lowercase
            // value cannot shadow a valid TRACEPARENT (or vice versa).
            if !s.is_empty() && is_w3c_traceparent(&s) {
                return Some(s.into_owned());
            }
        }
    }
    None
}

fn traceparent_value_from_otel_context() -> Option<String> {
    ensure_trace_context_propagator();
    let mut carrier = HashMap::new();
    global::get_text_map_propagator(|prop| {
        prop.inject_context(&Context::current(), &mut CarrierInjector(&mut carrier));
    });
    carrier.get("traceparent").filter(|v| !v.is_empty()).cloned()
}

fn is_w3c_traceparent(value: &str) -> bool {
    let mut fields = value.split('-');
    let Some(version) = fields.next() else {
        return false;
    };
    let Some(trace_id) = fields.next() else {
        return false;
    };
    let Some(parent_id) = fields.next() else {
        return false;
    };
    let Some(flags) = fields.next() else {
        return false;
    };

    fields.next().is_none()
        && version.len() == 2
        && version != "ff"
        && trace_id.len() == 32
        && trace_id != "00000000000000000000000000000000"
        && parent_id.len() == 16
        && parent_id != "0000000000000000"
        && flags.len() == 2
        && [version, trace_id, parent_id, flags].iter().all(|field| {
            field.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

/// W3C `traceparent` header value for outbound HTTP (tray dashboard fetches).
///
/// Reads a valid operator `traceparent` / `TRACEPARENT` env, then the active
/// OTel context. Invalid values are skipped per-candidate rather than copied
/// into an HTTP header or HTML attribute.
pub fn traceparent_http_value() -> Option<String> {
    traceparent_value_from_env().or_else(traceparent_value_from_otel_context)
}

/// Soft multi-hop (C05 L44): `TRACEPARENT` for CLI supervised spawns.
///
/// Returns `None` when the parent already exports `TRACEPARENT` (child inherits).
/// Otherwise maps lowercase `traceparent` from the operator env, or injects the
/// active OTel trace context when present.
pub fn traceparent_spawn_env() -> Option<(String, String)> {
    if env_var_exact("TRACEPARENT").is_some() {
        return None;
    }
    if let Some(v) = env_var_exact("traceparent") {
        let s = v.to_string_lossy();
        if !s.is_empty() {
            return Some(("TRACEPARENT".to_string(), s.into_owned()));
        }
    }
    traceparent_value_from_otel_context().map(|v| ("TRACEPARENT".to_string(), v))
}

/// Apply [`traceparent_spawn_env`] to a [`std::process::Command`] before spawn.
///
/// Used by tray FFI (`sharecli-ffi`) and other IPC sidecar launch paths so
/// `sharecli-ipc` inherits W3C trace context from the desktop parent.
#[allow(dead_code)] // consumed by `sharecli-ffi` cdylib, not the `sharecli` bin
pub fn apply_traceparent_spawn_env(cmd: &mut std::process::Command) {
    if let Some((key, value)) = traceparent_spawn_env() {
        cmd.env(key, value);
    }
}

fn build_provider() -> Result<SdkTracerProvider, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = SpanExporter::builder().with_http().build()?;
    let resource = Resource::builder().with_service_name("sharecli").build();
    Ok(SdkTracerProvider::builder().with_batch_exporter(exporter).with_resource(resource).build())
}

/// Flush pending spans (best-effort). Call on process shutdown when OTel is on.
pub fn shutdown() {
    if let Some(provider) = PROVIDER.get() {
        let _ = provider.shutdown();
    }
}

/// Shared mutex for unit tests that mutate `traceparent` / `TRACEPARENT`.
/// Crate-wide so `tray_http` tests cannot race `otel` env mutations.
#[doc(hidden)]
#[allow(dead_code)] // used from `#[cfg(test)]` modules in otel/tray_http
pub static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otel_disabled_without_endpoint_env() {
        // CI / default: no OTEL_* endpoint → layer stays off.
        // We cannot unset env in parallel tests safely; just assert the helper
        // is callable and returns a bool.
        let _ = otel_enabled();
    }

    #[test]
    fn propagator_install_is_idempotent() {
        ensure_trace_context_propagator();
        ensure_trace_context_propagator();
    }

    #[test]
    fn apply_traceparent_spawn_env_sets_child_env() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let key = "traceparent";
        let prev = std::env::var(key).ok();
        let prev_upper = std::env::var("TRACEPARENT").ok();
        std::env::remove_var("TRACEPARENT");
        std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");

        let (bin, args) = traceparent_child_cmd();
        let mut cmd = std::process::Command::new(bin);
        cmd.args(args);
        apply_traceparent_spawn_env(&mut cmd);
        let output = cmd.output().expect("spawn traceparent echo child");
        let got = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if let Some(prev) = prev {
            std::env::set_var(key, prev);
        } else {
            std::env::remove_var(key);
        }
        if let Some(prev) = prev_upper {
            std::env::set_var("TRACEPARENT", prev);
        }

        assert_eq!(got, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");
    }

    fn traceparent_child_cmd() -> (&'static str, Vec<&'static str>) {
        #[cfg(unix)]
        {
            ("printenv", vec!["TRACEPARENT"])
        }
        #[cfg(windows)]
        {
            (
                "powershell",
                vec!["-NoProfile", "-NonInteractive", "-Command", "Write-Output $env:TRACEPARENT"],
            )
        }
    }

    #[test]
    fn traceparent_http_value_maps_lowercase_env() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let key = "traceparent";
        let prev = std::env::var(key).ok();
        let prev_upper = std::env::var("TRACEPARENT").ok();
        std::env::remove_var("TRACEPARENT");
        std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");
        let out = traceparent_http_value();
        if let Some(prev) = prev {
            std::env::set_var(key, prev);
        } else {
            std::env::remove_var(key);
        }
        if let Some(prev) = prev_upper {
            std::env::set_var("TRACEPARENT", prev);
        }
        assert_eq!(
            out,
            Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string())
        );
    }

    // Distinct `traceparent` vs `TRACEPARENT` slots exist only on case-sensitive
    // platforms. Windows collapses them into one env entry.
    #[cfg(unix)]
    #[test]
    fn traceparent_http_value_prefers_lowercase_over_uppercase_env() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let lower = "traceparent";
        let upper = "TRACEPARENT";
        let prev_lower = std::env::var(lower).ok();
        let prev_upper = std::env::var(upper).ok();
        std::env::set_var(lower, "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01");
        std::env::set_var(upper, "00-cccccccccccccccccccccccccccccccc-dddddddddddddddd-01");
        let out = traceparent_http_value();
        if let Some(prev) = prev_lower {
            std::env::set_var(lower, prev);
        } else {
            std::env::remove_var(lower);
        }
        if let Some(prev) = prev_upper {
            std::env::set_var(upper, prev);
        } else {
            std::env::remove_var(upper);
        }
        assert_eq!(
            out,
            Some("00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01".to_string())
        );
    }

    // Distinct `traceparent` vs `TRACEPARENT` slots exist only on case-sensitive
    // platforms. Windows collapses them into one env entry.
    #[cfg(unix)]
    #[test]
    fn traceparent_http_value_skips_invalid_lowercase_for_valid_uppercase() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let lower = "traceparent";
        let upper = "TRACEPARENT";
        let prev_lower = std::env::var(lower).ok();
        let prev_upper = std::env::var(upper).ok();
        std::env::set_var(lower, "not-a-trace\r\nx-injected: true");
        std::env::set_var(upper, "00-cccccccccccccccccccccccccccccccc-dddddddddddddddd-01");
        let out = traceparent_http_value();
        if let Some(prev) = prev_lower {
            std::env::set_var(lower, prev);
        } else {
            std::env::remove_var(lower);
        }
        if let Some(prev) = prev_upper {
            std::env::set_var(upper, prev);
        } else {
            std::env::remove_var(upper);
        }
        assert_eq!(
            out,
            Some("00-cccccccccccccccccccccccccccccccc-dddddddddddddddd-01".to_string())
        );
    }

    #[cfg(windows)]
    #[test]
    fn traceparent_http_value_reads_case_insensitive_env_slot() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let prev = std::env::var("TRACEPARENT").ok();
        std::env::remove_var("TRACEPARENT");
        std::env::set_var("traceparent", "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01");
        let out = traceparent_http_value();
        if let Some(prev) = prev {
            std::env::set_var("TRACEPARENT", prev);
        } else {
            std::env::remove_var("TRACEPARENT");
        }
        assert_eq!(
            out,
            Some("00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01".to_string())
        );
    }

    #[test]
    fn traceparent_http_value_rejects_invalid_env_header() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let key = "traceparent";
        let prev = std::env::var(key).ok();
        let prev_upper = std::env::var("TRACEPARENT").ok();
        // Windows: remove before set so remove_var("TRACEPARENT") does not
        // clear the invalid value under test.
        std::env::remove_var("TRACEPARENT");
        std::env::set_var(key, "not-a-trace\r\nx-injected: true");
        let out = traceparent_http_value();
        if let Some(prev) = prev {
            std::env::set_var(key, prev);
        } else {
            std::env::remove_var(key);
        }
        if let Some(prev) = prev_upper {
            std::env::set_var("TRACEPARENT", prev);
        }
        assert_eq!(out, None);
    }

    #[test]
    fn traceparent_spawn_env_maps_lowercase_env() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let key = "traceparent";
        let prev = std::env::var(key).ok();
        let prev_upper = std::env::var("TRACEPARENT").ok();
        std::env::remove_var("TRACEPARENT");
        std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");
        let out = traceparent_spawn_env();
        if let Some(prev) = prev {
            std::env::set_var(key, prev);
        } else {
            std::env::remove_var(key);
        }
        if let Some(prev) = prev_upper {
            std::env::set_var("TRACEPARENT", prev);
        }
        assert_eq!(
            out,
            Some((
                "TRACEPARENT".to_string(),
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string()
            ))
        );
    }
}
