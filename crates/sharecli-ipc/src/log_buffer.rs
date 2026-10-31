// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `log_buffer` — fixed-size ring buffer of recent log events for `log.tail` IPC.
//!
//! The buffer is a thread-safe `VecDeque<LogEntry>` guarded by a `Mutex`. New
//! entries are pushed by either:
//!
//! * A `tracing-subscriber` [`Layer`] registered in the IPC server's main()
//!   (so every `tracing::info!/warn!/error!` call from any code path ends up
//!   in the buffer), **or**
//! * A direct `log_buffer().push(...)` from callers that bypass `tracing`
//!   (kept as a stub for future use).
//!
//! `log.tail` reads from the buffer, drops entries with `id <= since_id`,
//! caps the slice at 200 lines, and reports the highest id it has seen via
//! `last_id` so the client can resume from there.
//!
//! Capacity: 1000 entries. When full, oldest entries are evicted on push.
//!
//! Subsystem inference: events emitted via `tracing` carry a module path in
//! their metadata. We map that path to a small fixed set of subsystems the
//! tray expects (`ipc` / `pool` / `gate` / `health` / `config` / `core`).
//! Anything else becomes `core`.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Single ring buffer entry. `id` is monotonic and globally unique within
/// a process — clients use it to resume without re-receiving the entire
/// log history.
#[derive(Clone, Debug, Serialize)]
pub struct LogEntry {
    pub id: u64,
    pub ts: u64,
    pub level: String,
    pub subsystem: String,
    pub msg: String,
}

/// Shared ring buffer + monotonic counter.
pub struct LogBuffer {
    inner: Mutex<Inner>,
}

struct Inner {
    entries: VecDeque<LogEntry>,
    next_id: u64,
    last_id: u64,
}

const CAPACITY: usize = 1000;

impl LogBuffer {
    fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                entries: VecDeque::with_capacity(CAPACITY),
                next_id: 1,
                last_id: 0,
            }),
        }
    }

    /// Append a new entry. Returns the assigned id.
    pub fn push(&self, level: &str, subsystem: &str, msg: impl Into<String>) -> u64 {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut inner = self.inner.lock().expect("log buffer poisoned");
        let id = inner.next_id;
        inner.next_id += 1;
        if inner.entries.len() == CAPACITY {
            inner.entries.pop_front();
        }
        inner.entries.push_back(LogEntry {
            id,
            ts,
            level: level.to_string(),
            subsystem: subsystem.to_string(),
            msg: msg.into(),
        });
        inner.last_id = id;
        id
    }

    /// Read all entries with `id > since_id`, capped at `max` (defaults to 200).
    /// Returns `(entries, last_id)` — `last_id` is the highest id currently in
    /// the buffer (NOT the last entry returned; clients use it to advance the
    /// watermark regardless of filter results).
    pub fn tail(&self, since_id: u64, max: usize) -> (Vec<LogEntry>, u64) {
        let cap = max.min(CAPACITY);
        let inner = self.inner.lock().expect("log buffer poisoned");
        let last_id = inner.last_id;
        let mut out: Vec<LogEntry> =
            inner.entries.iter().filter(|e| e.id > since_id).cloned().collect();
        if out.len() > cap {
            out.truncate(cap);
        }
        (out, last_id)
    }

    /// Snapshot the current `last_id` watermark without taking entries.
    #[allow(dead_code)] // only consumed by bin target's optional diagnostic, not the public lib surface
    pub fn last_id(&self) -> u64 {
        let inner = self.inner.lock().expect("log buffer poisoned");
        inner.last_id
    }
}

/// Global ring buffer (process-wide singleton).
pub fn global() -> &'static LogBuffer {
    static BUF: OnceLock<LogBuffer> = OnceLock::new();
    BUF.get_or_init(LogBuffer::new)
}

/// Map a `tracing` module path to one of the tray's known subsystems.
fn subsystem_for(module_path: &str) -> &'static str {
    if module_path.contains("sharecli_ipc") || module_path.contains("sharecli-ipc") {
        "ipc"
    } else if module_path.contains("pool") {
        "pool"
    } else if module_path.contains("gate") || module_path.contains("thermal") {
        "gate"
    } else if module_path.contains("health") || module_path.contains("monitoring") {
        "health"
    } else if module_path.contains("config") {
        "config"
    } else if module_path.contains("fleet") {
        "pool"
    } else {
        "core"
    }
}

/// Visit a `tracing::Event` and collect its message + fields.
struct MsgVisitor {
    msg: String,
}

impl Visit for MsgVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.msg = format!("{:?}", value);
            // Strip surrounding quotes that Debug adds for &str.
            if self.msg.starts_with('"') && self.msg.ends_with('"') && self.msg.len() >= 2 {
                self.msg = self.msg[1..self.msg.len() - 1].to_string();
            }
        } else {
            if !self.msg.is_empty() {
                self.msg.push(' ');
            }
            self.msg.push_str(&format!("{}={:?}", field.name(), value));
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.msg = value.to_string();
        } else {
            if !self.msg.is_empty() {
                self.msg.push(' ');
            }
            self.msg.push_str(&format!("{}={}", field.name(), value));
        }
    }
}

/// `tracing-subscriber` Layer that forwards every event into the global
/// [`LogBuffer`]. Layer is cheap: one Mutex acquire + one String append per
/// event; no formatting work beyond the `Visit` pass.
pub struct LogBufferLayer;

impl<S> Layer<S> for LogBufferLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = match *event.metadata().level() {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };
        let subsystem = subsystem_for(event.metadata().module_path().unwrap_or("core"));
        let mut visitor = MsgVisitor { msg: String::new() };
        event.record(&mut visitor);
        let msg = if visitor.msg.is_empty() { String::new() } else { visitor.msg };
        global().push(level, subsystem, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_assigns_monotonic_ids() {
        let buf = LogBuffer::new();
        let a = buf.push("INFO", "ipc", "hello");
        let b = buf.push("WARN", "pool", "world");
        let c = buf.push("ERROR", "gate", "boom");
        assert!(b > a && c > b);
        assert_eq!(c, buf.last_id());
    }

    #[test]
    fn tail_returns_only_entries_after_since_id() {
        let buf = LogBuffer::new();
        for i in 0..5 {
            buf.push("INFO", "core", format!("line {i}"));
        }
        let (lines, last_id) = buf.tail(0, 200);
        assert_eq!(lines.len(), 5);
        assert_eq!(last_id, 5);
        // Skip first 2 (ids 1, 2) → expect ids 3, 4, 5.
        let (lines, _) = buf.tail(2, 200);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].id, 3);
        assert_eq!(lines[2].id, 5);
    }

    #[test]
    fn tail_respects_max_cap() {
        let buf = LogBuffer::new();
        for i in 0..50 {
            buf.push("INFO", "core", format!("line {i}"));
        }
        let (lines, _) = buf.tail(0, 10);
        assert_eq!(lines.len(), 10);
    }

    #[test]
    fn capacity_evicts_oldest() {
        let buf = LogBuffer::new();
        // Fill to capacity + 5
        for i in 0..(CAPACITY + 5) {
            buf.push("INFO", "core", format!("line {i}"));
        }
        let (lines, _) = buf.tail(0, CAPACITY * 2);
        assert_eq!(lines.len(), CAPACITY);
        // First returned line should be line 5 (oldest 5 evicted).
        assert!(lines[0].msg.contains("line 5"));
    }

    #[test]
    fn subsystem_classifier_maps_paths() {
        assert_eq!(subsystem_for("sharecli_ipc::handler"), "ipc");
        assert_eq!(subsystem_for("crate::pool::manager"), "pool");
        assert_eq!(subsystem_for("crate::gate::decision"), "gate");
        assert_eq!(subsystem_for("crate::health::snapshot"), "health");
        assert_eq!(subsystem_for("crate::config::loader"), "config");
        assert_eq!(subsystem_for("crate::random::thing"), "core");
    }

    #[test]
    fn global_returns_same_instance() {
        let a = global();
        let b = global();
        assert!(std::ptr::eq(a as *const _, b as *const _));
    }
}
