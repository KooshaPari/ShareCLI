// Minimal X.509 DER certificate parser (RFC 5280).
//
// Parses the outer Certificate structure (TBSCertificate + signatureAlgorithm +
// signatureValue) and extracts a few common fields: subject, issuer, validity
// dates, and a public key algorithm OID. Also counts extensions encountered.
//
// This is NOT a validator — it does not verify signatures, check revocation, or
// validate chains. It exists so consumers can introspect a DER blob without
// pulling in a full x509-parser crate.

use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DistinguishedName {
    pub raw: String,
    pub parts: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Validity {
    pub not_before: String,
    pub not_after: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Cert {
    pub version: u8,
    pub serial: Vec<u8>,
    pub signature_algorithm: String,
    pub issuer: DistinguishedName,
    pub subject: DistinguishedName,
    pub validity: Validity,
    pub public_key_algorithm: String,
    pub extensions_count: usize,
    pub raw_len: usize,
}

/// Parse a DER-encoded X.509 certificate. Returns the parsed structure or
/// a human-readable error string. The string error form is intentional: the
/// sharecli convention across parsers is `Result<T, String>`.
pub fn parse_der(input: &[u8]) -> Result<Cert, String> {
    let mut p = Parser::new(input);
    let outer = p.read_tlv().map_err(|e| e.to_string())?;
    if outer.tag != 0x30 {
        return Err(format!(
            "not a certificate: outer tag 0x{:02x} (expected SEQUENCE 0x30)",
            outer.tag
        ));
    }
    let mut inner = Parser::new(outer.content);

    // TBSCertificate (SEQUENCE)
    let tbs = inner.read_tlv().map_err(|e| e.to_string())?;
    if tbs.tag != 0x30 {
        return Err(format!("tbsCertificate tag 0x{:02x} (expected 0x30)", tbs.tag));
    }
    let mut tbs_p = Parser::new(tbs.content);

    // Optional [0] EXPLICIT version. Absent tag → v1 (the default).
    let mut version: u8 = 1;
    if tbs_p.peek() == Some(0xa0) {
        let v = tbs_p.read_tlv().map_err(|e| e.to_string())?;
        if v.tag != 0xa0 {
            return Err(format!("version tag 0x{:02x} (expected 0xa0)", v.tag));
        }
        let mut vp = Parser::new(v.content);
        let i = vp.read_tlv().map_err(|e| e.to_string())?;
        if i.tag != 0x02 {
            return Err(format!("version INTEGER tag 0x{:02x}", i.tag));
        }
        version = if i.content.is_empty() {
            1
        } else {
            // DER INTEGER: value 0 means v1, 1 means v2, 2 means v3.
            i.content[0] + 1
        };
    }

    // serialNumber INTEGER
    let serial_tlv = tbs_p.read_tlv().map_err(|e| e.to_string())?;
    if serial_tlv.tag != 0x02 {
        return Err(format!("serialNumber tag 0x{:02x}", serial_tlv.tag));
    }
    let serial = strip_leading_zeros(serial_tlv.content);

    // signature AlgorithmIdentifier
    let sig_alg = tbs_p.read_tlv().map_err(|e| e.to_string())?;
    if sig_alg.tag != 0x30 {
        return Err(format!("signature AlgorithmIdentifier tag 0x{:02x}", sig_alg.tag));
    }
    let sig_alg_oid = read_first_oid(sig_alg.content).map_err(|e| e.to_string())?;

    // issuer Name
    let issuer = read_name(&mut tbs_p).map_err(|e| e.to_string())?;

    // validity Validity
    let validity = read_validity(&mut tbs_p).map_err(|e| e.to_string())?;

    // subject Name
    let subject = read_name(&mut tbs_p).map_err(|e| e.to_string())?;

    // subjectPublicKeyInfo SubjectPublicKeyInfo — we just want the algorithm OID.
    let spki = tbs_p.read_tlv().map_err(|e| e.to_string())?;
    if spki.tag != 0x30 {
        return Err(format!("subjectPublicKeyInfo tag 0x{:02x}", spki.tag));
    }
    let pk_alg_oid = read_spki_algorithm(spki.content).map_err(|e| e.to_string())?;

    // Optional [3] EXPLICIT extensions
    let mut extensions_count: usize = 0;
    if tbs_p.peek() == Some(0xa3) {
        let ext_wrap = tbs_p.read_tlv().map_err(|e| e.to_string())?;
        if ext_wrap.tag != 0xa3 {
            return Err(format!("extensions wrap tag 0x{:02x}", ext_wrap.tag));
        }
        let mut ep = Parser::new(ext_wrap.content);
        // Extensions ::= SEQUENCE OF Extension
        let ext_seq = ep.read_tlv().map_err(|e| e.to_string())?;
        if ext_seq.tag != 0x30 {
            return Err(format!("extensions SEQUENCE tag 0x{:02x}", ext_seq.tag));
        }
        let mut xp = Parser::new(ext_seq.content);
        while !xp.eof() {
            let ext_one = xp.read_tlv().map_err(|e| e.to_string())?;
            if ext_one.tag != 0x30 {
                return Err(format!("extension tag 0x{:02x}", ext_one.tag));
            }
            extensions_count += 1;
            // Skip the rest — we just need the count.
            let _ = ext_one.content;
        }
    }

    // Optional issuerUniqueID [1] BIT STRING + subjectUniqueID [2] BIT STRING
    // and signatureValue BIT STRING — we don't need them, so we ignore any
    // trailing data inside the outer SEQUENCE. The parser already stopped at
    // the end of TBS, so this is fine.

    Ok(Cert {
        version,
        serial,
        signature_algorithm: sig_alg_oid,
        issuer,
        subject,
        validity,
        public_key_algorithm: pk_alg_oid,
        extensions_count,
        raw_len: input.len(),
    })
}

/// DNSSEC-style key tag (RFC 4034 §5.2). 32-bit accumulator folded into 16
/// bits. Useful for ordering trust anchors and detecting duplicate keys in
/// trust chain material.
pub fn compute_key_tag(rdata: &[u8]) -> u16 {
    let mut acc: u32 = 0;
    for (i, &b) in rdata.iter().enumerate() {
        if (i & 1) == 0 {
            acc = acc.wrapping_add((b as u32) << 8);
        } else {
            acc = acc.wrapping_add(b as u32);
        }
    }
    let folded = (acc & 0xffff) + (acc >> 16);
    (folded & 0xffff) as u16
}

fn strip_leading_zeros(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut start = 0;
    while start + 1 < bytes.len() && bytes[start] == 0x00 {
        start += 1;
    }
    bytes[start..].to_vec()
}

struct Tlv<'a> {
    tag: u8,
    content: &'a [u8],
}

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a [u8]) -> Self {
        Self { src: s, pos: 0 }
    }
    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }
    fn peek(&self) -> Option<u8> {
        if self.pos < self.src.len() {
            Some(self.src[self.pos])
        } else {
            None
        }
    }
    fn read_len(&mut self) -> Result<usize, ParseError> {
        let first = self.peek().ok_or(ParseError::Truncated)?;
        self.pos += 1;
        if first < 0x80 {
            Ok(first as usize)
        } else {
            let n = (first & 0x7f) as usize;
            if n == 0 {
                return Err(ParseError::BadLength);
            }
            if self.pos + n > self.src.len() {
                return Err(ParseError::Truncated);
            }
            let mut out: usize = 0;
            for _ in 0..n {
                out = (out << 8) | self.src[self.pos] as usize;
                self.pos += 1;
            }
            // DER requires the smallest possible length encoding.
            if n > 1 && (out >> ((n - 1) * 8)) == 0 {
                return Err(ParseError::BadLength);
            }
            Ok(out)
        }
    }
    fn read_tlv(&mut self) -> Result<Tlv<'a>, ParseError> {
        let tag = self.peek().ok_or(ParseError::Truncated)?;
        self.pos += 1;
        let len = self.read_len()?;
        if self.pos + len > self.src.len() {
            return Err(ParseError::Truncated);
        }
        let start = self.pos;
        self.pos += len;
        Ok(Tlv { tag, content: &self.src[start..start + len] })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    Truncated,
    BadTag(u8),
    BadLength,
    InvalidUtf8,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Truncated => write!(f, "input truncated"),
            ParseError::BadTag(t) => write!(f, "unexpected tag 0x{:02x}", t),
            ParseError::BadLength => write!(f, "bad length encoding"),
            ParseError::InvalidUtf8 => write!(f, "invalid utf-8"),
        }
    }
}

fn read_first_oid(seq_bytes: &[u8]) -> Result<String, ParseError> {
    let mut p = Parser::new(seq_bytes);
    let t = p.read_tlv()?;
    if t.tag != 0x06 {
        return Err(ParseError::BadTag(t.tag));
    }
    Ok(format_oid(t.content))
}

fn read_spki_algorithm(spki_bytes: &[u8]) -> Result<String, ParseError> {
    // SubjectPublicKeyInfo ::= SEQUENCE { algorithm AlgorithmIdentifier, subjectPublicKey BIT STRING }
    let mut p = Parser::new(spki_bytes);
    let alg = p.read_tlv()?;
    if alg.tag != 0x30 {
        return Err(ParseError::BadTag(alg.tag));
    }
    read_first_oid(alg.content)
}

fn format_oid(oid: &[u8]) -> String {
    if oid.is_empty() {
        return String::new();
    }
    let first = oid[0];
    let a = first / 40;
    let b = first % 40;
    let mut parts: Vec<String> = vec![a.to_string(), b.to_string()];
    let mut acc: u32 = 0;
    for &byte in &oid[1..] {
        acc = (acc << 7) | (byte & 0x7f) as u32;
        if byte & 0x80 == 0 {
            parts.push(acc.to_string());
            acc = 0;
        }
    }
    parts.join(".")
}

fn oid_to_name(oid: &str) -> &'static str {
    match oid {
        "2.5.4.3" => "CN",
        "2.5.4.6" => "C",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "2.5.4.5" => "serialNumber",
        "2.5.4.9" => "street",
        "2.5.4.17" => "postalCode",
        "1.2.840.113549.1.9.1" => "E",
        _ => "OID",
    }
}

fn read_name(p: &mut Parser) -> Result<DistinguishedName, ParseError> {
    let t = p.read_tlv()?;
    if t.tag != 0x30 {
        return Err(ParseError::BadTag(t.tag));
    }
    let mut inner = Parser::new(t.content);
    let mut parts: BTreeMap<String, String> = BTreeMap::new();
    let mut raw = String::new();
    while !inner.eof() {
        let set = inner.read_tlv()?;
        if set.tag != 0x31 {
            return Err(ParseError::BadTag(set.tag));
        }
        let mut sp = Parser::new(set.content);
        let seq = sp.read_tlv()?;
        if seq.tag != 0x30 {
            return Err(ParseError::BadTag(seq.tag));
        }
        let mut mp = Parser::new(seq.content);
        let oid_bytes = mp.read_tlv()?;
        if oid_bytes.tag != 0x06 {
            return Err(ParseError::BadTag(oid_bytes.tag));
        }
        let oid = format_oid(oid_bytes.content);
        let value_tlv = mp.read_tlv()?;
        // DirectoryString variants: UTF8String 0x0c, PrintableString 0x13,
        // IA5String 0x16, BMPString 0x1e, UniversalString 0x1c. We accept any
        // human-readable tag and fall back to hex for unknown ones.
        let value_str = match value_tlv.tag {
            0x0c | 0x13 | 0x16 | 0x1e | 0x1c | 0x14 => std::str::from_utf8(value_tlv.content)
                .map_err(|_| ParseError::InvalidUtf8)?
                .to_string(),
            _ => format!("0x{}", hex_lower(value_tlv.content)),
        };
        let key = oid_to_name(&oid);
        if !raw.is_empty() {
            raw.push(',');
        }
        raw.push_str(&format!("{}={}", key, value_str));
        parts.insert(key.to_string(), value_str);
    }
    Ok(DistinguishedName { raw, parts })
}

fn read_validity(p: &mut Parser) -> Result<Validity, ParseError> {
    let t = p.read_tlv()?;
    if t.tag != 0x30 {
        return Err(ParseError::BadTag(t.tag));
    }
    let mut inner = Parser::new(t.content);
    let nb = inner.read_tlv()?;
    let na = inner.read_tlv()?;
    let not_before = read_time(nb.tag, nb.content)?;
    let not_after = read_time(na.tag, na.content)?;
    Ok(Validity { not_before, not_after })
}

fn read_time(tag: u8, content: &[u8]) -> Result<String, ParseError> {
    let s = std::str::from_utf8(content).map_err(|_| ParseError::InvalidUtf8)?;
    match tag {
        0x17 | 0x18 => Ok(s.to_string()),
        _ => Err(ParseError::BadTag(tag)),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-craft a minimal v1 self-signed certificate.
    /// Cert ::= SEQUENCE { tbsCertificate, signatureAlgorithm, signatureValue }
    fn minimal_v1_cert() -> Vec<u8> {
        // issuer == subject == {CN=Test} minimal Name
        let cn_value = b"Test";
        let cn_oid = [0x55, 0x04, 0x03]; // 2.5.4.3
        let attr_type_and_value = |oid: &[u8], value: &[u8]| -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x30);
            encode_len(&mut v, oid.len() + 2 + value.len() + 2);
            v.push(0x06);
            encode_len(&mut v, oid.len());
            v.extend_from_slice(oid);
            v.push(0x0c); // UTF8String
            encode_len(&mut v, value.len());
            v.extend_from_slice(value);
            v
        };
        let rdn_set = |atv: Vec<u8>| -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x31);
            encode_len(&mut v, atv.len());
            v.extend(atv);
            v
        };
        let atv = attr_type_and_value(&cn_oid, cn_value);
        let rdn = rdn_set(atv);
        let mut name_inner: Vec<u8> = Vec::new();
        name_inner.extend(&rdn);
        let mut name = Vec::new();
        name.push(0x30);
        encode_len(&mut name, name_inner.len());
        name.extend(name_inner);

        // notBefore / notAfter — UTCTime format YYMMDDHHMMSSZ
        let nb = b"200101000000Z";
        let na = b"300101000000Z";

        // serialNumber
        let serial = [0x01u8];

        // signature AlgorithmIdentifier — rsaEncryption OID (1.2.840.113549.1.1.1)
        let sig_alg_oid = [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
        let mut sig_alg = Vec::new();
        sig_alg.push(0x30);
        let sig_alg_inner = {
            let mut v = Vec::new();
            v.push(0x06);
            encode_len(&mut v, sig_alg_oid.len());
            v.extend_from_slice(&sig_alg_oid);
            v.push(0x05);
            v.push(0x00); // NULL params
            v
        };
        encode_len(&mut sig_alg, sig_alg_inner.len());
        sig_alg.extend(sig_alg_inner);

        // subjectPublicKeyInfo — minimal RSA SPKI with algorithm OID
        let pk_alg_oid = sig_alg_oid; // reuse rsaEncryption
        let mut spki = Vec::new();
        spki.push(0x30);
        let spki_inner = {
            let mut v = Vec::new();
            // algorithm
            v.push(0x30);
            let alg_inner = {
                let mut a = Vec::new();
                a.push(0x06);
                encode_len(&mut a, pk_alg_oid.len());
                a.extend_from_slice(&pk_alg_oid);
                a.push(0x05);
                a.push(0x00);
                a
            };
            encode_len(&mut v, alg_inner.len());
            v.extend(alg_inner);
            // subjectPublicKey BIT STRING (empty)
            v.push(0x03);
            v.push(0x02);
            v.push(0x00);
            v.push(0x00);
            v
        };
        encode_len(&mut spki, spki_inner.len());
        spki.extend(spki_inner);

        // tbsCertificate
        let mut tbs = Vec::new();
        // version: omit for v1
        tbs.push(0x02);
        tbs.push(0x01);
        tbs.push(serial[0]);
        tbs.extend(&sig_alg);
        tbs.extend(&name); // issuer
                           // validity
        tbs.push(0x30);
        let validity_inner = {
            let mut v = Vec::new();
            v.push(0x17);
            v.push(nb.len() as u8);
            v.extend_from_slice(nb);
            v.push(0x17);
            v.push(na.len() as u8);
            v.extend_from_slice(na);
            v
        };
        encode_len(&mut tbs, validity_inner.len());
        tbs.extend(validity_inner);
        tbs.extend(&name); // subject
        tbs.extend(&spki);
        // No extensions in v1

        // signatureValue BIT STRING (empty, just for parsing)
        let sig_value = vec![0x03, 0x02, 0x00, 0x00];

        // Outer Certificate
        let mut cert = Vec::new();
        cert.push(0x30);
        let inner = {
            let mut v = Vec::new();
            // TBS wrapped in SEQUENCE
            v.push(0x30);
            encode_len(&mut v, tbs.len());
            v.extend(tbs);
            v.extend(&sig_alg);
            v.extend(sig_value);
            v
        };
        encode_len(&mut cert, inner.len());
        cert.extend(inner);
        cert
    }

    fn encode_len(out: &mut Vec<u8>, len: usize) {
        if len < 0x80 {
            out.push(len as u8);
        } else {
            let mut bytes = Vec::new();
            let mut n = len;
            while n > 0 {
                bytes.push(n as u8);
                n >>= 8;
            }
            bytes.reverse();
            out.push(0x80 | bytes.len() as u8);
            out.extend(bytes);
        }
    }

    #[test]
    fn parse_minimal_v1_cert() {
        let cert = minimal_v1_cert();
        let parsed = parse_der(&cert).expect("parse should succeed");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.serial, vec![0x01]);
        assert_eq!(parsed.subject.parts.get("CN").map(|s| s.as_str()), Some("Test"));
        assert_eq!(parsed.issuer.parts.get("CN").map(|s| s.as_str()), Some("Test"));
        assert_eq!(parsed.validity.not_before, "200101000000Z");
        assert_eq!(parsed.validity.not_after, "300101000000Z");
        assert_eq!(parsed.extensions_count, 0);
        assert_eq!(parsed.raw_len, cert.len());
    }

    #[test]
    fn version_v3_with_extensions() {
        // Build a v3 cert with two no-op extensions.
        // (minimal_v1_cert is a reference shape; this test hand-encodes v3 below.)
        let _ = minimal_v1_cert();
        // We need to inject the [0] version tag + [3] extensions into the TBS.
        // Simplest path: parse v1, then re-encode with version + extensions.
        // For test simplicity, hand-encode a v3 variant.

        // minimal CN name (reused)
        let cn_oid = [0x55u8, 0x04, 0x03];
        let build_name = || -> Vec<u8> {
            let mut atv_inner: Vec<u8> = Vec::new();
            atv_inner.push(0x06);
            atv_inner.push(0x03);
            atv_inner.extend_from_slice(&cn_oid);
            atv_inner.push(0x0c);
            atv_inner.push(0x02);
            atv_inner.push(b'C');
            atv_inner.push(b'N');
            let mut atv: Vec<u8> = Vec::new();
            atv.push(0x30);
            encode_len(&mut atv, atv_inner.len());
            atv.extend(atv_inner);
            let mut set: Vec<u8> = Vec::new();
            set.push(0x31);
            encode_len(&mut set, atv.len());
            set.extend(atv);
            let mut name: Vec<u8> = Vec::new();
            name.push(0x30);
            encode_len(&mut name, set.len());
            name.extend(set);
            name
        };
        let issuer = build_name();
        let subject = build_name();

        let sig_alg_oid = [0x2au8, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b];
        let build_sig_alg = || -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x30);
            v.push(0x0d);
            v.push(0x06);
            v.push(0x09);
            v.extend_from_slice(&sig_alg_oid);
            v.push(0x05);
            v.push(0x00);
            v
        };
        let sig_alg = build_sig_alg();

        let build_spki = || -> Vec<u8> {
            let mut inner: Vec<u8> = Vec::new();
            inner.push(0x30);
            inner.push(0x0d);
            inner.push(0x06);
            inner.push(0x09);
            inner.extend_from_slice(&sig_alg_oid);
            inner.push(0x05);
            inner.push(0x00);
            inner.push(0x03);
            inner.push(0x02);
            inner.push(0x00);
            inner.push(0x00);
            let mut v: Vec<u8> = Vec::new();
            v.push(0x30);
            encode_len(&mut v, inner.len());
            v.extend(inner);
            v
        };
        let spki = build_spki();

        let build_validity = || -> Vec<u8> {
            let mut inner: Vec<u8> = Vec::new();
            inner.push(0x17);
            inner.push(0x0d);
            inner.extend_from_slice(b"200101000000Z");
            inner.push(0x17);
            inner.push(0x0d);
            inner.extend_from_slice(b"300101000000Z");
            let mut v: Vec<u8> = Vec::new();
            v.push(0x30);
            encode_len(&mut v, inner.len());
            v.extend(inner);
            v
        };
        let validity = build_validity();

        // Two simple extensions: OID + OCTET STRING(0x00)
        let build_ext = |oid: &[u8]| -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x30);
            let inner = {
                let mut w = Vec::new();
                w.push(0x06);
                encode_len(&mut w, oid.len());
                w.extend_from_slice(oid);
                w.push(0x04);
                w.push(0x01);
                w.push(0x00);
                w
            };
            encode_len(&mut v, inner.len());
            v.extend(inner);
            v
        };
        // 2.5.29.14 (SubjectKeyIdentifier) + 2.5.29.19 (BasicConstraints)
        let ext1 = build_ext(&[0x55, 0x1d, 0x0e]);
        let ext2 = build_ext(&[0x55, 0x1d, 0x13]);
        let mut extensions_seq: Vec<u8> = Vec::new();
        extensions_seq.push(0x30);
        let mut exts_inner: Vec<u8> = Vec::new();
        exts_inner.extend(&ext1);
        exts_inner.extend(&ext2);
        encode_len(&mut extensions_seq, exts_inner.len());
        extensions_seq.extend(exts_inner);

        // Wrap in [3] EXPLICIT
        let mut extensions_wrap = Vec::new();
        extensions_wrap.push(0xa3);
        encode_len(&mut extensions_wrap, extensions_seq.len());
        extensions_wrap.extend(extensions_seq);

        // v3 version tag = [0] EXPLICIT INTEGER 0x02
        let mut version_tag = Vec::new();
        version_tag.push(0xa0);
        version_tag.push(0x03);
        version_tag.push(0x02);
        version_tag.push(0x01);
        version_tag.push(0x02);

        // tbsCertificate
        let mut tbs = Vec::new();
        tbs.extend(&version_tag);
        tbs.push(0x02);
        tbs.push(0x01);
        tbs.push(0x01);
        tbs.extend(&sig_alg);
        tbs.extend(&issuer);
        tbs.extend(&validity);
        tbs.extend(&subject);
        tbs.extend(&spki);
        tbs.extend(&extensions_wrap);

        // Outer certificate
        let mut cert = Vec::new();
        cert.push(0x30);
        let inner = {
            let mut v = Vec::new();
            v.push(0x30);
            encode_len(&mut v, tbs.len());
            v.extend(tbs);
            v.extend(&sig_alg);
            v.push(0x03);
            v.push(0x02);
            v.push(0x00);
            v.push(0x00);
            v
        };
        encode_len(&mut cert, inner.len());
        cert.extend(inner);
        let parsed = parse_der(&cert).expect("parse v3 should succeed");
        assert_eq!(parsed.version, 3);
        assert_eq!(parsed.extensions_count, 2);
    }

    #[test]
    fn issuer_subject_distinguished_name() {
        // Two-attribute issuer: CN=Issuer, O=Org
        let cn_oid = [0x55u8, 0x04, 0x03];
        let o_oid = [0x55u8, 0x04, 0x0a];

        let build_name = |pairs: &[(&[u8], &str)]| -> Vec<u8> {
            let mut rdns = Vec::new();
            for (oid, value) in pairs {
                let mut atv = Vec::new();
                atv.push(0x30);
                let mut atv_inner = Vec::new();
                atv_inner.push(0x06);
                encode_len(&mut atv_inner, oid.len());
                atv_inner.extend_from_slice(oid);
                atv_inner.push(0x0c);
                encode_len(&mut atv_inner, value.len());
                atv_inner.extend_from_slice(value.as_bytes());
                encode_len(&mut atv, atv_inner.len());
                atv.extend(atv_inner);
                let mut set = Vec::new();
                set.push(0x31);
                encode_len(&mut set, atv.len());
                set.extend(atv);
                rdns.extend(set);
            }
            let mut name = Vec::new();
            name.push(0x30);
            encode_len(&mut name, rdns.len());
            name.extend(rdns);
            name
        };

        let issuer_name = build_name(&[(&cn_oid, "My CA"), (&o_oid, "Org")]);
        let subject_name = build_name(&[(&cn_oid, "leaf.example.com")]);

        let sig_alg_oid = [0x2au8, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b];
        let build_sig_alg = || -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x30);
            v.push(0x0d);
            v.push(0x06);
            v.push(0x09);
            v.extend_from_slice(&sig_alg_oid);
            v.push(0x05);
            v.push(0x00);
            v
        };
        let sig_alg = build_sig_alg();

        let build_spki = || -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x30);
            v.push(0x13);
            v.push(0x30);
            v.push(0x0d);
            v.push(0x06);
            v.push(0x09);
            v.extend_from_slice(&sig_alg_oid);
            v.push(0x05);
            v.push(0x00);
            v.push(0x03);
            v.push(0x02);
            v.push(0x00);
            v.push(0x00);
            v
        };
        let spki = build_spki();

        let build_validity = || -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x30);
            v.push(0x1e);
            v.push(0x17);
            v.push(0x0d);
            v.extend_from_slice(b"240101000000Z");
            v.push(0x17);
            v.push(0x0d);
            v.extend_from_slice(b"340101000000Z");
            v
        };
        let validity = build_validity();

        let mut tbs = Vec::new();
        tbs.push(0x02);
        tbs.push(0x01);
        tbs.push(0x42);
        tbs.extend(&sig_alg);
        tbs.extend(&issuer_name);
        tbs.extend(&validity);
        tbs.extend(&subject_name);
        tbs.extend(&spki);

        let mut cert = Vec::new();
        cert.push(0x30);
        let inner = {
            let mut v = Vec::new();
            v.push(0x30);
            encode_len(&mut v, tbs.len());
            v.extend(tbs);
            v.extend(&sig_alg);
            v.push(0x03);
            v.push(0x02);
            v.push(0x00);
            v.push(0x00);
            v
        };
        encode_len(&mut cert, inner.len());
        cert.extend(inner);
        let parsed = parse_der(&cert).expect("parse should succeed");
        assert_eq!(parsed.issuer.parts.get("CN").map(|s| s.as_str()), Some("My CA"));
        assert_eq!(parsed.issuer.parts.get("O").map(|s| s.as_str()), Some("Org"));
        assert_eq!(parsed.subject.parts.get("CN").map(|s| s.as_str()), Some("leaf.example.com"));
        assert_eq!(parsed.serial, vec![0x42]);
        assert!(parsed.issuer.raw.contains("CN=My CA"));
        assert!(parsed.issuer.raw.contains("O=Org"));
    }

    #[test]
    fn validity_generalized_time() {
        // Build cert with GeneralizedTime (tag 0x18) instead of UTCTime.
        let cn_oid = [0x55u8, 0x04, 0x03];
        let build_name = |cn: &str| -> Vec<u8> {
            // Name: SET { SEQ { OID, UTF8String } }
            // SET: 0x31 + len + content
            // content = SEQ { OID(0x06 03 55 04 03), UTF8String(0x0c len cn) }
            // For cn="S" (1 byte), value TLV = 3 bytes, ATV = 2+9 = 11 bytes,
            // SET = 2+11 = 13 bytes, Name = 2+13 = 15 bytes.
            let mut atv_inner: Vec<u8> = Vec::new();
            atv_inner.push(0x06);
            atv_inner.push(0x03);
            atv_inner.extend_from_slice(&cn_oid);
            atv_inner.push(0x0c);
            atv_inner.push(cn.len() as u8);
            atv_inner.extend_from_slice(cn.as_bytes());
            let mut atv: Vec<u8> = Vec::new();
            atv.push(0x30);
            encode_len(&mut atv, atv_inner.len());
            atv.extend(atv_inner);
            let mut set: Vec<u8> = Vec::new();
            set.push(0x31);
            encode_len(&mut set, atv.len());
            set.extend(atv);
            let mut name: Vec<u8> = Vec::new();
            name.push(0x30);
            encode_len(&mut name, set.len());
            name.extend(set);
            name
        };
        let issuer = build_name("I");
        let subject = build_name("S");
        let sig_alg_oid = [0x2au8, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b];
        let build_sig_alg = || -> Vec<u8> {
            let mut v = Vec::new();
            v.push(0x30);
            v.push(0x0d);
            v.push(0x06);
            v.push(0x09);
            v.extend_from_slice(&sig_alg_oid);
            v.push(0x05);
            v.push(0x00);
            v
        };
        let sig_alg = build_sig_alg();
        let build_spki = || -> Vec<u8> {
            let mut v: Vec<u8> = Vec::new();
            v.push(0x30);
            // algorithm + BIT STRING(2 bytes unused) = 0x0d + 6 + 2 = 19? No —
            // algorithm is 0x30 0x0d [13 bytes] and BIT STRING is 0x03 0x02 0x00 0x00
            // so inner = 15 + 4 = 19 bytes. Total = 0x30 0x13 + 19 = 21.
            v.push(0x13);
            v.push(0x30);
            v.push(0x0d);
            v.push(0x06);
            v.push(0x09);
            v.extend_from_slice(&sig_alg_oid);
            v.push(0x05);
            v.push(0x00);
            v.push(0x03);
            v.push(0x02);
            v.push(0x00);
            v.push(0x00);
            v
        };
        let spki = build_spki();

        // GeneralizedTime "20240101000000Z"
        let nb = b"20240101000000Z";
        let na = b"20340101000000Z";
        let mut validity = Vec::new();
        validity.push(0x30);
        let vi = {
            let mut v = Vec::new();
            v.push(0x18);
            v.push(nb.len() as u8);
            v.extend_from_slice(nb);
            v.push(0x18);
            v.push(na.len() as u8);
            v.extend_from_slice(na);
            v
        };
        encode_len(&mut validity, vi.len());
        validity.extend(vi);

        let mut tbs = Vec::new();
        tbs.push(0x02);
        tbs.push(0x01);
        tbs.push(0x07);
        tbs.extend(&sig_alg);
        tbs.extend(&issuer);
        tbs.extend(&validity);
        tbs.extend(&subject);
        tbs.extend(&spki);

        let mut cert = Vec::new();
        cert.push(0x30);
        let inner = {
            let mut v = Vec::new();
            v.push(0x30);
            encode_len(&mut v, tbs.len());
            v.extend(tbs);
            v.extend(&sig_alg);
            v.push(0x03);
            v.push(0x02);
            v.push(0x00);
            v.push(0x00);
            v
        };
        encode_len(&mut cert, inner.len());
        cert.extend(inner);

        let parsed = parse_der(&cert).expect("parse generalized time should succeed");
        assert_eq!(parsed.validity.not_before, "20240101000000Z");
        assert_eq!(parsed.validity.not_after, "20340101000000Z");
    }

    #[test]
    fn reject_truncated_input() {
        // Truncated length: 0x30 0x82 0x00 0x10 declares 16 bytes but only 0 follow.
        let result = parse_der(&[0x30, 0x82, 0x00, 0x10]);
        assert!(result.is_err());
    }

    #[test]
    fn reject_not_a_certificate() {
        // INTEGER at top level — not a SEQUENCE.
        assert!(parse_der(&[0x02, 0x01, 0x01]).is_err());
        // OCTET STRING at top level.
        assert!(parse_der(&[0x04, 0x00]).is_err());
        // Empty input.
        assert!(parse_der(&[]).is_err());
        // Truncated before tag.
        assert!(parse_der(&[0x30]).is_err());
    }

    #[test]
    fn key_tag_determinism_and_known_vector() {
        // RFC 4034 §5.2 test vector: example.com DNSKEY RDATA key tag = 63640.
        // We can't easily reproduce the full RDATA here, so we assert determinism
        // + a hand-computed value.
        let rdata = b"\x01\x02\x03\x04";
        let t1 = compute_key_tag(rdata);
        let t2 = compute_key_tag(rdata);
        assert_eq!(t1, t2);
        // Hand-compute: 0x01 << 8 + 0x02 = 258, 0x03 << 8 + 0x04 = 772, sum=1030.
        // Fold: (1030 & 0xffff) + (1030 >> 16) = 1030 + 0 = 1030.
        assert_eq!(t1, 1030);
    }

    #[test]
    fn public_key_algorithm_oid_rsa() {
        // rsaEncryption = 1.2.840.113549.1.1.1
        let cert = minimal_v1_cert();
        let parsed = parse_der(&cert).expect("parse ok");
        assert_eq!(parsed.public_key_algorithm, "1.2.840.113549.1.1.1");
        assert_eq!(parsed.signature_algorithm, "1.2.840.113549.1.1.1");
    }

    #[test]
    fn public_key_algorithm_oid_ec() {
        // Build a cert with id-ecPublicKey = 1.2.840.10045.2.1
        let cn_oid = [0x55u8, 0x04, 0x03];
        let build_name = |cn: &str| -> Vec<u8> {
            let mut atv_inner: Vec<u8> = Vec::new();
            atv_inner.push(0x06);
            atv_inner.push(0x03);
            atv_inner.extend_from_slice(&cn_oid);
            atv_inner.push(0x0c);
            atv_inner.push(cn.len() as u8);
            atv_inner.extend_from_slice(cn.as_bytes());
            let mut atv: Vec<u8> = Vec::new();
            atv.push(0x30);
            encode_len(&mut atv, atv_inner.len());
            atv.extend(atv_inner);
            let mut set: Vec<u8> = Vec::new();
            set.push(0x31);
            encode_len(&mut set, atv.len());
            set.extend(atv);
            let mut name: Vec<u8> = Vec::new();
            name.push(0x30);
            encode_len(&mut name, set.len());
            name.extend(set);
            name
        };
        let issuer = build_name("I");
        let subject = build_name("S");
        let sig_alg_oid = [0x2au8, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b];
        let ec_oid = [0x2au8, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
        let build_sig_alg = || -> Vec<u8> {
            let mut inner: Vec<u8> = Vec::new();
            inner.push(0x06);
            inner.push(0x09);
            inner.extend_from_slice(&sig_alg_oid);
            inner.push(0x05);
            inner.push(0x00);
            let mut v: Vec<u8> = Vec::new();
            v.push(0x30);
            encode_len(&mut v, inner.len());
            v.extend(inner);
            v
        };
        let sig_alg = build_sig_alg();
        let build_spki = || -> Vec<u8> {
            // algorithm: SEQ { OID(id-ecPublicKey), OID(P-256) }
            let mut alg_inner: Vec<u8> = Vec::new();
            alg_inner.push(0x06);
            alg_inner.push(0x07);
            alg_inner.extend_from_slice(&ec_oid);
            alg_inner.push(0x06);
            alg_inner.push(0x08);
            alg_inner.extend_from_slice(&[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07]);
            let mut alg: Vec<u8> = Vec::new();
            alg.push(0x30);
            encode_len(&mut alg, alg_inner.len());
            alg.extend(alg_inner);
            // BIT STRING (empty)
            let mut bs: Vec<u8> = Vec::new();
            bs.push(0x03);
            bs.push(0x02);
            bs.push(0x00);
            bs.push(0x00);
            // SPKI = SEQ { alg, bs }
            let mut spki_inner: Vec<u8> = Vec::new();
            spki_inner.extend(&alg);
            spki_inner.extend(&bs);
            let mut v: Vec<u8> = Vec::new();
            v.push(0x30);
            encode_len(&mut v, spki_inner.len());
            v.extend(spki_inner);
            v
        };
        let spki = build_spki();
        let build_validity = || -> Vec<u8> {
            let mut inner: Vec<u8> = Vec::new();
            inner.push(0x17);
            inner.push(0x0d);
            inner.extend_from_slice(b"200101000000Z");
            inner.push(0x17);
            inner.push(0x0d);
            inner.extend_from_slice(b"300101000000Z");
            let mut v: Vec<u8> = Vec::new();
            v.push(0x30);
            encode_len(&mut v, inner.len());
            v.extend(inner);
            v
        };
        let validity = build_validity();
        let mut tbs: Vec<u8> = Vec::new();
        tbs.push(0x02);
        tbs.push(0x01);
        tbs.push(0x01);
        tbs.extend(&sig_alg);
        tbs.extend(&issuer);
        tbs.extend(&validity);
        tbs.extend(&subject);
        tbs.extend(&spki);
        let mut cert: Vec<u8> = Vec::new();
        cert.push(0x30);
        let inner = {
            let mut v: Vec<u8> = Vec::new();
            v.push(0x30);
            encode_len(&mut v, tbs.len());
            v.extend(tbs);
            v.extend(&sig_alg);
            v.push(0x03);
            v.push(0x02);
            v.push(0x00);
            v.push(0x00);
            v
        };
        encode_len(&mut cert, inner.len());
        cert.extend(inner);
        let parsed = parse_der(&cert).expect("parse ec cert should succeed");
        assert_eq!(parsed.public_key_algorithm, "1.2.840.10045.2.1");
    }

    #[test]
    fn oid_format_known() {
        assert_eq!(format_oid(&[0x55, 0x04, 0x03]), "2.5.4.3");
        assert_eq!(
            format_oid(&[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b]),
            "1.2.840.113549.1.1.11"
        );
    }

    #[test]
    fn strip_leading_zeros_works() {
        assert_eq!(strip_leading_zeros(&[0x00, 0x01, 0x02]), vec![0x01, 0x02]);
        // keep at least one byte
        assert_eq!(strip_leading_zeros(&[0x00, 0x00, 0x00]), vec![0x00]);
        assert_eq!(strip_leading_zeros(&[]), Vec::<u8>::new());
    }

    // --- oid_to_name coverage ---

    #[test]
    fn oid_to_name_known_oids() {
        assert_eq!(oid_to_name("2.5.4.3"), "CN");
        assert_eq!(oid_to_name("2.5.4.6"), "C");
        assert_eq!(oid_to_name("2.5.4.7"), "L");
        assert_eq!(oid_to_name("2.5.4.8"), "ST");
        assert_eq!(oid_to_name("2.5.4.10"), "O");
        assert_eq!(oid_to_name("2.5.4.11"), "OU");
        assert_eq!(oid_to_name("2.5.4.5"), "serialNumber");
        assert_eq!(oid_to_name("2.5.4.9"), "street");
        assert_eq!(oid_to_name("2.5.4.17"), "postalCode");
        assert_eq!(oid_to_name("1.2.840.113549.1.9.1"), "E");
    }

    #[test]
    fn oid_to_name_unknown_returns_oid_fallback() {
        assert_eq!(oid_to_name("9.9.9"), "OID");
        assert_eq!(oid_to_name(""), "OID");
    }

    // --- hex_lower coverage ---

    #[test]
    fn hex_lower_empty() {
        assert_eq!(hex_lower(b""), "");
    }

    #[test]
    fn hex_lower_single_byte() {
        assert_eq!(hex_lower(&[0x0a]), "0a");
        assert_eq!(hex_lower(&[0xff]), "ff");
        assert_eq!(hex_lower(&[0x00]), "00");
    }

    #[test]
    fn hex_lower_multi_byte() {
        assert_eq!(hex_lower(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
    }

    // --- format_oid edge cases ---

    #[test]
    fn format_oid_empty() {
        assert_eq!(format_oid(&[]), "");
    }

    #[test]
    fn format_oid_single_byte() {
        // First byte: 0 => 0/40=0, 0%40=0 => "0.0"
        assert_eq!(format_oid(&[0x00]), "0.0");
    }

    #[test]
    fn format_oid_second_arc() {
        // byte 40 => 40/40=1, 40%40=0 => "1.0"
        assert_eq!(format_oid(&[40]), "1.0");
        // byte 80 => 80/40=2, 80%40=0 => "2.0"
        assert_eq!(format_oid(&[80]), "2.0");
    }

    // --- compute_key_tag edge cases ---

    #[test]
    fn compute_key_tag_empty() {
        assert_eq!(compute_key_tag(b""), 0);
    }

    #[test]
    fn compute_key_tag_single_byte() {
        // 0x01: acc = 0x01 << 8 = 256, folded = 256
        assert_eq!(compute_key_tag(&[0x01]), 256);
    }

    #[test]
    fn compute_key_tag_wraparound() {
        // Large input that causes accumulator wrap; just verify no panic
        let rdata = [0xffu8; 64];
        let _tag = compute_key_tag(&rdata);
    }

    // --- ParseError Display ---

    #[test]
    fn parse_error_display_truncated() {
        assert_eq!(ParseError::Truncated.to_string(), "input truncated");
    }

    #[test]
    fn parse_error_display_bad_tag() {
        let err = ParseError::BadTag(0x99);
        assert_eq!(err.to_string(), "unexpected tag 0x99");
    }

    #[test]
    fn parse_error_display_bad_length() {
        assert_eq!(ParseError::BadLength.to_string(), "bad length encoding");
    }

    #[test]
    fn parse_error_display_invalid_utf8() {
        assert_eq!(ParseError::InvalidUtf8.to_string(), "invalid utf-8");
    }

    // --- parse_der error paths ---

    #[test]
    fn parse_der_bad_outer_tag() {
        // INTEGER at top level
        assert!(parse_der(&[0x02, 0x01, 0x01]).is_err());
    }

    #[test]
    fn parse_der_truncated_tbs() {
        // SEQUENCE with tbsCertificate too short
        assert!(parse_der(&[0x30, 0x03, 0x30, 0x01]).is_err());
    }

    #[test]
    fn parse_der_bad_serial_tag() {
        // Build a minimal cert where serial is not INTEGER (0x02)
        let cn_oid = [0x55u8, 0x04, 0x03];
        let build_name = || -> Vec<u8> {
            let mut atv_inner: Vec<u8> = Vec::new();
            atv_inner.push(0x06);
            atv_inner.push(0x03);
            atv_inner.extend_from_slice(&cn_oid);
            atv_inner.push(0x0c);
            atv_inner.push(0x01);
            atv_inner.push(b'X');
            let mut atv: Vec<u8> = Vec::new();
            atv.push(0x30);
            encode_len(&mut atv, atv_inner.len());
            atv.extend(atv_inner);
            let mut set: Vec<u8> = Vec::new();
            set.push(0x31);
            encode_len(&mut set, atv.len());
            set.extend(atv);
            let mut name: Vec<u8> = Vec::new();
            name.push(0x30);
            encode_len(&mut name, set.len());
            name.extend(set);
            name
        };
        let name = build_name();
        let mut tbs = Vec::new();
        // serial as OCTET STRING instead of INTEGER
        tbs.push(0x04);
        tbs.push(0x01);
        tbs.push(0x01);
        let mut cert = Vec::new();
        cert.push(0x30);
        let inner = {
            let mut v = Vec::new();
            v.push(0x30);
            encode_len(&mut v, tbs.len());
            v.extend(tbs);
            v
        };
        encode_len(&mut cert, inner.len());
        cert.extend(inner);
        let err = parse_der(&cert).unwrap_err();
        assert!(err.contains("serialNumber"), "error should mention serial: {err}");
    }

    // --- read_spki_algorithm error path ---

    #[test]
    fn read_spki_algorithm_bad_tag() {
        // Not a SEQUENCE
        let result = read_spki_algorithm(&[0x02, 0x01, 0x01]);
        assert!(result.is_err());
    }

    // --- read_first_oid error path ---

    #[test]
    fn read_first_oid_bad_tag() {
        // Not an OID tag (0x06)
        let result = read_first_oid(&[0x04, 0x01, 0x00]);
        assert!(result.is_err());
    }

    // --- read_time error path ---

    #[test]
    fn read_time_bad_tag() {
        // Neither UTCTime (0x17) nor GeneralizedTime (0x18)
        let result = read_time(0x04, b"test");
        assert!(result.is_err());
    }

    #[test]
    fn read_time_valid_ucttime() {
        let result = read_time(0x17, b"240101000000Z");
        assert_eq!(result.unwrap(), "240101000000Z");
    }

    #[test]
    fn read_time_valid_generalizedtime() {
        let result = read_time(0x18, b"20240101000000Z");
        assert_eq!(result.unwrap(), "20240101000000Z");
    }

    // --- read_validity error paths ---

    #[test]
    fn read_validity_bad_tag() {
        // Not a SEQUENCE
        let mut p = Parser::new(&[0x04, 0x00]);
        let result = read_validity(&mut p);
        assert!(result.is_err());
    }

    // --- read_name error paths ---

    #[test]
    fn read_name_bad_tag_not_sequence() {
        let mut p = Parser::new(&[0x04, 0x00]);
        let result = read_name(&mut p);
        assert!(result.is_err());
    }

    // --- DistinguishedName raw format ---

    #[test]
    fn distinguished_name_raw_format() {
        let cert = minimal_v1_cert();
        let parsed = parse_der(&cert).unwrap();
        assert!(parsed.issuer.raw.contains("CN=Test"));
        assert!(parsed.subject.raw.contains("CN=Test"));
    }

    // --- Cert struct fields ---

    #[test]
    fn cert_raw_len_matches_input() {
        let cert = minimal_v1_cert();
        let parsed = parse_der(&cert).unwrap();
        assert_eq!(parsed.raw_len, cert.len());
    }

    #[test]
    fn cert_signature_algorithm_is_oid() {
        let cert = minimal_v1_cert();
        let parsed = parse_der(&cert).unwrap();
        assert!(parsed.signature_algorithm.contains('.'), "sig alg should be dotted OID");
    }

    // --- read_name with PrintableString tag ---

    #[test]
    fn read_name_with_printable_string() {
        // Build a cert with PrintableString (0x13) in name
        let cn_oid = [0x55u8, 0x04, 0x03];
        let mut atv_inner: Vec<u8> = Vec::new();
        atv_inner.push(0x06);
        atv_inner.push(0x03);
        atv_inner.extend_from_slice(&cn_oid);
        atv_inner.push(0x13); // PrintableString
        atv_inner.push(0x03);
        atv_inner.extend_from_slice(b"ABC");
        let mut atv: Vec<u8> = Vec::new();
        atv.push(0x30);
        encode_len(&mut atv, atv_inner.len());
        atv.extend(atv_inner);
        let mut set: Vec<u8> = Vec::new();
        set.push(0x31);
        encode_len(&mut set, atv.len());
        set.extend(atv);
        let mut name: Vec<u8> = Vec::new();
        name.push(0x30);
        encode_len(&mut name, set.len());
        name.extend(set);
        let mut p = Parser::new(&name);
        let dn = read_name(&mut p).unwrap();
        assert_eq!(dn.parts.get("CN").map(|s| s.as_str()), Some("ABC"));
    }

    // --- read_name with IA5String tag ---

    #[test]
    fn read_name_with_ia5string() {
        let cn_oid = [0x55u8, 0x04, 0x03];
        let mut atv_inner: Vec<u8> = Vec::new();
        atv_inner.push(0x06);
        atv_inner.push(0x03);
        atv_inner.extend_from_slice(&cn_oid);
        atv_inner.push(0x16); // IA5String
        atv_inner.push(0x05);
        atv_inner.extend_from_slice(b"hello");
        let mut atv: Vec<u8> = Vec::new();
        atv.push(0x30);
        encode_len(&mut atv, atv_inner.len());
        atv.extend(atv_inner);
        let mut set: Vec<u8> = Vec::new();
        set.push(0x31);
        encode_len(&mut set, atv.len());
        set.extend(atv);
        let mut name: Vec<u8> = Vec::new();
        name.push(0x30);
        encode_len(&mut name, set.len());
        name.extend(set);
        let mut p = Parser::new(&name);
        let dn = read_name(&mut p).unwrap();
        assert_eq!(dn.parts.get("CN").map(|s| s.as_str()), Some("hello"));
    }
}
