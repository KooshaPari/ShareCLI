//! PKCS#7 padding for AES/block ciphers (RFC 5652 §6.3).
//!
//! `pad(data, block)` always appends at least one byte. `unpad` validates
//! that the trailing count byte is in 1..=block_size AND that the trailing
//! `count` bytes all equal that value.

pub const PKCS7_ERR: &str = "invalid PKCS#7 padding";

pub fn required_padding_len(len: usize, block: usize) -> usize {
    let rem = len % block;
    if rem == 0 {
        block
    } else {
        block - rem
    }
}

pub fn pad(data: &[u8], block: usize) -> Vec<u8> {
    assert!(block > 0, "block size must be > 0");
    let n = required_padding_len(data.len(), block);
    let mut out = Vec::with_capacity(data.len() + n);
    out.extend_from_slice(data);
    for _ in 0..n {
        out.push(n as u8);
    }
    out
}

pub fn validate_pkcs7(data: &[u8], block: usize) -> bool {
    unpad(data, block).is_ok()
}

pub fn unpad(data: &[u8], block: usize) -> Result<&[u8], &'static str> {
    if data.is_empty() || block == 0 {
        return Err(PKCS7_ERR);
    }
    if data.len() % block != 0 {
        return Err(PKCS7_ERR);
    }
    let n = *data.last().unwrap() as usize;
    if n == 0 || n > block || n > data.len() {
        return Err(PKCS7_ERR);
    }
    if data[data.len() - n..] != vec![n as u8; n] {
        return Err(PKCS7_ERR);
    }
    Ok(&data[..data.len() - n])
}
