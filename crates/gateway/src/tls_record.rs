//! TLS 1.x record parse + write helpers. Parses one record header
//! (`type || version || length`) and returns a view onto the payload bytes.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContentType {
    ChangeCipherSpec,
    Alert,
    Handshake,
    ApplicationData,
}

impl ContentType {
    pub fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            20 => ContentType::ChangeCipherSpec,
            21 => ContentType::Alert,
            22 => ContentType::Handshake,
            23 => ContentType::ApplicationData,
            _ => return None,
        })
    }

    pub fn to_byte(&self) -> u8 {
        match self {
            ContentType::ChangeCipherSpec => 20,
            ContentType::Alert => 21,
            ContentType::Handshake => 22,
            ContentType::ApplicationData => 23,
        }
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ContentType::ChangeCipherSpec => "ChangeCipherSpec",
            ContentType::Alert => "Alert",
            ContentType::Handshake => "Handshake",
            ContentType::ApplicationData => "ApplicationData",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

pub const VERSION_TLS_1_2: ProtocolVersion = ProtocolVersion { major: 3, minor: 3 };

#[derive(Debug, Clone, Copy)]
pub enum TlsError {
    TooShort { need: usize, got: usize },
    UnknownContentType(u8),
    BadLength { declared: usize, available: usize },
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsError::TooShort { need, got } => write!(f, "tls: too short (need {need}, got {got})"),
            TlsError::UnknownContentType(b) => write!(f, "tls: unknown content type {b}"),
            TlsError::BadLength { declared, available } => {
                write!(f, "tls: declared len {declared} > available {available}")
            }
        }
    }
}

impl std::error::Error for TlsError {}

#[derive(Debug, Clone)]
pub struct TlsRecord<'a> {
    pub content_type: ContentType,
    pub version: ProtocolVersion,
    pub payload: &'a [u8],
}

pub fn parse_record(buf: &[u8]) -> Result<TlsRecord<'_>, TlsError> {
    if buf.len() < 5 {
        return Err(TlsError::TooShort {
            need: 5,
            got: buf.len(),
        });
    }
    let ct = ContentType::from_byte(buf[0]).ok_or(TlsError::UnknownContentType(buf[0]))?;
    let version = ProtocolVersion {
        major: buf[1],
        minor: buf[2],
    };
    let len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
    if buf.len() < 5 + len {
        return Err(TlsError::BadLength {
            declared: len,
            available: buf.len() - 5,
        });
    }
    Ok(TlsRecord {
        content_type: ct,
        version,
        payload: &buf[5..5 + len],
    })
}

/// Append a record header + payload to `buf`. The payload is copied.
pub fn write_record(kind: ContentType, version: ProtocolVersion, payload: &[u8], buf: &mut Vec<u8>) {
    buf.push(kind.to_byte());
    buf.push(version.major);
    buf.push(version.minor);
    buf.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    buf.extend_from_slice(payload);
}
