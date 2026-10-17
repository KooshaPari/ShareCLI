//! JWT HS256 sign / verify + b64url helpers used by gateway-tools.
//!
//! HS256 is implemented in terms of `sha2::Sha256` only (no `hmac` crate, to
//! sidestep the digest 0.10 vs 0.11 cross-version issue that surfaces in
//! multi-crate workspaces). The implementation follows RFC 2104 directly.

use sha2::{Digest, Sha256};

/// Standard base64url (no padding) encoding/decoding.
pub fn b64url_encode(input: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input)
}

pub fn b64url_decode(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(input)
}

/// Encode a JWT: `header_b64.payload_b64` + `HMAC-SHA256(header.payload, key)` signature.
///
/// Inputs are pre-encoded JSON header/payload strings; we re-encode them as
/// b64url internally so callers can pass raw JSON.
pub fn encode_hs256(header: &str, payload: &str, key: &[u8]) -> String {
    let h_b64 = b64url_encode(header.as_bytes());
    let p_b64 = b64url_encode(payload.as_bytes());
    let signing_input = format!("{h_b64}.{p_b64}");
    let sig = hmac_sha256(key, signing_input.as_bytes());
    let s_b64 = b64url_encode(&sig);
    format!("{signing_input}.{s_b64}")
}

/// Verify a JWT and return `(header_str, payload_str)` on success.
pub fn verify_hs256(
    token: &str,
    key: &[u8],
) -> Result<(String, String), &'static str> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("invalid token shape (expected 3 parts)");
    }
    let (h_b64, p_b64, s_b64) = (parts[0], parts[1], parts[2]);
    let signing_input = format!("{h_b64}.{p_b64}");
    let expected = hmac_sha256(key, signing_input.as_bytes());
    let provided = b64url_decode(s_b64).map_err(|_| "bad signature b64")?;
    if expected.as_slice() != provided.as_slice() {
        return Err("signature mismatch");
    }
    let header_bytes = b64url_decode(h_b64).map_err(|_| "bad header b64")?;
    let payload_bytes = b64url_decode(p_b64).map_err(|_| "bad payload b64")?;
    let header = String::from_utf8(header_bytes).map_err(|_| "header not utf-8")?;
    let payload = String::from_utf8(payload_bytes).map_err(|_| "payload not utf-8")?;
    Ok((header, payload))
}

/// HMAC-SHA256 per RFC 2104:
///
///   H(K' ⊕ opad || H(K' ⊕ ipad || message))
///
/// where `K'` is the key normalized to the SHA-256 block length (64 bytes):
///   - if `len(K) == block_len`, use as-is
///   - if `len(K) < block_len`, right-pad with zero bytes
///   - if `len(K) > block_len`, replace with H(K) (which is exactly 32 bytes < 64)
fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    const BLOCK_LEN: usize = 64;
    const OPAD: u8 = 0x5c;
    const IPAD: u8 = 0x36;

    let mut normalized = [0u8; BLOCK_LEN];
    if key.len() > BLOCK_LEN {
        let hashed = Sha256::digest(key);
        normalized[..hashed.len()].copy_from_slice(&hashed);
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }

    let mut inner = Vec::with_capacity(BLOCK_LEN + message.len());
    for b in normalized.iter() {
        inner.push(b ^ IPAD);
    }
    inner.extend_from_slice(message);
    let inner_hash = Sha256::digest(&inner);

    let mut outer = Vec::with_capacity(BLOCK_LEN + inner_hash.len());
    for b in normalized.iter() {
        outer.push(b ^ OPAD);
    }
    outer.extend_from_slice(&inner_hash);
    Sha256::digest(&outer).to_vec()
}
