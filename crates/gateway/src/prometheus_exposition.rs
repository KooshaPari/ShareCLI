//! Prometheus text-exposition format (`version 0.0.4`).
//!
//! Renders a list of `Metric { name, help, metric_type, samples }` into the
//! canonical line-oriented text:
//!
//! ```text
//! # HELP <name> <help>
//! # TYPE <name> counter
//! <name>{label="value"} <number>
//! ```

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

impl MetricType {
    pub fn as_prom_type(self) -> &'static str {
        match self {
            MetricType::Counter => "counter",
            MetricType::Gauge => "gauge",
            MetricType::Histogram => "histogram",
            MetricType::Summary => "summary",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    /// Each sample is `(labels, value)`. A label is `(key, value)`. An empty
    /// label list produces an unlabeled series.
    pub samples: Vec<(Vec<(String, String)>, f64)>,
}

pub fn render(metrics: &[Metric]) -> String {
    let mut out = String::new();
    for m in metrics {
        let _ = writeln!(out, "# HELP {} {}", m.name, m.help);
        let _ = writeln!(out, "# TYPE {} {}", m.name, m.metric_type.as_prom_type());
        for (labels, value) in &m.samples {
            out.push_str(&m.name);
            if !labels.is_empty() {
                out.push('{');
                for (i, (k, v)) in labels.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&format_label(k, v));
                }
                out.push('}');
            }
            out.push(' ');
            push_value(&mut out, *value);
            out.push('\n');
        }
    }
    out
}

fn format_label(k: &str, v: &str) -> String {
    let mut s = String::with_capacity(k.len() + v.len() + 4);
    s.push_str(k);
    s.push_str("=\"");
    for ch in v.chars() {
        match ch {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '\n' => s.push_str("\\n"),
            other => s.push(other),
        }
    }
    s.push('"');
    s
}

fn push_value(out: &mut String, v: f64) {
    if v.is_nan() {
        out.push_str("NaN");
    } else if v.is_infinite() {
        if v > 0.0 {
            out.push_str("+Inf");
        } else {
            out.push_str("-Inf");
        }
    } else if v == v.trunc() && v.abs() < 1e15 {
        let _ = write!(out, "{}", v as i64);
    } else {
        let _ = write!(out, "{}", v);
    }
}

pub fn histogram_buckets(buckets: &[f64]) -> Vec<String> {
    let mut sorted: Vec<f64> = buckets.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::with_capacity(sorted.len() + 1);
    for b in &sorted {
        out.push(format!("{b}"));
    }
    out.push("+Inf".to_string());
    out
}

pub fn format_labels(labels: &[(String, String)]) -> String {
    let mut out = String::new();
    out.push('{');
    for (i, (k, v)) in labels.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format_label(k, v));
    }
    out.push('}');
    out
}
