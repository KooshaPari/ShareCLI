// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the SNMPv3 message envelope parser (RFC 3412 §6).
//
// High-value target because it parses a binary BER-TLV framing
// with:
//   * INTEGER fields with variable-length encoding
//   * OCTET STRING fields with short-form and long-form lengths
//   * A SEQUENCE (scopedPDU) whose body length is derived from input
//
// Malformed length fields or truncated buffers are the primary
// attack surface for buffer over-reads and integer-overflow bugs.
fuzz_target!(|data: &[u8]| {
    let _ = sharecli::snmpv3_msg::parse(data);
});
