use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sharecli_ipc::scheduling::{ResourceEnvelope, ResourceVector};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct KnownResources {
    cpu: f64,
    memory_bytes: u64,
    gpu_count: u32,
    vram_bytes: u64,
    disk_bytes: u64,
    io_weight: f64,
}

impl KnownResources {
    fn from_vector(vector: &ResourceVector) -> Result<Self, AdmissionRejection> {
        Ok(Self {
            cpu: vector.cpu.ok_or_else(|| AdmissionRejection::Unknown("cpu".into()))?,
            memory_bytes: vector
                .memory_bytes
                .ok_or_else(|| AdmissionRejection::Unknown("memory".into()))?,
            gpu_count: vector
                .gpu_count
                .ok_or_else(|| AdmissionRejection::Unknown("gpu_count".into()))?,
            vram_bytes: vector
                .vram_bytes
                .ok_or_else(|| AdmissionRejection::Unknown("vram".into()))?,
            disk_bytes: vector
                .disk_bytes
                .ok_or_else(|| AdmissionRejection::Unknown("disk".into()))?,
            io_weight: vector
                .io_weight
                .ok_or_else(|| AdmissionRejection::Unknown("io_weight".into()))?,
        })
    }

    fn add(self, rhs: Self) -> Self {
        Self {
            cpu: self.cpu + rhs.cpu,
            memory_bytes: self.memory_bytes.saturating_add(rhs.memory_bytes),
            gpu_count: self.gpu_count.saturating_add(rhs.gpu_count),
            vram_bytes: self.vram_bytes.saturating_add(rhs.vram_bytes),
            disk_bytes: self.disk_bytes.saturating_add(rhs.disk_bytes),
            io_weight: self.io_weight + rhs.io_weight,
        }
    }

    fn sub(self, rhs: Self) -> Self {
        Self {
            cpu: (self.cpu - rhs.cpu).max(0.0),
            memory_bytes: self.memory_bytes.saturating_sub(rhs.memory_bytes),
            gpu_count: self.gpu_count.saturating_sub(rhs.gpu_count),
            vram_bytes: self.vram_bytes.saturating_sub(rhs.vram_bytes),
            disk_bytes: self.disk_bytes.saturating_sub(rhs.disk_bytes),
            io_weight: (self.io_weight - rhs.io_weight).max(0.0),
        }
    }

    fn exceeds(self, cap: Self) -> Option<&'static str> {
        if self.cpu > cap.cpu {
            Some("cpu")
        } else if self.memory_bytes > cap.memory_bytes {
            Some("memory")
        } else if self.gpu_count > cap.gpu_count {
            Some("gpu_count")
        } else if self.vram_bytes > cap.vram_bytes {
            Some("vram")
        } else if self.disk_bytes > cap.disk_bytes {
            Some("disk")
        } else if self.io_weight > cap.io_weight {
            Some("io_weight")
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRejection {
    Unknown(String),
    MissingCapability(String),
    DoesNotFit(String),
    WouldExceed(String),
}

impl fmt::Display for AdmissionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(resource) => write!(f, "unknown resource demand/capacity: {resource}"),
            Self::MissingCapability(capability) => {
                write!(f, "missing required capability: {capability}")
            }
            Self::DoesNotFit(resource) => write!(f, "work item cannot fit envelope: {resource}"),
            Self::WouldExceed(resource) => {
                write!(f, "admission would exceed active envelope: {resource}")
            }
        }
    }
}

impl std::error::Error for AdmissionRejection {}

#[derive(Debug)]
struct AdmissionState {
    used: KnownResources,
    active: BTreeMap<String, KnownResources>,
}

#[derive(Debug, Clone)]
pub struct ResourceAdmissionPool {
    envelope_id: String,
    capacity: KnownResources,
    capabilities: Vec<String>,
    state: Arc<Mutex<AdmissionState>>,
    sequence: Arc<AtomicU64>,
}

impl ResourceAdmissionPool {
    pub fn new(envelope: &ResourceEnvelope) -> Result<Self, AdmissionRejection> {
        Ok(Self {
            envelope_id: envelope.id.clone(),
            capacity: KnownResources::from_vector(&envelope.capacity)?,
            capabilities: envelope.capacity.capabilities.clone(),
            state: Arc::new(Mutex::new(AdmissionState {
                used: KnownResources::default(),
                active: BTreeMap::new(),
            })),
            sequence: Arc::new(AtomicU64::new(1)),
        })
    }

    pub fn envelope_id(&self) -> &str {
        &self.envelope_id
    }

    pub fn active_count(&self) -> usize {
        self.state.lock().expect("resource admission mutex poisoned").active.len()
    }

    pub fn try_acquire(
        &self,
        work_item_id: &str,
        demand: &ResourceVector,
    ) -> Result<ResourceAdmissionLease, AdmissionRejection> {
        let demand_known = KnownResources::from_vector(demand)?;

        for capability in &demand.capabilities {
            if !self.capabilities.iter().any(|value| value == capability) {
                return Err(AdmissionRejection::MissingCapability(capability.clone()));
            }
        }

        if let Some(resource) = demand_known.exceeds(self.capacity) {
            return Err(AdmissionRejection::DoesNotFit(resource.into()));
        }

        let mut state = self.state.lock().expect("resource admission mutex poisoned");
        let next = state.used.add(demand_known);
        if let Some(resource) = next.exceeds(self.capacity) {
            return Err(AdmissionRejection::WouldExceed(resource.into()));
        }

        let ordinal = self.sequence.fetch_add(1, Ordering::Relaxed);
        let reservation_id = format!("reservation-{ordinal}-{work_item_id}");
        state.used = next;
        state.active.insert(reservation_id.clone(), demand_known);
        drop(state);

        Ok(ResourceAdmissionLease {
            reservation_id,
            work_item_id: work_item_id.to_string(),
            demand: demand_known,
            state: Arc::clone(&self.state),
            released: false,
        })
    }
}

#[derive(Debug)]
pub struct ResourceAdmissionLease {
    reservation_id: String,
    work_item_id: String,
    demand: KnownResources,
    state: Arc<Mutex<AdmissionState>>,
    released: bool,
}

impl ResourceAdmissionLease {
    pub fn reservation_id(&self) -> &str {
        &self.reservation_id
    }

    pub fn work_item_id(&self) -> &str {
        &self.work_item_id
    }

    pub fn release(mut self) {
        self.release_inner();
    }

    fn release_inner(&mut self) {
        if self.released {
            return;
        }
        let mut state = self.state.lock().expect("resource admission mutex poisoned");
        if state.active.remove(&self.reservation_id).is_some() {
            state.used = state.used.sub(self.demand);
        }
        self.released = true;
    }
}

impl Drop for ResourceAdmissionLease {
    fn drop(&mut self) {
        self.release_inner();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(cpu: f64, memory: u64, gpu: u32, vram: u64, caps: &[&str]) -> ResourceVector {
        ResourceVector {
            cpu: Some(cpu),
            memory_bytes: Some(memory),
            gpu_count: Some(gpu),
            vram_bytes: Some(vram),
            disk_bytes: Some(0),
            io_weight: Some(0.0),
            capabilities: caps.iter().map(|value| (*value).to_string()).collect(),
        }
    }

    fn envelope() -> ResourceEnvelope {
        ResourceEnvelope {
            id: "host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: vector(4.0, 8_000, 1, 24_000, &["cuda"]),
        }
    }

    #[test]
    fn lease_identity_is_not_pid_and_drop_returns_capacity() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        let lease = pool
            .try_acquire("compile", &vector(2.0, 4_000, 0, 0, &[]))
            .unwrap();
        assert!(lease.reservation_id().starts_with("reservation-"));
        assert!(lease.reservation_id().contains("compile"));
        assert_eq!(pool.active_count(), 1);
        drop(lease);
        assert_eq!(pool.active_count(), 0);
        assert!(pool.try_acquire("next", &vector(4.0, 8_000, 1, 24_000, &["cuda"])).is_ok());
    }

    #[test]
    fn aggregate_reservations_cannot_overbook_envelope() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        let _a = pool
            .try_acquire("a", &vector(2.0, 4_000, 0, 0, &[]))
            .unwrap();
        let _b = pool
            .try_acquire("b", &vector(2.0, 4_000, 1, 20_000, &["cuda"]))
            .unwrap();
        assert!(matches!(
            pool.try_acquire("c", &vector(1.0, 1_000, 0, 0, &[])),
            Err(AdmissionRejection::WouldExceed(_))
        ));
    }

    #[test]
    fn unknown_demand_and_missing_capability_fail_closed() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        let mut unknown = vector(1.0, 1_000, 0, 0, &[]);
        unknown.memory_bytes = None;
        assert!(matches!(
            pool.try_acquire("unknown", &unknown),
            Err(AdmissionRejection::Unknown(_))
        ));
        assert!(matches!(
            pool.try_acquire("rocm", &vector(1.0, 1_000, 1, 4_000, &["rocm"])),
            Err(AdmissionRejection::MissingCapability(_))
        ));
    }
}
