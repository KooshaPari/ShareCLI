// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Seed target for C07 L67 — toml_lite is a pure parser with no I/O.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = sharecli::util::toml_lite::parse(s);
    }
});
