// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the DNS wire-format parser (RFC 1035).
//
// High-value target because it handles:
//   * compression pointers (potential infinite loops, OOB reads)
//   * variable-length label encoding
//   * multi-section messages (questions, answers, authority, additional)
//
// The parser enforces MAX_POINTER_DEPTH=64 and MAX_LABEL_LENGTH=63,
// but malformed inputs can still stress bounds-checking logic.
fuzz_target!(|data: &[u8]| {
    let _ = sharecli::dns_query_parser::parse(data);
});
