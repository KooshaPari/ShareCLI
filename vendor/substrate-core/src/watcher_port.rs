//! WatcherPort — debounced filesystem watch events.
//!
//! Core defines the port contract; `file-watcher` wraps the `notify` crate
//! (inotify / FSEvents / ReadDirectoryChangesW) with a debounced stream.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

/// Kind of filesystem change observed by a watcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchEventKind {
    /// A path was created.
    Create,
    /// A path was modified.
    Modify,
    /// A path was removed.
    Remove,
    /// Any other/normalized event kind.
    Other,
}

/// A single debounced watch notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchEvent {
    /// Affected path.
    pub path: PathBuf,
    /// Normalized event kind.
    pub kind: WatchEventKind,
}

/// Subscription handle returned by [`WatcherPort::watch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchHandle {
    /// Adapter-local subscription id.
    pub id: Uuid,
}

/// Filesystem watch port with debounced event delivery.
#[async_trait]
pub trait WatcherPort: Send + Sync {
    /// Begin watching `path` (non-recursive) with `debounce_ms` coalescing.
    async fn watch(&self, path: &Path, debounce_ms: u64) -> Result<WatchHandle>;

    /// Receive the next debounced event, waiting up to `timeout`.
    async fn recv_event(
        &self,
        handle: &WatchHandle,
        timeout: Duration,
    ) -> Result<Option<WatchEvent>>;

    /// Stop watching and release resources for `handle`.
    async fn unwatch(&self, handle: &WatchHandle) -> Result<()>;
}
