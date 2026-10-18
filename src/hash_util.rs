// SPDX-License-Identifier: Apache-2.0 OR MIT
pub fn djb2(s: &str) -> u32 {
    let mut hash: u32 = 5381;
    for c in s.chars() {
        hash = hash.wrapping_mul(33).wrapping_add(c as u32);
    }
    hash
}

pub fn fnv1a(s: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for c in s.chars() {
        hash ^= c as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

pub fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h = h.wrapping_mul(0x100000001b3) ^ b as u64;
    }
    h
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn djb2_empty() {
        assert_eq!(djb2(""), 5381);
    }
    #[test]
    fn djb2_deterministic() {
        assert_eq!(djb2("hello"), djb2("hello"));
    }
    #[test]
    fn djb2_different() {
        assert_ne!(djb2("hello"), djb2("world"));
    }
    #[test]
    fn fnv1a_empty() {
        assert_eq!(fnv1a(""), 0x811c9dc5);
    }
    #[test]
    fn fnv1a_consistent() {
        assert_eq!(fnv1a("test"), fnv1a("test"));
    }
    #[test]
    fn simple_hash_empty() {
        assert_eq!(simple_hash(""), 0xcbf29ce484222325);
    }
    #[test]
    fn simple_hash_unique() {
        assert_ne!(simple_hash("alpha"), simple_hash("beta"));
    }
}
