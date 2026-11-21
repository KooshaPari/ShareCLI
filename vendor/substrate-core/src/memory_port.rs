//! MemoryPort — two-tier agent memory (recent ring + persistent history).
//!
//! Core defines the contract; `substrate-memory` provides ring-buffer and
//! composed implementations; `store-sqlite` may back the persistent tier.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single memory record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Stable record id.
    pub id: Uuid,
    /// Logical key (topic, session, etc.).
    pub key: String,
    /// Stored text payload.
    pub content: String,
    /// Unix epoch seconds when the entry was written.
    pub created_at: i64,
}

/// Agent memory: recent window + durable history.
pub trait MemoryPort: Send + Sync {
    /// Error type returned by memory operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Append a new entry under `key` with `content`. Returns the new id.
    fn append(&self, key: &str, content: &str) -> Result<Uuid, Self::Error>;

    /// Return the latest value for `key`, if any.
    fn get(&self, key: &str) -> Result<Option<String>, Self::Error>;

    /// Return up to `limit` most-recent entries (newest first).
    fn recent(&self, limit: usize) -> Result<Vec<MemoryEntry>, Self::Error>;

    /// Return full durable history (newest first).
    fn history(&self) -> Result<Vec<MemoryEntry>, Self::Error>;
}
