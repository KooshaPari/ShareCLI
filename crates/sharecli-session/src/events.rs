//! Bounded, sequenced surface events for live terminal I/O.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use base64::Engine;
use serde::{Deserialize, Serialize};

/// Hard upper bound on a single surface event chunk.
pub const MAX_EVENT_CHUNK_BYTES: usize = 64 * 1024;
/// Maximum number of queued events per surface subscription.
pub const MAX_EVENT_QUEUE_CAPACITY: usize = 256;

/// Kinds of surface event delivered to subscribers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceEventKind {
    /// PTY/process output bytes for the surface.
    Output,
    /// Surface geometry changed (resize, split, etc).
    Resize,
    /// Process exit code (with the process id when known).
    Exit,
    /// Surface title changed.
    Title,
    /// Surface working directory changed.
    Cwd,
    /// Events were dropped because the subscription queue was saturated.
    Dropped,
}

/// Per-event payload delivered to subscribers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceEventParams {
    /// Subscription id this event belongs to.
    pub subscription_id: u64,
    /// Surface identifier this event belongs to.
    pub surface_id: String,
    /// Monotonic sequence number assigned by the hub.
    pub seq: u64,
    /// Event kind discriminator.
    pub kind: SurfaceEventKind,
    /// Optional event timestamp (ISO 8601) as captured by the source.
    pub timestamp: Option<String>,
    /// Optional base64-encoded raw output bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_bytes_base64: Option<String>,
    /// Optional count of events dropped prior to this one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropped: Option<u64>,
    /// Optional signal that the client must resync the surface state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resync_required: Option<bool>,
}

/// JSON-RPC shaped event notification pushed to a subscribed client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceEventNotification {
    /// JSON-RPC version literal (`"2.0"`).
    pub jsonrpc: &'static str,
    /// JSON-RPC method name (`"surface.io.event"`).
    pub method: &'static str,
    /// Event payload.
    pub params: SurfaceEventParams,
}

/// Capabilities negotiated when a client subscribes to a surface.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceSubscriptionCapabilities {
    /// Maximum bytes per event chunk negotiated for the subscription.
    pub max_chunk_bytes: usize,
    /// Maximum events the subscription's queue will hold.
    pub queue_capacity: usize,
    /// Whether the hub will replay missed events on reconnect.
    pub replay: bool,
}

/// Acknowledgement returned to the client after a successful subscription.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceSubscribeAck {
    /// Identifier the client should use to drain events.
    pub subscription_id: u64,
    /// First sequence number the client should expect on drain.
    pub next_seq: u64,
    /// Effective capabilities negotiated for the subscription.
    pub capabilities: SurfaceSubscriptionCapabilities,
}

/// Client-side request to subscribe to one or all surfaces.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct SurfaceSubscribeRequest {
    /// Optional surface id; `None` means "every surface".
    #[serde(default)]
    pub surface_id: Option<String>,
    /// Optional sequence to replay from on connect.
    #[serde(default)]
    pub from_seq: Option<u64>,
    /// Maximum bytes the client accepts per event chunk.
    #[serde(default = "default_chunk_bytes")]
    pub max_chunk_bytes: usize,
    /// Maximum events to hold in the subscription queue.
    #[serde(default = "default_queue_capacity")]
    pub queue_capacity: usize,
}

fn default_chunk_bytes() -> usize {
    MAX_EVENT_CHUNK_BYTES
}

fn default_queue_capacity() -> usize {
    64
}

impl SurfaceSubscribeRequest {
    /// Build a subscription request for one surface with default chunk/queue limits.
    pub fn new(surface_id: impl Into<String>) -> Self {
        Self {
            surface_id: Some(surface_id.into()),
            from_seq: None,
            max_chunk_bytes: MAX_EVENT_CHUNK_BYTES,
            queue_capacity: 64,
        }
    }

        /// Override the desired per-subscription queue capacity.
    pub fn with_queue_capacity(mut self, queue_capacity: usize) -> Self {
        self.queue_capacity = queue_capacity;
        self
    }
}

#[derive(Debug)]
struct SubscriptionState {
    surface_id: Option<String>,
    from_seq: u64,
    max_chunk_bytes: usize,
    queue_capacity: usize,
    queue: VecDeque<SurfaceEventNotification>,
    dropped: u64,
}

#[derive(Debug, Default)]
struct HubState {
    next_subscription: u64,
    next_seq: u64,
    subscriptions: HashMap<u64, SubscriptionState>,
}

/// Thread-safe broker. Publishing never waits on a subscriber and queues are bounded.
#[derive(Debug, Default)]
pub struct SurfaceEventHub {
    state: Mutex<HubState>,
}

impl SurfaceEventHub {
    /// Build a new hub with default per-subscription queue capacity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new subscription, returning the negotiated ack.
    pub fn subscribe(
        &self,
        request: SurfaceSubscribeRequest,
    ) -> Result<SurfaceSubscribeAck, SurfaceEventError> {
        if request.max_chunk_bytes == 0 || request.max_chunk_bytes > MAX_EVENT_CHUNK_BYTES {
            return Err(SurfaceEventError::InvalidChunkBytes);
        }
        if request.queue_capacity == 0 || request.queue_capacity > MAX_EVENT_QUEUE_CAPACITY {
            return Err(SurfaceEventError::InvalidQueueCapacity);
        }
        let mut state = self.state.lock().expect("surface event hub mutex poisoned");
        state.next_subscription = state.next_subscription.saturating_add(1);
        let subscription_id = state.next_subscription;
        let next_seq = state.next_seq.saturating_add(1).max(request.from_seq.unwrap_or(1));
        state.subscriptions.insert(
            subscription_id,
            SubscriptionState {
                surface_id: request.surface_id,
                from_seq: next_seq,
                max_chunk_bytes: request.max_chunk_bytes,
                queue_capacity: request.queue_capacity,
                queue: VecDeque::new(),
                dropped: 0,
            },
        );
        Ok(SurfaceSubscribeAck {
            subscription_id,
            next_seq,
            capabilities: SurfaceSubscriptionCapabilities {
                max_chunk_bytes: request.max_chunk_bytes,
                queue_capacity: request.queue_capacity,
                replay: false,
            },
        })
    }

        /// Drop the subscription, returning `true` if a subscription was removed.
    pub fn unsubscribe(&self, subscription_id: u64) -> Result<bool, SurfaceEventError> {
        let mut state = self.state.lock().expect("surface event hub mutex poisoned");
        Ok(state.subscriptions.remove(&subscription_id).is_some())
    }

        /// Publish raw PTY/process output bytes for `surface_id`.
    pub fn publish_output(
        &self,
        surface_id: &str,
        bytes: &[u8],
        timestamp: Option<String>,
    ) -> Result<u64, SurfaceEventError> {
        self.publish(surface_id, SurfaceEventKind::Output, bytes, timestamp)
    }

    /// Publish a structured event chunked across subscribers; returns the last seq.
    pub fn publish(
        &self,
        surface_id: &str,
        kind: SurfaceEventKind,
        bytes: &[u8],
        timestamp: Option<String>,
    ) -> Result<u64, SurfaceEventError> {
        let mut state = self.state.lock().expect("surface event hub mutex poisoned");
        let mut seq = state.next_seq;
        let chunk_limit = state
            .subscriptions
            .values()
            .filter(|subscription| {
                subscription.surface_id.as_deref().is_none_or(|id| id == surface_id)
            })
            .map(|subscription| subscription.max_chunk_bytes)
            .min()
            .unwrap_or(MAX_EVENT_CHUNK_BYTES);
        let chunks: Vec<&[u8]> =
            if bytes.is_empty() { vec![&[]] } else { bytes.chunks(chunk_limit).collect() };
        for chunk in chunks {
            seq = seq.saturating_add(1);
            state.next_seq = seq;
            let encoded = if chunk.is_empty() {
                None
            } else {
                Some(base64::engine::general_purpose::STANDARD.encode(chunk))
            };
            let subscriptions = state.subscriptions.iter_mut().filter(|(_, subscription)| {
                subscription.surface_id.as_deref().is_none_or(|id| id == surface_id)
                    && seq >= subscription.from_seq
            });
            for (subscription_id, subscription) in subscriptions {
                let event = SurfaceEventNotification {
                    jsonrpc: "2.0",
                    method: "surface.io.event",
                    params: SurfaceEventParams {
                        subscription_id: *subscription_id,
                        surface_id: surface_id.to_owned(),
                        seq,
                        kind,
                        timestamp: timestamp.clone(),
                        event_bytes_base64: encoded.clone(),
                        dropped: None,
                        resync_required: None,
                    },
                };
                enqueue(subscription, event);
            }
        }
        Ok(seq)
    }

        /// Drain up to `max_events` queued notifications for the subscription.
    pub fn drain(
        &self,
        subscription_id: u64,
        max_events: usize,
    ) -> Result<Vec<SurfaceEventNotification>, SurfaceEventError> {
        let mut state = self.state.lock().expect("surface event hub mutex poisoned");
        let Some(subscription) = state.subscriptions.get_mut(&subscription_id) else {
            return Err(SurfaceEventError::UnknownSubscription);
        };
        let count = max_events.min(subscription.queue.len());
        Ok(subscription.queue.drain(..count).collect())
    }
}

fn enqueue(subscription: &mut SubscriptionState, event: SurfaceEventNotification) {
    if subscription.queue.len() < subscription.queue_capacity {
        subscription.queue.push_back(event);
        return;
    }
    subscription.queue.pop_front();
    subscription.dropped = subscription.dropped.saturating_add(1);
    let marker = SurfaceEventNotification {
        jsonrpc: "2.0",
        method: "surface.io.event",
        params: SurfaceEventParams {
            subscription_id: event.params.subscription_id,
            surface_id: event.params.surface_id.clone(),
            seq: event.params.seq.saturating_sub(1),
            kind: SurfaceEventKind::Dropped,
            timestamp: event.params.timestamp.clone(),
            event_bytes_base64: None,
            dropped: Some(subscription.dropped),
            resync_required: Some(true),
        },
    };
    if subscription.queue_capacity == 1 {
        subscription.queue.clear();
        subscription.queue.push_back(marker);
    } else {
        if subscription.queue.len() + 2 > subscription.queue_capacity {
            subscription.queue.pop_front();
        }
        subscription.queue.push_back(marker);
        subscription.queue.push_back(event);
    }
}

/// Errors that the surface event hub may surface to clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceEventError {
    /// `max_chunk_bytes` was zero or exceeded [`MAX_EVENT_CHUNK_BYTES`].
    InvalidChunkBytes,
    /// `queue_capacity` was zero or exceeded [`MAX_EVENT_QUEUE_CAPACITY`].
    InvalidQueueCapacity,
    /// The supplied subscription id did not match any active subscription.
    UnknownSubscription,
}

impl std::fmt::Display for SurfaceEventError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidChunkBytes => "max_chunk_bytes must be between 1 and 65536",
            Self::InvalidQueueCapacity => "queue_capacity must be between 1 and 256",
            Self::UnknownSubscription => "unknown subscription_id",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SurfaceEventError {}
