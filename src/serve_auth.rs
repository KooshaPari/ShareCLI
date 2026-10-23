//! AuthN for `sharecli serve`: open, static Bearer, or federated JWT (JWKS).
//!
//! Modes:
//! - **open** — no token required (loopback trust model).
//! - **bearer** — `SHARECLI_SERVE_TOKEN` or `config.serve.bearer_token`.
//! - **jwt** — validate `Authorization: Bearer <JWT>` against configured
//!   issuer/audience and a JWKS document (`jwks_path` or inline `jwks`).
//!
//! `/healthz` and `/readyz` stay public in all modes.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet, KeyAlgorithm};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::{ServeConfig, ServeJwtConfig};
use crate::error_envelope::{auth_failure_message, ErrorEnvelope};

/// Shared auth state cloned into the Axum router.
#[derive(Clone, Debug)]
pub struct ServeAuth {
    mode: AuthMode,
}

#[derive(Clone, Debug)]
enum AuthMode {
    Open,
    Bearer { token: String },
    Jwt(Arc<JwtValidator>),
}

#[derive(Clone)]
struct JwtValidator {
    issuer: String,
    audience: String,
    /// `(kid, key, alg)` entries from JWKS. Empty `kid` matches any header kid.
    keys: Vec<(Option<String>, DecodingKey, Algorithm)>,
}

impl std::fmt::Debug for JwtValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtValidator")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("keys", &self.keys.len())
            .finish()
    }
}

fn key_alg_to_algorithm(ka: KeyAlgorithm) -> Result<Algorithm, String> {
    match ka {
        KeyAlgorithm::RS256 => Ok(Algorithm::RS256),
        KeyAlgorithm::RS384 => Ok(Algorithm::RS384),
        KeyAlgorithm::RS512 => Ok(Algorithm::RS512),
        KeyAlgorithm::ES256 => Ok(Algorithm::ES256),
        KeyAlgorithm::ES384 => Ok(Algorithm::ES384),
        other => Err(format!("unsupported JWK alg {other:?} (HS*/PS* denied)")),
    }
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    sub: Option<String>,
}

impl ServeAuth {
    /// Resolve mode: env bearer token wins; else config `auth_mode` / jwt / bearer.
    pub fn from_env_or_config(serve: &ServeConfig) -> Result<Self, String> {
        if let Ok(token) = std::env::var("SHARECLI_SERVE_TOKEN") {
            if !token.is_empty() {
                return Ok(Self { mode: AuthMode::Bearer { token } });
            }
        }

        let mode_hint = std::env::var("SHARECLI_SERVE_AUTH_MODE")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| serve.auth_mode.clone())
            .map(|s| s.to_ascii_lowercase());

        match mode_hint.as_deref() {
            Some("jwt") => {
                let jwt = resolve_jwt_config(serve)?;
                let validator = JwtValidator::from_config(&jwt)?;
                Ok(Self { mode: AuthMode::Jwt(Arc::new(validator)) })
            }
            Some("bearer") => {
                let token =
                    serve.bearer_token.clone().filter(|s| !s.is_empty()).ok_or_else(|| {
                        "auth_mode=bearer requires serve.bearer_token or SHARECLI_SERVE_TOKEN"
                            .to_string()
                    })?;
                Ok(Self { mode: AuthMode::Bearer { token } })
            }
            Some("open") | None => {
                if let Some(token) = serve.bearer_token.as_ref().filter(|s| !s.is_empty()) {
                    return Ok(Self { mode: AuthMode::Bearer { token: token.clone() } });
                }
                if serve.jwt.is_some() && mode_hint.as_deref() == Some("open") {
                    return Ok(Self { mode: AuthMode::Open });
                }
                // Auto-enable jwt when jwt block present and no bearer.
                if serve.jwt.is_some() && mode_hint.is_none() {
                    let jwt = resolve_jwt_config(serve)?;
                    let validator = JwtValidator::from_config(&jwt)?;
                    return Ok(Self { mode: AuthMode::Jwt(Arc::new(validator)) });
                }
                Ok(Self { mode: AuthMode::Open })
            }
            Some(other) => {
                Err(format!("unknown serve.auth_mode `{other}` (expected open|bearer|jwt)"))
            }
        }
    }

    /// Backward-compatible helper used by older call sites / tests.
    #[allow(dead_code)]
    pub fn from_env_or_token(config_token: Option<&str>) -> Self {
        let token = std::env::var("SHARECLI_SERVE_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| config_token.map(str::to_owned).filter(|s| !s.is_empty()));
        match token {
            Some(token) => Self { mode: AuthMode::Bearer { token } },
            None => Self { mode: AuthMode::Open },
        }
    }

    pub fn enabled(&self) -> bool {
        !matches!(self.mode, AuthMode::Open)
    }

    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            AuthMode::Open => "open",
            AuthMode::Bearer { .. } => "bearer",
            AuthMode::Jwt(_) => "jwt",
        }
    }

    pub fn check_authorization(&self, header_val: Option<&str>) -> Result<Option<String>, String> {
        match &self.mode {
            AuthMode::Open => Ok(None),
            AuthMode::Bearer { token } => {
                let Some(raw) = header_val else {
                    return Err("missing_authorization".into());
                };
                let Some(provided) =
                    raw.strip_prefix("Bearer ").or_else(|| raw.strip_prefix("bearer "))
                else {
                    return Err("not_bearer".into());
                };
                if tokens_equal(provided.trim(), token) {
                    Ok(None)
                } else {
                    Err("invalid_bearer".into())
                }
            }
            AuthMode::Jwt(v) => {
                let Some(raw) = header_val else {
                    return Err("missing_authorization".into());
                };
                let Some(provided) =
                    raw.strip_prefix("Bearer ").or_else(|| raw.strip_prefix("bearer "))
                else {
                    return Err("not_bearer".into());
                };
                v.validate(provided.trim()).map(Some)
            }
        }
    }

    /// Legacy name retained for unit tests.
    #[allow(dead_code)]
    pub fn check_bearer(&self, header_val: Option<&str>) -> bool {
        self.check_authorization(header_val).is_ok()
    }
}

fn resolve_jwt_config(serve: &ServeConfig) -> Result<ServeJwtConfig, String> {
    let mut jwt = serve.jwt.clone().ok_or_else(|| {
        "auth_mode=jwt requires [serve.jwt] with issuer, audience, and jwks_path or jwks"
            .to_string()
    })?;
    if let Ok(iss) = std::env::var("SHARECLI_SERVE_JWT_ISSUER") {
        if !iss.is_empty() {
            jwt.issuer = iss;
        }
    }
    if let Ok(aud) = std::env::var("SHARECLI_SERVE_JWT_AUDIENCE") {
        if !aud.is_empty() {
            jwt.audience = aud;
        }
    }
    if let Ok(path) = std::env::var("SHARECLI_SERVE_JWKS_PATH") {
        if !path.is_empty() {
            jwt.jwks_path = Some(path);
        }
    }
    if jwt.issuer.is_empty() || jwt.audience.is_empty() {
        return Err("serve.jwt requires non-empty issuer and audience".into());
    }
    if jwt.jwks_path.is_none() && jwt.jwks.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        return Err("serve.jwt requires jwks_path or inline jwks".into());
    }
    Ok(jwt)
}

impl JwtValidator {
    fn from_config(cfg: &ServeJwtConfig) -> Result<Self, String> {
        let jwks_json = if let Some(inline) = cfg.jwks.as_ref().filter(|s| !s.is_empty()) {
            inline.clone()
        } else if let Some(path) = cfg.jwks_path.as_ref() {
            std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read JWKS at {path}: {e}"))?
        } else {
            return Err("serve.jwt requires jwks_path or inline jwks".into());
        };
        let set: JwkSet =
            serde_json::from_str(&jwks_json).map_err(|e| format!("invalid JWKS JSON: {e}"))?;
        if set.keys.is_empty() {
            return Err("JWKS contains no keys".into());
        }
        let mut keys = Vec::new();
        for jwk in &set.keys {
            let kid = jwk.common.key_id.clone();
            let (decoding_key, default_alg) = match &jwk.algorithm {
                AlgorithmParameters::RSA(params) => {
                    let key = DecodingKey::from_rsa_components(&params.n, &params.e)
                        .map_err(|e| format!("RSA JWK decode failed: {e}"))?;
                    (key, Algorithm::RS256)
                }
                AlgorithmParameters::EllipticCurve(params) => {
                    let key = DecodingKey::from_ec_components(&params.x, &params.y)
                        .map_err(|e| format!("EC JWK decode failed: {e}"))?;
                    (key, Algorithm::ES256)
                }
                other => {
                    return Err(format!(
                        "unsupported JWK algorithm parameters {other:?} (HS* denied)"
                    ));
                }
            };
            let alg = match jwk.common.key_algorithm {
                Some(ka) => key_alg_to_algorithm(ka)?,
                None => default_alg,
            };
            if !matches!(
                alg,
                Algorithm::RS256
                    | Algorithm::RS384
                    | Algorithm::RS512
                    | Algorithm::ES256
                    | Algorithm::ES384
            ) {
                return Err(format!("unsupported alg {alg:?}"));
            }
            keys.push((kid, decoding_key, alg));
        }
        Ok(Self { issuer: cfg.issuer.clone(), audience: cfg.audience.clone(), keys })
    }

    fn validate(&self, token: &str) -> Result<String, String> {
        let header = decode_header(token).map_err(|e| format!("jwt_header:{e}"))?;
        let candidates: Vec<_> = self
            .keys
            .iter()
            .filter(|(kid, _, alg)| {
                if header.alg != *alg {
                    return false;
                }
                match (&header.kid, kid) {
                    (Some(h), Some(k)) => h == k,
                    (Some(_), None) => false,
                    (None, _) => true,
                }
            })
            .collect();
        if candidates.is_empty() {
            return Err("jwt_no_matching_key".into());
        }

        let mut last_err = "jwt_invalid".to_string();
        for (_, key, alg) in candidates {
            let mut validation = Validation::new(*alg);
            validation.set_issuer(&[&self.issuer]);
            validation.set_audience(&[&self.audience]);
            validation.leeway = 60;
            validation.validate_exp = true;
            validation.validate_nbf = true;
            match decode::<JwtClaims>(token, key, &validation) {
                Ok(data) => {
                    return Ok(data.claims.sub.unwrap_or_else(|| "unknown".into()));
                }
                Err(e) => {
                    last_err = map_jwt_error(&e);
                }
            }
        }
        Err(last_err)
    }
}

fn map_jwt_error(err: &jsonwebtoken::errors::Error) -> String {
    use jsonwebtoken::errors::ErrorKind;
    match err.kind() {
        ErrorKind::ExpiredSignature => "jwt_expired".into(),
        ErrorKind::InvalidIssuer => "jwt_invalid_iss".into(),
        ErrorKind::InvalidAudience => "jwt_invalid_aud".into(),
        ErrorKind::ImmatureSignature => "jwt_nbf".into(),
        ErrorKind::InvalidSignature => "jwt_invalid_sig".into(),
        _ => format!("jwt_invalid:{err}"),
    }
}

fn tokens_equal(a: &str, b: &str) -> bool {
    let da = Sha256::digest(a.as_bytes());
    let db = Sha256::digest(b.as_bytes());
    da.as_slice() == db.as_slice()
}

/// Paths that remain reachable without AuthN (liveness/readiness).
pub fn is_public_path(path: &str) -> bool {
    matches!(path, "/healthz" | "/readyz") || crate::dashboard_assets::is_dashboard_asset_path(path)
}

/// Axum middleware: enforce bearer or JWT when configured.
pub async fn require_bearer(
    axum::extract::State(auth): axum::extract::State<ServeAuth>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_owned();
    if is_public_path(&path) || !auth.enabled() {
        return next.run(req).await;
    }

    let header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());

    match auth.check_authorization(header) {
        Ok(sub) => {
            let mut body = json!({ "path": path, "mode": auth.mode_label() });
            if let Some(sub) = sub {
                body["sub"] = json!(sub);
            }
            crate::audit_log::emit("auth_ok", body);
            next.run(req).await
        }
        Err(reason) => {
            crate::audit_log::emit(
                "auth_fail",
                json!({
                    "path": path,
                    "mode": auth.mode_label(),
                    "reason": reason,
                }),
            );
            let envelope = ErrorEnvelope::unauthorized(auth_failure_message(&reason));
            (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer")], Json(envelope))
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use serde::Deserialize;

    use super::*;

    /// Serialize env mutations — llvm-cov runs lib tests in parallel.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Deserialize)]
    struct Fixture {
        jwks: serde_json::Value,
        tokens: Tokens,
    }

    #[derive(Deserialize)]
    struct Tokens {
        valid: [String; 3],
        expired: [String; 3],
        bad_aud: [String; 3],
        bad_iss: [String; 3],
    }

    fn join_jwt(parts: &[String; 3]) -> String {
        format!("{}.{}.{}", parts[0], parts[1], parts[2])
    }

    fn fixture() -> &'static Fixture {
        static FIX: OnceLock<Fixture> = OnceLock::new();
        FIX.get_or_init(|| {
            serde_json::from_str(include_str!("../tests/fixtures/jwt_static.json"))
                .expect("jwt_static.json")
        })
    }

    fn jwt_auth() -> ServeAuth {
        let jwt = ServeJwtConfig {
            issuer: "https://idp.example/".into(),
            audience: "sharecli-serve".into(),
            jwks_path: None,
            jwks: Some(fixture().jwks.to_string()),
        };
        ServeAuth { mode: AuthMode::Jwt(Arc::new(JwtValidator::from_config(&jwt).expect("jwks"))) }
    }

    #[test]
    fn public_paths() {
        assert!(is_public_path("/healthz"));
        assert!(is_public_path("/readyz"));
        assert!(is_public_path("/assets/dashboard/ui/favicons/phenotype.ico"));
        assert!(!is_public_path("/metrics/prometheus"));
        assert!(!is_public_path("/config"));
    }

    #[test]
    fn auth_disabled_allows_any() {
        // Construct Open directly — do not call from_env_or_token under parallel llvm-cov.
        let auth = ServeAuth { mode: AuthMode::Open };
        assert!(auth.check_bearer(None));
        assert!(auth.check_bearer(Some("Bearer nope")));
        assert_eq!(auth.mode_label(), "open");
    }

    #[test]
    fn auth_enabled_requires_matching_bearer() {
        // Construct Bearer directly so SHARECLI_SERVE_TOKEN races cannot poison assertions.
        let auth = ServeAuth { mode: AuthMode::Bearer { token: "s3cret".into() } };
        assert!(!auth.check_bearer(None));
        assert!(!auth.check_bearer(Some("Bearer wrong")));
        assert!(!auth.check_bearer(Some("Basic s3cret")));
        assert!(auth.check_bearer(Some("Bearer s3cret")));
        assert!(auth.check_bearer(Some("bearer s3cret")));
    }

    #[test]
    fn env_overrides_config_bearer() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("SHARECLI_SERVE_TOKEN", "from-env");
        }
        let cfg = ServeConfig {
            bearer_token: Some("from-config".into()),
            auth_mode: None,
            jwt: None,
            ..ServeConfig::default()
        };
        let auth = ServeAuth::from_env_or_config(&cfg).unwrap();
        assert!(auth.check_bearer(Some("Bearer from-env")));
        unsafe {
            std::env::remove_var("SHARECLI_SERVE_TOKEN");
        }
        let auth = ServeAuth::from_env_or_config(&cfg).unwrap();
        assert!(auth.check_bearer(Some("Bearer from-config")));
    }

    #[test]
    fn jwt_valid_rs256() {
        let auth = jwt_auth();
        let token = join_jwt(&fixture().tokens.valid);
        let sub = auth.check_authorization(Some(&format!("Bearer {token}"))).expect("valid");
        assert_eq!(sub.as_deref(), Some("user-1"));
    }

    #[test]
    fn jwt_expired_rejected() {
        let auth = jwt_auth();
        let token = join_jwt(&fixture().tokens.expired);
        let err = auth.check_authorization(Some(&format!("Bearer {token}"))).unwrap_err();
        assert_eq!(err, "jwt_expired");
    }

    #[test]
    fn jwt_wrong_audience_rejected() {
        let auth = jwt_auth();
        let token = join_jwt(&fixture().tokens.bad_aud);
        let err = auth.check_authorization(Some(&format!("Bearer {token}"))).unwrap_err();
        assert_eq!(err, "jwt_invalid_aud");
    }

    #[test]
    fn jwt_wrong_issuer_rejected() {
        let auth = jwt_auth();
        let token = join_jwt(&fixture().tokens.bad_iss);
        let err = auth.check_authorization(Some(&format!("Bearer {token}"))).unwrap_err();
        assert_eq!(err, "jwt_invalid_iss");
    }

    #[test]
    fn auth_failure_envelope_matches_contract() {
        use crate::error_envelope::{auth_failure_message, ErrorEnvelope};

        let body = ErrorEnvelope::unauthorized(auth_failure_message("missing_authorization"));
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["error"]["type"], "authentication_error");
        assert_eq!(json["error"]["code"], "unauthorized");
        assert_eq!(json["error"]["message"], "missing or invalid bearer token");
        assert!(json["error"]["request_id"].is_null());
    }

    #[test]
    fn jwt_bad_signature_rejected() {
        // Flip a single trailing byte of the valid signature to invalidate it.
        // Same header/payload, only the signature segment is mutated.
        let auth = jwt_auth();
        let parts = fixture().tokens.valid.clone();
        let mut sig = parts[2].clone();
        // The signature is base64url; flip the last char to a different valid
        // base64url alphabet member so re-encoding still parses but the bytes differ.
        let last = sig.pop().unwrap();
        let replacement = if last == 'A' { 'B' } else { 'A' };
        sig.push(replacement);
        let token = format!("{}.{}.{}", parts[0], parts[1], sig);
        let err = auth
            .check_authorization(Some(&format!("Bearer {token}")))
            .unwrap_err();
        assert!(
            err == "jwt_invalid_sig" || err.starts_with("jwt_invalid:"),
            "expected signature-mapped error, got {err}"
        );
    }

    #[test]
    fn jwt_garbage_token_rejected() {
        let auth = jwt_auth();
        // A string with two dots looks structurally like a JWT but the payload
        // is not valid base64/JSON for our claims; header decode must fail.
        let err = auth
            .check_authorization(Some("Bearer this-is-not-a-jwt"))
            .unwrap_err();
        assert!(
            err.starts_with("jwt_header:") || err.starts_with("jwt_invalid"),
            "expected header-decode error, got {err}"
        );
    }

    #[test]
    fn jwt_empty_token_rejected() {
        let auth = jwt_auth();
        let err = auth
            .check_authorization(Some("Bearer "))
            .unwrap_err();
        assert!(
            err.starts_with("jwt_header:") || err.starts_with("jwt_invalid"),
            "expected header-decode error for empty token, got {err}"
        );
    }

    #[test]
    fn jwt_malformed_header_segment_rejected() {
        let auth = jwt_auth();
        // Header segment is not valid base64url -> decode_header fails.
        let payload = fixture().tokens.valid[1].clone();
        let sig = fixture().tokens.valid[2].clone();
        let bad_header = "***not_base64***";
        let token = format!("{bad_header}.{payload}.{sig}");
        let err = auth
            .check_authorization(Some(&format!("Bearer {token}")))
            .unwrap_err();
        assert!(
            err.starts_with("jwt_header:") || err.starts_with("jwt_invalid"),
            "expected malformed-header error, got {err}"
        );
    }

    #[test]
    fn serve_auth_unknown_mode_hint_returns_error() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure no overrides are set.
        unsafe {
            std::env::remove_var("SHARECLI_SERVE_TOKEN");
            std::env::remove_var("SHARECLI_SERVE_AUTH_MODE");
        }
        let cfg = ServeConfig {
            auth_mode: Some("bogus".into()),
            bearer_token: None,
            jwt: None,
            ..ServeConfig::default()
        };
        let err = ServeAuth::from_env_or_config(&cfg).unwrap_err();
        assert_eq!(
            err,
            "unknown serve.auth_mode `bogus` (expected open|bearer|jwt)"
        );
    }
}
