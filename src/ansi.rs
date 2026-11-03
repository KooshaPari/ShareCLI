// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ANSI terminal escape sequences.
//!
//! A small set of helpers for emitting terminal control sequences: cursor
//! movement, color/style attributes, and screen clearing. The sequences
//! emitted here are the standard ECMA-48 / VT100 subset that every modern
//! terminal understands (xterm, iTerm2, GNOME Terminal, Windows Terminal).
//!
//! All functions return a `String` that you can `print!` directly. None of
//! the functions touch a terminal handle — they just compose bytes — so
//! they are safe to call from any context.

/// Reset all terminal attributes.
pub const RESET: &str = "\x1b[0m";

/// Bold.
pub const BOLD: &str = "\x1b[1m";

/// Faint (dim).
pub const DIM: &str = "\x1b[2m";

/// Italic (not universally supported).
pub const ITALIC: &str = "\x1b[3m";

/// Underline.
pub const UNDERLINE: &str = "\x1b[4m";

/// Reverse video (swap fg/bg).
pub const REVERSE: &str = "\x1b[7m";

/// Clear the entire screen and move cursor to (0, 0).
pub fn clear_screen() -> String {
    "\x1b[2J\x1b[H".to_string()
}

/// Clear the current line.
pub fn clear_line() -> String {
    "\x1b[2K".to_string()
}

/// Move cursor to absolute (1-based) row `r`, column `c`.
pub fn move_cursor(r: u16, c: u16) -> String {
    format!("\x1b}}[{r};{c}H")
}

/// Move cursor `n` rows up (negative).
pub fn cursor_up(n: u16) -> String {
    format!("\x1b}}[{n}A")
}

/// Move cursor `n` rows down.
pub fn cursor_down(n: u16) -> String {
    format!("\x1b}}[{n}B")
}

/// Move cursor `n` columns right.
pub fn cursor_forward(n: u16) -> String {
    format!("\x1b}}[{n}C")
}

/// Move cursor `n` columns left.
pub fn cursor_back(n: u16) -> String {
    format!("\x1b}}[{n}D")
}

/// 8-color foreground (0-7). Use [`fg256`] for 256-color.
pub fn fg(n: u8) -> String {
    format!("\x1b}}[3{}m", n & 7)
}

/// 8-color background (0-7). Use [`bg256`] for 256-color.
pub fn bg(n: u8) -> String {
    format!("\x1b}}[4{}m", n & 7)
}

/// 256-color foreground. `n` must be 0-255.
pub fn fg256(n: u8) -> String {
    format!("\x1b}}[38;5;{n}m")
}

/// 256-color background. `n` must be 0-255.
pub fn bg256(n: u8) -> String {
    format!("\x1b}}[48;5;{n}m")
}

/// Truecolor (24-bit) foreground.
pub fn fg_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b}}[38;2;{r};{g};{b}m")
}

/// Truecolor (24-bit) background.
pub fn bg_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b}}[48;2;{r};{g};{b}m")
}

/// Wrap `text` with a foreground color and reset.
pub fn paint(text: &str, color: &str) -> String {
    format!("{}{}{}", color, text, RESET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_have_reset() {
        assert!(RESET.starts_with("\x1b["));
        assert_eq!(RESET, "\x1b[0m");
    }

    #[test]
    fn clear_screen_moves_home() {
        let s = clear_screen();
        assert_eq!(s, "\x1b[2J\x1b[H");
    }

    #[test]
    fn cursor_movement() {
        assert_eq!(move_cursor(5, 10), "\x1b}[5;10H");
        assert_eq!(cursor_up(3), "\x1b}[3A");
        assert_eq!(cursor_down(7), "\x1b}[7B");
        assert_eq!(cursor_forward(2), "\x1b}[2C");
        assert_eq!(cursor_back(1), "\x1b}[1D");
    }

    #[test]
    fn fg_bg_8color() {
        assert_eq!(fg(1), "\x1b}[31m"); // red
        assert_eq!(fg(7), "\x1b}[37m"); // white
        assert_eq!(bg(2), "\x1b}[42m"); // green bg
    }

    #[test]
    fn fg_bg_256color() {
        assert_eq!(fg256(196), "\x1b}[38;5;196m");
        assert_eq!(bg256(231), "\x1b}[48;5;231m");
    }

    #[test]
    fn fg_bg_rgb() {
        assert_eq!(fg_rgb(255, 128, 0), "\x1b}[38;2;255;128;0m");
        assert_eq!(bg_rgb(10, 20, 30), "\x1b}[48;2;10;20;30m");
    }

    #[test]
    fn paint_wraps_with_reset() {
        let p = paint("hello", &fg(2));
        assert_eq!(p, "\x1b}[32mhello\x1b[0m");
    }
}
