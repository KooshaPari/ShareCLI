// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the SSH binary packet parser (RFC 4253 §6).
//
// High-value target because it parses:
//   * A 4-byte length field that must be validated against actual input
//   * A padding_length byte followed by variable-length payload
//   * Padding that must be between 4 and 255 bytes
//
// The parser enforces MIN_PACKET_LENGTH and MAX_PACKET_LENGTH, but
// edge cases around the boundary between header and payload are a
// common source of off-by-one errors.
fuzz_target!(|data: &[u8]| {
    let _ = sharecli::ssh_packet::parse(data);
});
