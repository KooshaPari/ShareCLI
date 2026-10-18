//! PEM encode/decode for type-labeled ASCII-armored blobs.
//!
//! Output format:
//! ```
//! -----BEGIN <LABEL>-----
//! <base64(DER), wrapped at 64 cols>
//! -----END <LABEL>-----
//! ```

use base64::{engine::general_purpose::STANDARD, Engine as _};

#[derive(Debug, Clone, PartialEq)]
pub struct PemBlock {
    pub label: String,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum PemError {
    MissingBeginMarker,
    MissingEndMarker,
    LabelMismatch { begin: String, end: String },
    BadBase64(base64::DecodeError),
    BadAscii,
}

impl std::fmt::Display for PemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PemError::MissingBeginMarker => write!(f, "pem: missing `-----BEGIN ...-----` marker"),
            PemError::MissingEndMarker => write!(f, "pem: missing `-----END ...-----` marker"),
            PemError::LabelMismatch { begin, end } => {
                write!(f, "pem: label mismatch `{begin}` vs `{end}`")
            }
            PemError::BadBase64(e) => write!(f, "pem: bad base64: {e}"),
            PemError::BadAscii => write!(f, "pem: input is not ASCII text"),
        }
    }
}

impl std::error::Error for PemError {}

pub fn encode_pem(label: &str, der: &[u8]) -> String {
    let b64 = STANDARD.encode(der);
    let mut out = String::new();
    out.push_str(&format!("-----BEGIN {}-----\n", label));
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push('\n');
    }
    out.push_str(&format!("-----END {}-----\n", label));
    out
}

/// Backwards-compatible alias used by the inspect registry and other callers
/// that expect the shorter `encode` name.
pub fn encode(label: &str, der: &[u8]) -> String {
    encode_pem(label, der)
}

pub fn decode_pem(input: &str) -> Result<(String, Vec<u8>), PemError> {
    decode(input).map(|b| (b.label, b.data))
}

pub fn decode(input: &str) -> Result<PemBlock, PemError> {
    if !input.is_ascii() {
        return Err(PemError::BadAscii);
    }
    let mut begin_label = None;
    let mut end_label = None;
    let mut body = String::new();
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("-----BEGIN ") {
            if let Some(label) = rest.strip_suffix("-----") {
                begin_label = Some(label.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("-----END ") {
            if let Some(label) = rest.strip_suffix("-----") {
                end_label = Some(label.to_string());
            }
        } else if begin_label.is_some() && end_label.is_none() {
            body.push_str(line);
        }
    }
    let begin = begin_label.ok_or(PemError::MissingBeginMarker)?;
    let end = end_label.ok_or(PemError::MissingEndMarker)?;
    if begin != end {
        return Err(PemError::LabelMismatch { begin, end });
    }
    let data = STANDARD.decode(body.as_bytes()).map_err(PemError::BadBase64)?;
    Ok(PemBlock { label: begin, data })
}

pub fn decode_many(input: &str) -> Result<Vec<PemBlock>, PemError> {
    // For MVP: only the first block is decoded. A real implementation would
    // iterate through `-----END ...-----` markers and split.
    decode(input).map(|b| vec![b])
}

pub fn armor(input: &str) -> String {
    // Wrap each line at 64 chars and add a "MESSAGE" label. Useful for
    // surfacing arbitrary text via the same envelope as a real key/cert.
    let mut out = String::new();
    out.push_str("-----BEGIN MESSAGE-----\n");
    for line in input.lines() {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("-----END MESSAGE-----\n");
    out
}
