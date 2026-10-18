// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::time::{SystemTime, UNIX_EPOCH};

pub fn v7_like() -> String {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let rand: u64 = (SystemTime::now().elapsed().map(|d| d.subsec_nanos()).unwrap_or(0) as u64)
        ^ 0xdeadbeef_cafebabe;
    format!("{:x}-{:x}", ts, rand)
}

pub fn is_valid(s: &str) -> bool {
    if s.len() < 3 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    !parts.is_empty()
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_hexdigit()))
}

pub fn short_id() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    format!("{:08x}", nanos)
}

pub fn slug() -> String {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:x}-{}", ts, short_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_v7_like_unique() {
        std::thread::sleep(std::time::Duration::from_millis(2));
        let a = v7_like();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = v7_like();
        assert_ne!(a, b);
    }
    #[test]
    fn test_v7_like_format() {
        let s = v7_like();
        assert!(s.contains('-'));
        assert!(s.len() >= 8);
    }
    #[test]
    fn test_validate_valid() {
        assert!(is_valid("abc-123-def"));
        assert!(is_valid("12345678"));
    }
    #[test]
    fn test_validate_invalid() {
        assert!(!is_valid(""));
        assert!(!is_valid("xy"));
        assert!(!is_valid("xx-yy-zz!"));
    }
    #[test]
    fn test_short_id_format() {
        assert_eq!(short_id().len(), 8);
    }
    #[test]
    fn test_slug_format() {
        let s = slug();
        assert!(s.contains('-'));
    }
}
