// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the CoAP option parser (RFC 7252 §3.1).
//
// High-value target because it handles:
//   * Delta and length nibbles with 3-tier extension encoding
//   * Extended 1-byte (13..268) and 2-byte (269..65803) values
//   * Incremental option number tracking across a chain
//   * Payload marker (0xFF) detection
//
// Off-by-one in nibble extraction or truncated extension bytes
// are the primary attack surface.
fuzz_target!(|data: &[u8]| {
    let _ = sharecli::coap_option_parse::parse_options(data);
});
