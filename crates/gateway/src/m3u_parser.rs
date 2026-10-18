// SPDX-License-Identifier: Apache-2.0 OR MIT
//! M3U / M3U8 playlist parse + render.
//!
//! Parses a text playlist into `M3uEntry { duration_secs, title, uri }`.
//! Recognizes:
//! - `#EXTM3U` header (required)
//! - `#EXTINF:<dur>,<title>` per entry (optional but recommended)
//! - URI/path lines (any other non-empty, non-`#`-prefixed line)

#[derive(Debug, Clone, PartialEq)]
pub struct M3uEntry {
    pub duration_secs: Option<f64>,
    pub title: Option<String>,
    pub uri: String,
}

impl M3uEntry {
    fn path(uri: String) -> Self {
        M3uEntry { duration_secs: None, title: None, uri }
    }

    fn extinf(duration: f64, title: String, uri: String) -> Self {
        M3uEntry { duration_secs: Some(duration), title: Some(title), uri }
    }
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub items: Vec<M3uEntry>,
}

pub fn parse(text: &str) -> Vec<M3uEntry> {
    let mut items = Vec::new();
    let mut pending: Option<(f64, String)> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#EXTM3U") {
            continue;
        }
        if line.starts_with("#EXT-X-") || line.starts_with("#EXT-ALBUMART") {
            // ignore advanced M3U8 directives for this MVP
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let (dur_str, title) = match rest.find(',') {
                Some(i) => (&rest[..i], rest[i + 1..].to_string()),
                None => (rest, String::new()),
            };
            let dur = dur_str.parse::<f64>().unwrap_or(-1.0);
            pending = Some((dur, title));
            continue;
        }
        if line.starts_with('#') {
            // unknown directive; ignore
            continue;
        }
        match pending.take() {
            Some((d, t)) => items.push(M3uEntry::extinf(d, t, line.to_string())),
            None => items.push(M3uEntry::path(line.to_string())),
        }
    }
    items
}

/// Backwards-compat alias: returns a `Playlist` wrapper around the entries.
/// Older callers that still expect the `Playlist` shape can use this.
#[allow(dead_code)]
pub fn parse_playlist(text: &str) -> Playlist {
    Playlist { items: parse(text) }
}

pub fn render(entries: &[M3uEntry]) -> String {
    let mut out = String::new();
    out.push_str("#EXTM3U\n");
    for entry in entries {
        match (entry.duration_secs, entry.title.as_ref()) {
            (Some(d), Some(t)) => {
                out.push_str(&format!("#EXTINF:{},{}\n", d, t));
            }
            (Some(d), None) => {
                out.push_str(&format!("#EXTINF:{}\n", d));
            }
            (None, _) => {}
        }
        out.push_str(&entry.uri);
        out.push('\n');
    }
    out
}

/// Backwards-compat alias: render a `Playlist` wrapper.
#[allow(dead_code)]
pub fn render_playlist(pl: &Playlist) -> String {
    render(&pl.items)
}

pub fn is_extinf_line(line: &str) -> bool {
    line.trim_start().starts_with("#EXTINF:")
}
