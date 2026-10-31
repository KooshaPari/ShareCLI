// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the LDAP search filter parser (RFC 4515).
//
// High-value target because it parses:
//   * Recursive Boolean expressions with nested parentheses
//   * Substring filters with wildcard (`*`) patterns
//   * Unbalanced or deeply nested parenthetical groups
//
// The parser is recursive, so deeply-nested inputs (e.g. thousands
// of `(!(!...` layers) could cause stack exhaustion if depth limits
// are not enforced.  Backtracking in substring matching is another
// quadratic-time attack vector.
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = sharecli::ldap_filter::parse(s);
    }
});
