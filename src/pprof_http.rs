//! Opt-in CPU profiling HTTP surface for `sharecli serve` (audit-v38 L45).
//!
//! Enable with `SHARECLI_PPROF=1`. Profile capture is Unix-only (`pprof` crate);
//! Windows returns 501 so the route still appears in OpenAPI / ops docs.
//!
//! Output is Google `profile.proto` (pprof protobuf), not SVG flamegraphs — the
//! `flamegraph` feature pulls `inferno` → vulnerable `quick-xml` < 0.41.

use axum::extract::Query;
#[cfg(unix)]
use axum::http::header;
use axum::http::StatusCode;
#[cfg(unix)]
use axum::response::IntoResponse;
use axum::response::Response;
use serde::Deserialize;

use crate::error_envelope::ErrorEnvelope;

/// Parse `SHARECLI_PPROF` style values (`None`/`""`/`"0"` ⇒ off).
pub fn pprof_enabled_from(val: Option<&str>) -> bool {
    matches!(val, Some(v) if !v.is_empty() && v != "0")
}

/// True when `SHARECLI_PPROF` is set to a non-empty, non-`0` value.
pub fn pprof_enabled() -> bool {
    pprof_enabled_from(std::env::var("SHARECLI_PPROF").ok().as_deref())
}

#[derive(Debug, Deserialize)]
pub struct ProfileQuery {
    /// Capture window in seconds (default 10, hard-capped at 60).
    #[serde(default = "default_seconds")]
    pub seconds: u64,
}

fn default_seconds() -> u64 {
    10
}

/// `GET /debug/pprof/profile` — protobuf CPU profile of a short sample.
///
/// Requires `SHARECLI_PPROF=1`. Honors serve Bearer auth when configured
/// (route is not public).
pub async fn profile_handler(Query(q): Query<ProfileQuery>) -> Response {
    if !pprof_enabled() {
        return ErrorEnvelope::not_found("profiling disabled; set SHARECLI_PPROF=1 to enable")
            .into_response(StatusCode::NOT_FOUND);
    }

    let seconds = q.seconds.clamp(1, 60);

    #[cfg(unix)]
    {
        match capture_pprof_protobuf(seconds).await {
            Ok(bytes) => {
                (StatusCode::OK, [(header::CONTENT_TYPE, "application/vnd.google.protobuf")], bytes)
                    .into_response()
            }
            Err(err) => {
                tracing::warn!("pprof capture failed: {err}");
                ErrorEnvelope::internal().into_response(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = seconds;
        ErrorEnvelope::not_implemented(
            "CPU profiling via pprof is Unix-only on this build; use samply/perf externally (see docs/ops/profiling.md)",
        )
        .into_response(StatusCode::NOT_IMPLEMENTED)
    }
}

#[cfg(unix)]
async fn capture_pprof_protobuf(seconds: u64) -> Result<Vec<u8>, String> {
    use prost::Message;

    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .map_err(|e| e.to_string())?;

    tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;

    tokio::task::spawn_blocking(move || {
        let report = guard.report().build().map_err(|e| e.to_string())?;
        let profile = report.pprof().map_err(|e| e.to_string())?;
        let mut body = Vec::new();
        profile.encode(&mut body).map_err(|e| e.to_string())?;
        Ok(body)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pprof_parse_disabled_cases() {
        assert!(!pprof_enabled_from(None));
        assert!(!pprof_enabled_from(Some("")));
        assert!(!pprof_enabled_from(Some("0")));
    }

    #[test]
    fn profile_query_defaults_to_ten_seconds() {
        assert_eq!(default_seconds(), 10);
        let query: ProfileQuery = serde_json::from_str("{}").expect("empty query");
        assert_eq!(query.seconds, 10);
        let explicit: ProfileQuery = serde_json::from_str(r#"{"seconds": 30}"#).expect("explicit");
        assert_eq!(explicit.seconds, 30);
    }

    #[tokio::test]
    async fn profile_handler_is_404_when_the_env_flag_is_off() {
        let previous = std::env::var("SHARECLI_PPROF").ok();
        std::env::remove_var("SHARECLI_PPROF");

        let response = profile_handler(Query(ProfileQuery { seconds: 10 })).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        if let Some(value) = previous {
            std::env::set_var("SHARECLI_PPROF", value);
        }
    }

    #[test]
    fn pprof_parse_enabled_cases() {
        assert!(pprof_enabled_from(Some("1")));
        assert!(pprof_enabled_from(Some("true")));
        assert!(pprof_enabled_from(Some("yes")));
    }
}
