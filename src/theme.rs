// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Backbone-2 theme tokens for sharecli (Phase 2 tokens.css adoption).
//!
//! Source of truth: tokens.css (`bb2-graphite`, `bb2-panel`, `bb2-pulse-green`,
//! `bb2-sync-violet`, `bb2-warm-amber`, `bb2-error`). This module is the Rust
//! mirror of those CSS custom properties so any non-CLI surface (logs, tui,
//! markdown reports) can paint with the same family without re-deriving hex
//! anywhere.
//!
//! All `from_hex` is `const fn`, so the palette is statically available at
//! compile time and `Tokens::default()` is a `const` value.

#![allow(dead_code)]

/// A packed RGBA color (no alpha channel — keep the bag small).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Build an `Rgb` from a `#rrggbb` or `rrggbb` hex literal at compile time.
    /// Panics on malformed input — only call with string literals.
    // The panic is a documented contract for malformed color literals (the
    // caller passes string literals, so the branch is unreachable in practice);
    // deliberately kept so misconfiguration fails loudly at startup.
    #[allow(clippy::panic)]
    pub const fn from_hex(hex: &str) -> Self {
        let bytes = hex.as_bytes();
        // accept '#rrggbb' (7) or 'rrggbb' (6)
        let start = if bytes[0] == b'#' { 1 } else { 0 };
        let mut n: u32 = 0;
        let mut i = 0;
        // read exactly 6 hex digits
        while i < 6 {
            let b = bytes[start + i];
            let v = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => panic!("non-hex digit in from_hex"),
            };
            n = (n << 4) | v as u32;
            i += 1;
        }
        Rgb(((n >> 16) & 0xff) as u8, ((n >> 8) & 0xff) as u8, (n & 0xff) as u8)
    }

    /// ANSI 24-bit truecolor escape for this pixel.
    pub fn ansi_fg(self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.0, self.1, self.2)
    }
}

/// Backbone-2 family. Variant naming follows tokens.css exactly.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ThemeVariant {
    #[default]
    Backbone2,
    /// Light surfaces; same accent hues as Backbone-2.
    Backbone2Light,
}

/// Palette + derived decorations.
#[derive(Copy, Clone, Debug)]
pub struct Tokens {
    pub variant: ThemeVariant,
    pub graphite: Rgb,
    pub panel: Rgb,
    pub pulse_green: Rgb,
    pub sync_violet: Rgb,
    pub warm_amber: Rgb,
    pub error: Rgb,
}

impl Tokens {
    /// Canonical Backbone-2 family, matching tokens.css exactly.
    pub const BACKBONE2: Tokens = Tokens {
        variant: ThemeVariant::Backbone2,
        graphite: Rgb::from_hex("#0a0d12"),
        panel: Rgb::from_hex("#161b22"),
        pulse_green: Rgb::from_hex("#3fb950"),
        sync_violet: Rgb::from_hex("#a371f7"),
        warm_amber: Rgb::from_hex("#d29922"),
        error: Rgb::from_hex("#f85149"),
    };

    /// Light variant — paper ground with Backbone-2 accents (tokens.css `[data-theme=light]`).
    pub const BACKBONE2_LIGHT: Tokens = Tokens {
        variant: ThemeVariant::Backbone2Light,
        graphite: Rgb::from_hex("#f6f8fa"),
        panel: Rgb::from_hex("#ffffff"),
        pulse_green: Rgb::from_hex("#1a7f37"),
        sync_violet: Rgb::from_hex("#8250df"),
        warm_amber: Rgb::from_hex("#9a6700"),
        error: Rgb::from_hex("#cf222e"),
    };

    /// Default palette = Backbone-2.
    pub const fn default() -> Self {
        Self::BACKBONE2
    }

    /// Resolve variant by name; case-insensitive on the canonical family name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "backbone-2" | "backbone2" | "bb2" | "dark" => Some(Self::BACKBONE2),
            "backbone-2-light" | "backbone2-light" | "bb2-light" | "light" => {
                Some(Self::BACKBONE2_LIGHT)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_drops_prefix_and_packs_rgb() {
        let raw = Rgb::from_hex("#3fb950");
        assert_eq!(raw, Rgb(0x3f, 0xb9, 0x50));
    }

    #[test]
    fn from_hex_accepts_no_prefix() {
        let raw = Rgb::from_hex("a371f7");
        assert_eq!(raw, Rgb(0xa3, 0x71, 0xf7));
    }

    #[test]
    fn backbone2_constants_match_tokens_css() {
        let t = Tokens::BACKBONE2;
        assert_eq!(t.graphite, Rgb(0x0a, 0x0d, 0x12));
        assert_eq!(t.panel, Rgb(0x16, 0x1b, 0x22));
        assert_eq!(t.pulse_green, Rgb(0x3f, 0xb9, 0x50));
        assert_eq!(t.sync_violet, Rgb(0xa3, 0x71, 0xf7));
        assert_eq!(t.warm_amber, Rgb(0xd2, 0x99, 0x22));
        assert_eq!(t.error, Rgb(0xf8, 0x51, 0x49));
    }

    #[test]
    fn default_is_backbone2() {
        assert_eq!(Tokens::default().variant, ThemeVariant::Backbone2);
    }

    #[test]
    fn from_name_resolves_aliases() {
        assert!(Tokens::from_name("backbone-2").is_some());
        assert!(Tokens::from_name("Backbone2").is_some());
        assert!(Tokens::from_name("BB2").is_some());
        assert!(Tokens::from_name("light").is_some());
        assert_eq!(Tokens::from_name("bb2-light").unwrap().variant, ThemeVariant::Backbone2Light);
        assert!(Tokens::from_name("not-a-family").is_none());
    }

    #[test]
    fn backbone2_light_uses_paper_ground() {
        let t = Tokens::BACKBONE2_LIGHT;
        assert_eq!(t.graphite, Rgb(0xf6, 0xf8, 0xfa));
        assert_eq!(t.panel, Rgb(0xff, 0xff, 0xff));
    }

    #[test]
    fn ansi_fg_emits_truecolor_payload() {
        let s = Tokens::BACKBONE2.pulse_green.ansi_fg();
        assert!(s.starts_with("\x1b[38;2;"));
        assert!(s.contains("63")); // 0x3f = 63
        assert!(s.ends_with("m"));
    }
}
