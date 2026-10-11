//! Content-addressed revision of the operator config.
//!
//! Phase 1 task 1.5 (lane 3 HIGH): `config.set` is a blind read-modify-write, so
//! the tray and the CLI can silently clobber each other's edits. The fix
//! references RFC 9110 `If-Match`: a client reads the revision once, echoes it
//! back as `if_revision`, and a write whose guard no longer matches is refused
//! with `CONFLICT` instead of overwriting.
//!
//! The revision is a fingerprint of the config *content* rather than a
//! persisted counter, so it needs no storage, cannot drift across a sidecar
//! restart, and is reproducible by any client that serializes the config the
//! same way.
//!
//! ## Canonicalization
//!
//! [`Config`] stores its maps as [`std::collections::HashMap`], whose iteration
//! order is randomized per process. Serializing the struct directly would emit a
//! different byte string for the same config on every run. Round-tripping
//! through [`serde_json::Value`] first fixes that: this crate builds
//! `serde_json` *without* the `preserve_order` feature, so `serde_json::Map` is
//! a `BTreeMap` and every object key is sorted at every nesting depth — which
//! makes [`Value`] a canonical encoding by construction. SHA-256 over that
//! string is stable across runs, processes, and machines.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use sharecli::config::Config;

/// Hex SHA-256 of the canonical JSON encoding of `config`.
///
/// Pure *content* fingerprint of the in-memory value. Prefer
/// [`revision_with_disk`] for guard decisions — on its own it is blind to
/// edits made to the config file by other processes.
pub fn revision_of(config: &Config) -> Result<String> {
    let value = serde_json::to_value(config).context("config is not serializable")?;
    let canonical =
        serde_json::to_string(&value).context("config revision could not be encoded")?;
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// Guard token: content fingerprint of `config` mixed with the bytes currently
/// on disk at [`Config::config_path`].
///
/// CodeRabbit CRITICAL #3 (task 1.5): hashing only the in-memory value let an
/// *out-of-process* edit of the config file pass the `if_revision` guard —
/// the daemon's digest never moved, so a held token stayed valid while the
/// file underneath it changed, and the guarded write clobbered the external
/// edit. Mixing the on-disk bytes into the digest means any external write
/// (even a comment-only change) advances the revision and invalidates held
/// tokens.
///
/// Tokens are issued by the `config.revision` endpoint and echoed back by
/// clients; no client recomputes them, so folding local disk state in does not
/// break the protocol. Absent/unreadable file hashes to a fixed sentinel so a
/// first-time save still advances the token.
pub fn revision_with_disk(config: &Config) -> Result<String> {
    let base = revision_of(config)?;
    let disk = match Config::config_path() {
        Ok(path) => std::fs::read(path).unwrap_or_else(|_| b"<absent>".to_vec()),
        Err(_) => b"<no-path>".to_vec(),
    };
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    hasher.update(b"\n");
    hasher.update(&disk);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// Canonicalization: two independently constructed `Config::default()`
    /// values share their data but not their `RandomState`, so their `HashMap`s
    /// iterate in different orders. A direct `serde_json::to_string(&config)`
    /// would therefore produce different bytes and fail here; routing through
    /// `Value` makes the digest independent of that order.
    #[test]
    fn independently_built_equal_configs_share_one_revision() {
        let first = Config::default();
        let second = Config::default();

        assert_eq!(
            revision_of(&first).unwrap(),
            revision_of(&second).unwrap(),
            "revision must not depend on HashMap iteration order"
        );
    }

    /// Rebuilding a map in a different insertion order must not move the
    /// revision: only content decides.
    #[test]
    fn rebuilding_a_map_in_another_order_keeps_the_revision() {
        let mut base = Config::default();
        let original = revision_of(&base).unwrap();

        let mut reversed = HashMap::with_capacity(base.projects.len());
        let mut entries: Vec<(String, String)> =
            base.projects.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        entries.reverse();
        for (key, value) in entries {
            reversed.insert(key, value);
        }
        base.projects = reversed;

        assert_eq!(original, revision_of(&base).unwrap());
    }

    /// Idempotence: asking twice about the same value gives the same answer.
    #[test]
    fn revision_of_the_same_config_is_repeatable() {
        let config = Config::default();

        assert_eq!(revision_of(&config).unwrap(), revision_of(&config).unwrap());
    }

    /// Tracking: a content change must advance the revision, otherwise a guard
    /// built from the old value would keep passing after an edit.
    #[test]
    fn revision_advances_when_a_field_changes() {
        let mut config = Config::default();
        let before = revision_of(&config).unwrap();

        config.runtime.max_processes =
            Some(config.runtime.max_processes.unwrap_or(0).saturating_add(1));
        let after = revision_of(&config).unwrap();

        assert_ne!(before, after, "an edited config must get a new revision");
    }

    /// The digest is a real SHA-256 hex string, so clients can compare it
    /// verbatim without normalizing.
    #[test]
    fn revision_is_lowercase_hex_sha256() {
        let revision = revision_of(&Config::default()).unwrap();

        assert_eq!(revision.len(), 64, "sha256 hex must be 64 characters");
        assert!(
            revision.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "revision must be lowercase hex, got {revision}"
        );
    }
}
