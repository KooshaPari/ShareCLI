//! Pressure observation domain and Linux PSI parser.
//! Additive only; no scheduling or enforcement side effects.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StallWindow {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PressureSnapshot {
    pub resource: String,
    pub observed_at_unix_ms: i64,
    pub source: String,
    pub some: StallWindow,
    pub full: Option<StallWindow>,
}

pub trait PressureProvider {
    fn id(&self) -> &str;
    fn sample(&self) -> Result<PressureSnapshot, String>;
}

fn parse_line(line: &str) -> Result<(&str, StallWindow), String> {
    let mut it = line.split_whitespace();
    let kind = it.next().ok_or_else(|| "missing pressure kind".to_string())?;
    let mut avg10 = None;
    let mut avg60 = None;
    let mut avg300 = None;
    let mut total = None;

    for token in it {
        let (k, v) = token
            .split_once('=')
            .ok_or_else(|| format!("invalid pressure token {token:?}"))?;
        match k {
            "avg10" => avg10 = Some(v.parse::<f64>().map_err(|e| e.to_string())?),
            "avg60" => avg60 = Some(v.parse::<f64>().map_err(|e| e.to_string())?),
            "avg300" => avg300 = Some(v.parse::<f64>().map_err(|e| e.to_string())?),
            "total" => total = Some(v.parse::<u64>().map_err(|e| e.to_string())?),
            _ => {}
        }
    }

    let window = StallWindow {
        avg10: avg10.ok_or_else(|| "missing avg10".to_string())?,
        avg60: avg60.ok_or_else(|| "missing avg60".to_string())?,
        avg300: avg300.ok_or_else(|| "missing avg300".to_string())?,
        total_micros: total.ok_or_else(|| "missing total".to_string())?,
    };
    for (name, value) in [
        ("avg10", window.avg10),
        ("avg60", window.avg60),
        ("avg300", window.avg300),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!("invalid pressure {name}={value}"));
        }
    }

    Ok((kind, window))
}

pub fn parse_linux_psi(
    resource: impl Into<String>,
    source: impl Into<String>,
    observed_at_unix_ms: i64,
    raw: &str,
) -> Result<PressureSnapshot, String> {
    let mut some = None;
    let mut full = None;

    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let (kind, window) = parse_line(line)?;
        match kind {
            "some" => some = Some(window),
            "full" => full = Some(window),
            other => return Err(format!("unknown pressure kind {other:?}")),
        }
    }

    Ok(PressureSnapshot {
        resource: resource.into(),
        observed_at_unix_ms,
        source: source.into(),
        some: some.ok_or_else(|| "missing some pressure line".to_string())?,
        full,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_memory_pressure() {
        let p = parse_linux_psi(
            "memory",
            "/proc/pressure/memory",
            123,
            "some avg10=1.25 avg60=0.50 avg300=0.10 total=12345\nfull avg10=0.25 avg60=0.10 avg300=0.01 total=2345\n",
        )
        .unwrap();
        assert_eq!(p.resource, "memory");
        assert_eq!(p.some.avg10, 1.25);
        assert_eq!(p.full.as_ref().unwrap().total_micros, 2345);
    }

    #[test]
    fn accepts_cpu_psi_without_full_line() {
        let p = parse_linux_psi(
            "cpu",
            "/proc/pressure/cpu",
            123,
            "some avg10=2.00 avg60=1.00 avg300=0.50 total=999\n",
        )
        .unwrap();
        assert!(p.full.is_none());
    }

    #[test]
    fn rejects_missing_some_line() {
        let err = parse_linux_psi(
            "memory",
            "/proc/pressure/memory",
            123,
            "full avg10=0.1 avg60=0.1 avg300=0.1 total=1\n",
        )
        .unwrap_err();
        assert!(err.contains("missing some"));
    }

    #[test]
    fn rejects_nonfinite_or_negative_pressure_values() {
        for invalid in ["NaN", "inf", "-1.0"] {
            let raw = format!(
                "some avg10={invalid} avg60=0 avg300=0 total=1\n"
            );
            assert!(parse_linux_psi("memory", "/proc/pressure/memory", 123, &raw).is_err());
        }
    }

    #[test]
    fn rejects_malformed_pressure_tokens() {
        assert!(parse_linux_psi(
            "io",
            "/proc/pressure/io",
            123,
            "some avg10=x avg60=0 avg300=0 total=1\n",
        )
        .is_err());
    }
}
