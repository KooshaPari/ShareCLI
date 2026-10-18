//! HTTP/1.1 chunked transfer-encoding helpers, surfaced as plain bytes (not
//! the wire CRLF-prefixed text). The `gateway-tools chunked` subcommand wraps
//! `encode_chunks` with hex chunk-size headers and appends the `0\r\n\r\n`
//! terminator, but the underlying `Vec<Vec<u8>>` representation is what the
//! binary actually emits/reads.

pub fn encode_chunks(data: &[u8], chunk: usize) -> Vec<Vec<u8>> {
    if chunk == 0 {
        // Degenerate input — treat the whole payload as a single chunk so
        // round-trip decode still succeeds.
        return vec![data.to_vec()];
    }
    let mut out = Vec::with_capacity((data.len() + chunk - 1) / chunk);
    for slice in data.chunks(chunk) {
        out.push(slice.to_vec());
    }
    out
}

pub fn decode_chunks(chunks: &[Vec<u8>]) -> Vec<u8> {
    let total: usize = chunks.iter().map(|c| c.len()).sum();
    let mut out = Vec::with_capacity(total);
    for c in chunks {
        out.extend_from_slice(c);
    }
    out
}

/// `0` for the terminating chunk, otherwise the lowercase hex size followed
/// by `;gateway-tools=ok` extension tag. The actual wire chunk-size line
/// in `gateway-tools chunked` is just `format!("{}{}", hex_chunk_header(n), "\r\n")`.
pub fn hex_chunk_header(n: usize) -> String {
    if n == 0 {
        "0".to_string()
    } else {
        format!("{:x}", n)
    }
}

pub fn last_zero_chunk() -> &'static str {
    "0\r\n\r\n"
}

pub fn parse_chunk_size(line: &str) -> Result<usize, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty chunk size line".into());
    }
    let end = line.find(';').map(|i| line[..i].trim()).unwrap_or(line);
    usize::from_str_radix(end, 16).map_err(|e| format!("bad hex size `{end}`: {e}"))
}
