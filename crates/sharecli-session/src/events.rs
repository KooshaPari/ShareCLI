// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Bounded, sequenced surface events for live terminal I/O.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use base64::Engine;
use serde::{Deserialize, Serialize};

pub const MAX_EVENT_CHUNK_BYTES: usize = 64 * 1024;
pub const MAX_EVENT_QUEUE_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceEventKind {
    Output,
    Resize,
    Exit,
    Title,
    Cwd,
    Dropped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceEventParams {
    pub subscription_id: u64,
    pub surface_id: String,
    pub seq: u64,
    pub kind: SurfaceEventKind,
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_bytes_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropped: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resync_required: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceEventNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: SurfaceEventParams,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceSubscriptionCapabilities {
    pub max_chunk_bytes: usize,
    pub queue_capacity: usize,
    pub replay: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceSubscribeAck {
    pub subscription_id: u64,
    pub next_seq: u64,
    pub capabilities: SurfaceSubscriptionCapabilities,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct SurfaceSubscribeRequest {
    #[serde(default)]
    pub surface_id: Option<String>,
    #[serde(default)]
    pub from_seq: Option<u64>,
    #[serde(default = "default_chunk_bytes")]
    pub max_chunk_bytes: usize,
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
    pub fn new(surface_id: impl Into<String>) -> Self {
        Self {
            surface_id: Some(surface_id.into()),
            from_seq: None,
            max_chunk_bytes: MAX_EVENT_CHUNK_BYTES,
            queue_capacity: 64,
        }
    }

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
    pub fn new() -> Self {
        Self::default()
    }

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

    pub fn unsubscribe(&self, subscription_id: u64) -> Result<bool, SurfaceEventError> {
        let mut state = self.state.lock().expect("surface event hub mutex poisoned");
        Ok(state.subscriptions.remove(&subscription_id).is_some())
    }

    pub fn publish_output(
        &self,
        surface_id: &str,
        bytes: &[u8],
        timestamp: Option<String>,
    ) -> Result<u64, SurfaceEventError> {
        self.publish(surface_id, SurfaceEventKind::Output, bytes, timestamp)
    }

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceEventError {
    InvalidChunkBytes,
    InvalidQueueCapacity,
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
