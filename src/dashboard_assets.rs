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

    #[test]
    fn dashboard_asset_path_prefix() {
        assert!(is_dashboard_asset_path("/assets/dashboard/ui/favicons/phenotype.ico"));
        assert!(!is_dashboard_asset_path("/metrics/prometheus"));
    }
}
