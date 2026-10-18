// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Minimal DNS packet parser: 12-byte header + a single question section.
//!
//! Used by `gateway-tools dns` for parse + print. The full RFC-1035 surface
//! (compression pointers, multi-question packets, etc.) is out of scope for
//! this MVP.

#[derive(Debug, Clone, PartialEq)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: u16,
    pub qd_count: u16,
    pub an_count: u16,
    pub ns_count: u16,
    pub ar_count: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub qname: String,
    pub qtype: u16,
    pub qclass: u16,
}

#[derive(Debug)]
pub enum DnsError {
    TooShort { need: usize, got: usize },
    BadLabel { at: usize },
}

impl std::fmt::Display for DnsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DnsError::TooShort { need, got } => {
                write!(f, "dns: too short (need {need}, got {got})")
            }
            DnsError::BadLabel { at } => write!(f, "dns: bad label at byte {at}"),
        }
    }
}

impl std::error::Error for DnsError {}

pub fn parse_header(buf: &[u8]) -> Result<DnsHeader, DnsError> {
    if buf.len() < 12 {
        return Err(DnsError::TooShort { need: 12, got: buf.len() });
    }
    Ok(DnsHeader {
        id: u16::from_be_bytes([buf[0], buf[1]]),
        flags: u16::from_be_bytes([buf[2], buf[3]]),
        qd_count: u16::from_be_bytes([buf[4], buf[5]]),
        an_count: u16::from_be_bytes([buf[6], buf[7]]),
        ns_count: u16::from_be_bytes([buf[8], buf[9]]),
        ar_count: u16::from_be_bytes([buf[10], buf[11]]),
    })
}

/// Parse exactly one question starting at `offset`. Returns the question and
/// the total bytes consumed (including qname + 4 bytes qtype/qclass).
pub fn parse_question(buf: &[u8], offset: usize) -> Result<(Question, usize), DnsError> {
    let mut i = offset;
    let mut labels = Vec::new();
    loop {
        if i >= buf.len() {
            return Err(DnsError::BadLabel { at: i });
        }
        let len = buf[i] as usize;
        i += 1;
        if len == 0 {
            break;
        }
        if i + len > buf.len() {
            return Err(DnsError::BadLabel { at: i });
        }
        labels
            .push(std::str::from_utf8(&buf[i..i + len]).map_err(|_| DnsError::BadLabel { at: i })?);
        i += len;
    }
    if i + 4 > buf.len() {
        return Err(DnsError::BadLabel { at: i });
    }
    let qtype = u16::from_be_bytes([buf[i], buf[i + 1]]);
    let qclass = u16::from_be_bytes([buf[i + 2], buf[i + 3]]);
    i += 4;
    Ok((Question { qname: labels.join("."), qtype, qclass }, i - offset))
}

/// Inverse of `parse_question`: encode a qname + qtype + qclass into a buffer.
/// Used by tests / fixtures that need to construct packets. Format:
/// label-length-prefixed ASCII labels + 0x00 + 2 bytes qtype + 2 bytes qclass.
pub fn encode_question(q: &Question) -> Vec<u8> {
    let mut out = Vec::new();
    for label in q.qname.split('.') {
        let bytes = label.as_bytes();
        out.push(bytes.len() as u8);
        out.extend_from_slice(bytes);
    }
    out.push(0);
    out.extend_from_slice(&q.qtype.to_be_bytes());
    out.extend_from_slice(&q.qclass.to_be_bytes());
    out
}
