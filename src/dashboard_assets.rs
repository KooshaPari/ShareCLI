//! Embedded Phenotype UI pack assets for `sharecli serve` (`assets/dashboard/ui/`).
//!
//! Served at `/assets/dashboard/ui/*` so the dashboard HTML can reference favicons,
//! banners, and empty-state art without external CDN dependencies.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

/// URL prefix for embedded dashboard UI assets.
pub const URL_PREFIX: &str = "/assets/dashboard/ui";

struct EmbeddedAsset {
    bytes: &'static [u8],
    content_type: &'static str,
}

fn lookup(relative_path: &str) -> Option<EmbeddedAsset> {
    Some(match relative_path {
        "favicons/phenotype.ico" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/favicons/phenotype.ico"),
            content_type: "image/x-icon",
        },
        "favicons/phenotype_16.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/favicons/phenotype_16.png"),
            content_type: "image/png",
        },
        "favicons/phenotype_32.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/favicons/phenotype_32.png"),
            content_type: "image/png",
        },
        "favicons/phenotype_64.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/favicons/phenotype_64.png"),
            content_type: "image/png",
        },
        "favicons/phenotype_128.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/favicons/phenotype_128.png"),
            content_type: "image/png",
        },
        "banners/dashboard_1280x320.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/banners/dashboard_1280x320.png"),
            content_type: "image/png",
        },
        "empty-states/no-data.svg" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/empty-states/no-data.svg"),
            content_type: "image/svg+xml",
        },
        "empty-states/no-data.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/empty-states/no-data.png"),
            content_type: "image/png",
        },
        "empty-states/no-results.svg" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/empty-states/no-results.svg"),
            content_type: "image/svg+xml",
        },
        "empty-states/no-results.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/empty-states/no-results.png"),
            content_type: "image/png",
        },
        "empty-states/error.svg" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/empty-states/error.svg"),
            content_type: "image/svg+xml",
        },
        "empty-states/error.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/empty-states/error.png"),
            content_type: "image/png",
        },
        "error-states/disconnect.svg" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/error-states/disconnect.svg"),
            content_type: "image/svg+xml",
        },
        "icons/phenotype_icon.png" => EmbeddedAsset {
            bytes: include_bytes!("../assets/dashboard/ui/icons/phenotype_icon.png"),
            content_type: "image/png",
        },
        "dashboard.js" => EmbeddedAsset {
            bytes: include_bytes!("dashboard.js"),
            content_type: "application/javascript",
        },
        _ => return None,
    })
}

/// `true` when `path` is an embedded dashboard UI asset URL.
pub fn is_dashboard_asset_path(path: &str) -> bool {
    path.starts_with(URL_PREFIX)
}

/// Axum handler for `GET /assets/dashboard/ui/{*path}`.
pub async fn serve(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    let Some(asset) = lookup(path.trim_start_matches('/')) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut response = Response::new(Body::from(asset.bytes));
    *response.status_mut() = StatusCode::OK;
    if let Ok(val) = HeaderValue::from_str(asset.content_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, val);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_favicon_and_empty_states_resolve() {
        assert!(lookup("favicons/phenotype_32.png").is_some());
        assert!(lookup("empty-states/no-data.svg").is_some());
        assert!(lookup("error-states/disconnect.svg").is_some());
        assert!(lookup("banners/dashboard_1280x320.png").is_some());
        assert!(lookup("dashboard.js").is_some());
        assert!(lookup("video/brand_intro.mp4").is_none());
    }

    #[tokio::test]
    async fn serve_returns_the_embedded_bytes_and_content_type() {
        let response = serve(axum::extract::Path("favicons/phenotype_32.png".to_string())).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
            Some("image/png")
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
        assert_eq!(
            body.as_ref(),
            lookup("favicons/phenotype_32.png").expect("asset").bytes,
            "served bytes must be the embedded asset, not a re-encoded copy"
        );
    }

    #[tokio::test]
    async fn serve_uses_the_javascript_content_type_for_the_dashboard_script() {
        let response = serve(axum::extract::Path("dashboard.js".to_string())).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
            Some("application/javascript")
        );
    }

    #[tokio::test]
    async fn serve_accepts_a_leading_slash_and_rejects_unknown_assets() {
        let ok = serve(axum::extract::Path("/empty-states/error.svg".to_string())).await;
        assert_eq!(ok.status(), StatusCode::OK, "a leading slash must be trimmed");

        let missing = serve(axum::extract::Path("video/brand_intro.mp4".to_string())).await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn every_embedded_asset_resolves_with_a_non_empty_body() {
        for path in [
            "favicons/phenotype.ico",
            "favicons/phenotype_16.png",
            "favicons/phenotype_32.png",
            "favicons/phenotype_64.png",
            "favicons/phenotype_128.png",
            "banners/dashboard_1280x320.png",
            "empty-states/no-data.svg",
            "empty-states/no-data.png",
            "empty-states/no-results.svg",
            "empty-states/no-results.png",
            "empty-states/error.svg",
            "empty-states/error.png",
            "error-states/disconnect.svg",
            "icons/phenotype_icon.png",
            "dashboard.js",
        ] {
            let asset = lookup(path).unwrap_or_else(|| panic!("{path} must be embedded"));
            assert!(!asset.bytes.is_empty(), "{path} must carry bytes");
            assert!(!asset.content_type.is_empty(), "{path} must declare a content type");
        }
    }

    #[test]
    fn dashboard_asset_path_prefix() {
        assert!(is_dashboard_asset_path("/assets/dashboard/ui/favicons/phenotype.ico"));
        assert!(!is_dashboard_asset_path("/metrics/prometheus"));
    }
}
