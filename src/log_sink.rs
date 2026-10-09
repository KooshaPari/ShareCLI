use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Where a process routes its human-readable tracing output.
///
/// Audit task 1.11 (`docs/audit/2026-09-20/PLAN.md` lines 183-185).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSinkRoute {
    /// Interactive console (stderr is a TTY, or `--verbose`): mirror events to
    /// stderr.
    Console,
    /// Daemonized / piped (stderr is not a TTY): never write to stderr, so the
    /// FR-007 stderr-silent contract holds; events go to the file sink only.
    Daemon,
}

/// Resolved subscriber configuration for one process start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriberPlan {
    /// Whether to install the global subscriber. `--quiet` is the only opt-out.
    pub install: bool,
    /// Where human-readable output is routed.
    pub route: LogSinkRoute,
    /// Global level filter: DEBUG with `--verbose`, INFO otherwise.
    pub level: tracing::Level,
}

/// Decide the subscriber configuration from CLI flags and the stderr TTY.
///
/// Audit task 1.11: the subscriber is installed even when stderr is not a TTY,
/// so a daemonized `sharecli serve` emits logs. The stderr console layer is
/// attached only on [`LogSinkRoute::Console`]; the file sink carries the log
/// for the daemon route.
pub fn plan_subscriber(quiet: bool, verbose: bool, stderr_is_tty: bool) -> SubscriberPlan {
    let level = if verbose { tracing::Level::DEBUG } else { tracing::Level::INFO };
    let route = if verbose || stderr_is_tty { LogSinkRoute::Console } else { LogSinkRoute::Daemon };
    SubscriberPlan { install: !quiet, route, level }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn from_level(level: &tracing::Level) -> Self {
        if level == &tracing::Level::ERROR {
            LogLevel::Error
        } else if level == &tracing::Level::WARN {
            LogLevel::Warn
        } else if level == &tracing::Level::INFO {
            LogLevel::Info
        } else {
            LogLevel::Debug
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone)]
pub struct LogSink {
    buffer: Arc<Mutex<VecDeque<LogEntry>>>,
    capacity: usize,
}
impl LogSink {
    pub fn new(capacity: usize) -> Self {
        Self { buffer: Arc::new(Mutex::new(VecDeque::new())), capacity }
    }
    pub fn write(&self, level: LogLevel, msg: impl Into<String>) {
        let mut buf = self.buffer.lock().expect("LogSink buffer mutex poisoned");
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(LogEntry { level, message: msg.into() });
    }
    pub fn info(&self, msg: impl Into<String>) {
        self.write(LogLevel::Info, msg);
    }
    pub fn warn(&self, msg: impl Into<String>) {
        self.write(LogLevel::Warn, msg);
    }
    pub fn error(&self, msg: impl Into<String>) {
        self.write(LogLevel::Error, msg);
    }
    pub fn drain(&self) -> Vec<LogEntry> {
        self.buffer.lock().expect("LogSink buffer mutex poisoned").drain(..).collect()
    }
    pub fn len(&self) -> usize {
        self.buffer.lock().expect("LogSink buffer mutex poisoned").len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drain all buffered entries and re-emit them via `tracing`.
    ///
    /// Useful when the LogSink collects entries from a subsystem that doesn't
    /// participate in the tracing ecosystem, and you want them to appear in
    /// structured logs or OTLP export.
    pub fn flush_to_tracing(&self) {
        let entries = self.drain();
        for entry in &entries {
            match entry.level {
                LogLevel::Debug => tracing::debug!(target: "log_sink", "{}", entry.message),
                LogLevel::Info => tracing::info!(target: "log_sink", "{}", entry.message),
                LogLevel::Warn => tracing::warn!(target: "log_sink", "{}", entry.message),
                LogLevel::Error => tracing::error!(target: "log_sink", "{}", entry.message),
            }
        }
    }
}

/// A `tracing::Layer` that captures events into a [`LogSink`] buffer.
///
/// Attach this layer to the tracing subscriber so all events that pass
/// through the subscriber are also available in the LogSink for UI display
/// (tray / serve dashboard).
///
/// # Example
///
/// ```ignore
/// let sink = LogSink::new(256);
/// let layer = LogSinkLayer::new(sink.clone());
/// tracing_subscriber::registry().with(layer).init();
/// ```
pub struct LogSinkLayer {
    sink: LogSink,
}

impl LogSinkLayer {
    pub fn new(sink: LogSink) -> Self {
        Self { sink }
    }
}

impl<S: Subscriber> Layer<S> for LogSinkLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let level = LogLevel::from_level(event.metadata().level());
        // Format the event using a visitor that collects the message.
        struct MessageVisitor(String);
        impl tracing::field::Visit for MessageVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                } else if self.0.is_empty() {
                    self.0 = format!("{}={value:?}", field.name());
                } else {
                    self.0.push_str(&format!(", {}={value:?}", field.name()));
                }
            }
        }
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);
        let message =
            if visitor.0.is_empty() { format!("{}", event.metadata().target()) } else { visitor.0 };
        self.sink.write(level, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plan_routes_console_for_a_tty_or_verbose() {
        let interactive = plan_subscriber(false, false, true);
        assert!(interactive.install, "only --quiet opts out of installing the subscriber");
        assert_eq!(interactive.route, LogSinkRoute::Console);
        assert_eq!(interactive.level, tracing::Level::INFO);

        let verbose = plan_subscriber(false, true, false);
        assert_eq!(verbose.route, LogSinkRoute::Console);
        assert_eq!(verbose.level, tracing::Level::DEBUG);
    }

    #[test]
    fn plan_routes_daemon_for_a_piped_stderr() {
        let daemon = plan_subscriber(false, false, false);
        assert_eq!(daemon.route, LogSinkRoute::Daemon);
        assert!(daemon.install, "a daemonized serve must still install the subscriber");
        assert_eq!(daemon.level, tracing::Level::INFO);
    }

    #[test]
    fn plan_honours_quiet() {
        assert!(!plan_subscriber(true, false, true).install);
        assert!(!plan_subscriber(true, true, false).install);
    }

    #[test]
    fn from_level_maps_every_tracing_level() {
        assert_eq!(LogLevel::from_level(&tracing::Level::ERROR), LogLevel::Error);
        assert_eq!(LogLevel::from_level(&tracing::Level::WARN), LogLevel::Warn);
        assert_eq!(LogLevel::from_level(&tracing::Level::INFO), LogLevel::Info);
        assert_eq!(LogLevel::from_level(&tracing::Level::DEBUG), LogLevel::Debug);
        assert_eq!(LogLevel::from_level(&tracing::Level::TRACE), LogLevel::Debug);
    }

    #[test]
    fn write_and_drain() {
        let s = LogSink::new(10);
        s.info("hi");
        let d = s.drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].message, "hi");
    }
    #[test]
    fn capacity_evicts() {
        let s = LogSink::new(2);
        s.info("a");
        s.info("b");
        s.info("c");
        assert_eq!(s.len(), 2);
        let d = s.drain();
        assert_eq!(d[0].message, "b");
    }
    #[test]
    fn levels() {
        let s = LogSink::new(10);
        s.warn("w");
        s.error("e");
        let d = s.drain();
        assert_eq!(d[0].level, LogLevel::Warn);
        assert_eq!(d[1].level, LogLevel::Error);
    }
    #[test]
    fn drain_empties() {
        let s = LogSink::new(5);
        s.info("x");
        s.drain();
        assert!(s.is_empty());
    }
    #[test]
    fn empty_initially() {
        assert!(LogSink::new(5).is_empty());
    }
    #[test]
    fn clone_shares() {
        let s = LogSink::new(5);
        let t = s.clone();
        t.info("shared");
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn flush_to_tracing_drains_entries() {
        let s = LogSink::new(10);
        s.info("before flush");
        s.warn("also before");
        s.flush_to_tracing();
        // After flush, buffer is empty.
        assert!(s.is_empty());
    }

    #[test]
    fn log_sink_layer_captures_events() {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::Registry;

        let sink = LogSink::new(64);
        let layer = LogSinkLayer::new(sink.clone());
        let subscriber = Registry::default().with(layer);

        let _guard = tracing::subscriber::set_default(subscriber);
        tracing::info!("layer capture test");

        assert_eq!(sink.len(), 1);
        let entries = sink.drain();
        assert_eq!(entries[0].level, LogLevel::Info);
        assert!(entries[0].message.contains("layer capture test"));
    }
}
