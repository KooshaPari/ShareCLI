//! Dotted-path get/set for `sharecli config`.
//!
//! The CLI speaks dotted TOML paths (`runtime.max_memory_mb`). Values are
//! resolved and assigned through a `serde_json::Value` projection of `Config`,
//! so one helper covers scalars, arrays and nested tables without a
//! hand-written accessor per field.

use anyhow::{bail, Result};
use serde_json::Value;

/// Split a dotted key into segments, rejecting empty segments so that
/// `config get ""` and `config get a..b` fail loudly instead of resolving to
/// something arbitrary.
pub fn split_path(key: &str) -> Result<Vec<&str>> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|p| p.trim().is_empty()) {
        bail!("invalid configuration key: {key:?}");
    }
    Ok(parts)
}

/// Resolve `path` inside `root`, returning `None` when any segment is absent.
pub fn lookup<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path {
        cur = cur.as_object()?.get(*seg)?;
    }
    Some(cur)
}

/// Assign `new` at `path`, creating intermediate tables as needed.
pub fn assign(root: &mut Value, path: &[&str], new: Value) -> Result<()> {
    if path.is_empty() {
        *root = new;
        return Ok(());
    }
    let map = root.as_object_mut().ok_or_else(|| anyhow::anyhow!("'{}' is not a table", path[0]))?;
    let entry =
        map.entry(path[0].to_string()).or_insert_with(|| Value::Object(serde_json::Map::new()));
    assign(entry, &path[1..], new)
}

/// Interpret a CLI string using the type already stored at the path, so that
/// `config set runtime.max_memory_mb 8192` stores a number rather than the
/// string `"8192"`, while a value for a string field such as `runtime.node_path`
/// stays a string even if it looks numeric.
pub fn coerce(existing: Option<&Value>, raw: &str) -> Value {
    match existing {
        Some(Value::Bool(_)) => {
            raw.parse::<bool>().map(Value::Bool).unwrap_or_else(|_| Value::String(raw.to_string()))
        }
        Some(Value::Number(_)) => {
            if let Ok(n) = raw.parse::<i64>() {
                return Value::from(n);
            }
            if let Ok(n) = raw.parse::<u64>() {
                return Value::from(n);
            }
            if let Ok(n) = raw.parse::<f64>() {
                return Value::from(n);
            }
            Value::String(raw.to_string())
        }
        Some(Value::String(_)) => Value::String(raw.to_string()),
        // Unknown or container path: prefer a JSON literal, else a plain string.
        _ => serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string())),
    }
}

/// Render a value the way a TOML scalar would read on the command line.
pub fn render(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn split_path_rejects_empty_segments() {
        assert!(split_path("runtime.max_memory_mb").is_ok());
        assert!(split_path("").is_err());
        assert!(split_path("a..b").is_err());
        assert!(split_path("a.").is_err());
    }

    #[test]
    fn lookup_resolves_nested_values() {
        let root = json!({ "runtime": { "max_memory_mb": 4096 } });
        assert_eq!(lookup(&root, &["runtime", "max_memory_mb"]), Some(&json!(4096)));
    }

    #[test]
    fn lookup_returns_none_for_unknown_path() {
        let root = json!({ "runtime": { "max_memory_mb": 4096 } });
        assert_eq!(lookup(&root, &["runtime", "nope"]), None);
        assert_eq!(lookup(&root, &["nope", "max_memory_mb"]), None);
        assert_eq!(lookup(&root, &["runtime", "max_memory_mb", "deeper"]), None);
    }

    #[test]
    fn assign_updates_existing_and_creates_missing_tables() {
        let mut root = json!({ "runtime": { "max_memory_mb": 4096 } });
        assign(&mut root, &["runtime", "max_memory_mb"], json!(8192)).unwrap();
        assert_eq!(root["runtime"]["max_memory_mb"], json!(8192));

        assign(&mut root, &["new_section", "leaf"], json!(1)).unwrap();
        assert_eq!(root["new_section"]["leaf"], json!(1));
    }

    #[test]
    fn assign_refuses_to_replace_a_scalar_with_a_table() {
        let mut root = json!({ "runtime": 7 });
        assert!(assign(&mut root, &["runtime", "leaf"], json!(1)).is_err());
    }

    #[test]
    fn coerce_preserves_declared_types() {
        // Numeric field stays numeric even though the CLI passes a string.
        assert_eq!(coerce(Some(&json!(4096)), "8192"), json!(8192));
        // Negative numbers resolve as signed integers.
        assert_eq!(coerce(Some(&json!(4096)), "-5"), json!(-5));
        // Non-numeric input for a numeric field degrades to a string and will
        // be rejected later by the config validation gate.
        assert_eq!(coerce(Some(&json!(4096)), "many"), json!("many"));
        // Boolean field.
        assert_eq!(coerce(Some(&json!(true)), "false"), json!(false));
        // String field keeps a path-like value verbatim.
        assert_eq!(coerce(Some(&json!("x")), "/usr/bin/node"), json!("/usr/bin/node"));
        // Unknown path: JSON literal preferred, else plain string.
        assert_eq!(coerce(None, "8192"), json!(8192));
        assert_eq!(coerce(None, "/usr/bin/node"), json!("/usr/bin/node"));
    }

    #[test]
    fn render_unquotes_strings_only() {
        assert_eq!(render(&json!("4096")), "4096");
        assert_eq!(render(&json!(4096)), "4096");
        assert_eq!(render(&json!(true)), "true");
    }
}
