// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Minimal RFC-6902 JSON Patch (add/replace/remove) for a flat in-memory
//! `JsValue` document. Only `/<key>` paths are supported; array indices and
//! nested pointer expressions are out of scope for this MVP.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    /// Object entries kept in insertion order. `Vec<(String, JsValue)>` is
    /// used so the round-trip preserves order without pulling in `IndexMap`.
    Object(Vec<(String, JsValue)>),
}

impl JsValue {
    pub fn from_number(n: f64) -> Self {
        JsValue::Number(n)
    }
    pub fn from_string(s: impl Into<String>) -> Self {
        JsValue::String(s.into())
    }
    pub fn from_bool(b: bool) -> Self {
        JsValue::Bool(b)
    }
    pub fn is_object(&self) -> bool {
        matches!(self, JsValue::Object(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Patch {
    Add(String, JsValue),
    Replace(String, JsValue),
    Remove(String),
}

#[derive(Debug)]
pub struct PatchError(pub String);

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "patch: {}", self.0)
    }
}

impl std::error::Error for PatchError {}

pub fn apply(doc: &mut JsValue, patches: &[Patch]) -> Result<(), PatchError> {
    for patch in patches {
        match patch {
            Patch::Add(path, value) => add(doc, path, value.clone())?,
            Patch::Replace(path, value) => replace(doc, path, value.clone())?,
            Patch::Remove(path) => remove(doc, path)?,
        }
    }
    Ok(())
}

fn key_from_path(path: &str) -> Result<&str, PatchError> {
    if let Some(k) = path.strip_prefix('/') {
        if k.is_empty() {
            return Err(PatchError(format!("path `{path}` is empty")));
        }
        Ok(k)
    } else {
        Err(PatchError(format!("unsupported path `{path}` (MVP supports `/<key>` only)")))
    }
}

fn add(doc: &mut JsValue, path: &str, value: JsValue) -> Result<(), PatchError> {
    let key = key_from_path(path)?.to_string();
    match doc {
        JsValue::Object(entries) => {
            if let Some((_, v)) = entries.iter_mut().find(|(k, _)| k == &key) {
                *v = value;
            } else {
                entries.push((key, value));
            }
            Ok(())
        }
        _ => Err(PatchError("add target is not an object".into())),
    }
}

fn replace(doc: &mut JsValue, path: &str, value: JsValue) -> Result<(), PatchError> {
    let key = key_from_path(path)?;
    match doc {
        JsValue::Object(entries) => {
            let pos = entries
                .iter()
                .position(|(k, _)| k == key)
                .ok_or_else(|| PatchError(format!("replace: key `{key}` not found")))?;
            entries[pos].1 = value;
            Ok(())
        }
        _ => Err(PatchError("replace target is not an object".into())),
    }
}

fn remove(doc: &mut JsValue, path: &str) -> Result<(), PatchError> {
    let key = key_from_path(path)?;
    match doc {
        JsValue::Object(entries) => {
            let pos = entries
                .iter()
                .position(|(k, _)| k == key)
                .ok_or_else(|| PatchError(format!("remove: key `{key}` not found")))?;
            entries.remove(pos);
            Ok(())
        }
        _ => Err(PatchError("remove target is not an object".into())),
    }
}
