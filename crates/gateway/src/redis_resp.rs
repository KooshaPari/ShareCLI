//! Redis RESP2 encoder + parser. Single-value encode + parse, used by
//! `gateway-tools redis`.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Option<Vec<RespValue>>),
}

#[derive(Debug)]
pub struct RespError(pub String);

impl fmt::Display for RespError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resp: {}", self.0)
    }
}

impl std::error::Error for RespError {}

/// Encode a RESP value into its wire form.
pub fn encode(v: &RespValue) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(v, &mut out);
    out
}

fn encode_into(v: &RespValue, out: &mut Vec<u8>) {
    match v {
        RespValue::SimpleString(s) => {
            out.push(b'+');
            out.extend_from_slice(s.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        RespValue::Error(s) => {
            out.push(b'-');
            out.extend_from_slice(s.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        RespValue::Integer(n) => {
            out.push(b':');
            out.extend_from_slice(n.to_string().as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        RespValue::BulkString(None) => {
            out.extend_from_slice(b"$-1\r\n");
        }
        RespValue::BulkString(Some(bytes)) => {
            out.push(b'$');
            out.extend_from_slice(bytes.len().to_string().as_bytes());
            out.extend_from_slice(b"\r\n");
            out.extend_from_slice(bytes);
            out.extend_from_slice(b"\r\n");
        }
        RespValue::Array(None) => {
            out.extend_from_slice(b"*-1\r\n");
        }
        RespValue::Array(Some(items)) => {
            out.push(b'*');
            out.extend_from_slice(items.len().to_string().as_bytes());
            out.extend_from_slice(b"\r\n");
            for item in items {
                encode_into(item, out);
            }
        }
    }
}

pub fn format_bulk(s: &str) -> Vec<u8> {
    encode(&RespValue::BulkString(Some(s.as_bytes().to_vec())))
}

pub fn format_array(items: &[RespValue]) -> Vec<u8> {
    encode(&RespValue::Array(Some(items.to_vec())))
}

/// Parse one complete RESP value from `input`. Returns `(value, bytes_consumed)`.
pub fn parse(input: &[u8]) -> Result<(RespValue, usize), RespError> {
    let mut cur = 0;
    let v = parse_one(input, &mut cur)?;
    Ok((v, cur))
}

fn parse_one(input: &[u8], cur: &mut usize) -> Result<RespValue, RespError> {
    if *cur >= input.len() {
        return Err(RespError("unexpected eof".into()));
    }
    let ty = input[*cur];
    *cur += 1;
    match ty {
        b'+' => read_line(input, cur).map(RespValue::SimpleString),
        b'-' => read_line(input, cur).map(RespValue::Error),
        b':' => read_line(input, cur)
            .and_then(|s| s.parse::<i64>().map_err(|e| RespError(format!("bad int: {e}"))))
            .map(RespValue::Integer),
        b'$' => {
            let n = read_line(input, cur)?
                .parse::<isize>()
                .map_err(|e| RespError(format!("bad bulk len: {e}")))?;
            if n == -1 {
                Ok(RespValue::BulkString(None))
            } else if n < 0 {
                Err(RespError(format!("bad bulk len: {n}")))
            } else {
                let n = n as usize;
                if *cur + n + 2 > input.len() {
                    return Err(RespError("bulk truncated".into()));
                }
                let bytes = input[*cur..*cur + n].to_vec();
                *cur += n + 2;
                Ok(RespValue::BulkString(Some(bytes)))
            }
        }
        b'*' => {
            let n = read_line(input, cur)?
                .parse::<isize>()
                .map_err(|e| RespError(format!("bad array len: {e}")))?;
            if n == -1 {
                Ok(RespValue::Array(None))
            } else if n < 0 {
                Err(RespError(format!("bad array len: {n}")))
            } else {
                let n = n as usize;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(parse_one(input, cur)?);
                }
                Ok(RespValue::Array(Some(items)))
            }
        }
        other => Err(RespError(format!("unsupported RESP type byte `{}`", other as char))),
    }
}

fn read_line(input: &[u8], cur: &mut usize) -> Result<String, RespError> {
    let start = *cur;
    let mut end = start;
    while end < input.len() && input[end] != b'\r' {
        end += 1;
    }
    if end + 1 >= input.len() {
        return Err(RespError("missing CRLF".into()));
    }
    if input[end + 1] != b'\n' {
        return Err(RespError("bad CRLF".into()));
    }
    let s = std::str::from_utf8(&input[start..end])
        .map_err(|e| RespError(format!("line not utf-8: {e}")))?
        .to_string();
    *cur = end + 2;
    Ok(s)
}
