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
        let resources = Self {
            cpu: vector.cpu.ok_or_else(|| AdmissionRejection::Unknown("cpu".into()))?,
            memory_bytes: vector.memory_bytes.ok_or_else(|| AdmissionRejection::Unknown("memory".into()))?,
            gpu_count: vector.gpu_count.ok_or_else(|| AdmissionRejection::Unknown("gpu_count".into()))?,
            vram_bytes: vector.vram_bytes.ok_or_else(|| AdmissionRejection::Unknown("vram".into()))?,
            disk_bytes: vector.disk_bytes.ok_or_else(|| AdmissionRejection::Unknown("disk".into()))?,
            io_weight: vector.io_weight.ok_or_else(|| AdmissionRejection::Unknown("io_weight".into()))?,
        };
        for (name, value) in [("cpu", resources.cpu), ("io_weight", resources.io_weight)] {
            if !value.is_finite() || value < 0.0 {
                return Err(AdmissionRejection::Invalid(name.into()));
            }
        }
        Ok(resources)
    }

    // Saturation is unsafe here: MAX + 1 must not appear to fit a MAX budget.
    fn checked_add(self, rhs: Self) -> Option<Self> {
        let cpu = self.cpu + rhs.cpu;
        let io_weight = self.io_weight + rhs.io_weight;
        if !cpu.is_finite()
            || !io_weight.is_finite()
            || (rhs.cpu > 0.0 && cpu <= self.cpu)
            || (rhs.io_weight > 0.0 && io_weight <= self.io_weight)
        {
            return None;
        }
        Some(Self {
            cpu,
            memory_bytes: self.memory_bytes.checked_add(rhs.memory_bytes)?,
            gpu_count: self.gpu_count.checked_add(rhs.gpu_count)?,
            vram_bytes: self.vram_bytes.checked_add(rhs.vram_bytes)?,
            disk_bytes: self.disk_bytes.checked_add(rhs.disk_bytes)?,
            io_weight,
        })
    }

    fn exceeds(self, cap: Self) -> Option<&'static str> {
        if self.cpu > cap.cpu { Some("cpu") }
        else if self.memory_bytes > cap.memory_bytes { Some("memory") }
        else if self.gpu_count > cap.gpu_count { Some("gpu_count") }
        else if self.vram_bytes > cap.vram_bytes { Some("vram") }
        else if self.disk_bytes > cap.disk_bytes { Some("disk") }
        else if self.io_weight > cap.io_weight { Some("io_weight") }
        else { None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRejection {
    Unknown(String),
    Invalid(String),
    MissingCapability(String),
    DoesNotFit(String),
    WouldExceed(String),
}

impl fmt::Display for AdmissionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(resource) => write!(f, "unknown resource demand/capacity: {resource}"),
            Self::Invalid(resource) => write!(f, "invalid resource demand/capacity: {resource}"),
            Self::MissingCapability(capability) => write!(f, "missing required capability: {capability}"),
            Self::DoesNotFit(resource) => write!(f, "work item cannot fit envelope: {resource}"),
            Self::WouldExceed(resource) => write!(f, "admission would exceed active envelope: {resource}"),
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
    pub fn envelope_id(&self) -> &str { &self.envelope_id }
    pub fn active_count(&self) -> usize {
        self.state.lock().expect("resource admission mutex poisoned").active.len()
    }
    pub fn try_acquire(
        &self,
        work_item_id: &str,
        demand: &ResourceVector,
    ) -> Result<ResourceAdmissionLease, AdmissionRejection> {
        if work_item_id.trim().is_empty() {
            return Err(AdmissionRejection::Invalid("work_item_id".into()));
        }
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
        let next = state.used.checked_add(demand_known)
            .ok_or_else(|| AdmissionRejection::WouldExceed("resource arithmetic overflow".into()))?;
        if let Some(resource) = next.exceeds(self.capacity) {
            return Err(AdmissionRejection::WouldExceed(resource.into()));
        }
        let ordinal = self.sequence.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| AdmissionRejection::Invalid("reservation identity exhausted".into()))?;
        let reservation_id = format!("reservation-{ordinal}-{work_item_id}");
        state.used = next;
        state.active.insert(reservation_id.clone(), demand_known);
        drop(state);
        Ok(ResourceAdmissionLease {
            reservation_id,
            work_item_id: work_item_id.to_string(),
            state: Arc::clone(&self.state),
            released: false,
        })
    }
}

#[derive(Debug)]
pub struct ResourceAdmissionLease {
    reservation_id: String,
    work_item_id: String,
    state: Arc<Mutex<AdmissionState>>,
    released: bool,
}
impl ResourceAdmissionLease {
    pub fn reservation_id(&self) -> &str { &self.reservation_id }
    pub fn work_item_id(&self) -> &str { &self.work_item_id }
    pub fn release(mut self) { self.release_inner(); }
    fn release_inner(&mut self) {
        if self.released { return; }
        let mut state = self.state.lock().expect("resource admission mutex poisoned");
        if state.active.remove(&self.reservation_id).is_some() {
            state.used = state
                .active
                .values()
                .copied()
                .try_fold(KnownResources::default(), |used, demand| used.checked_add(demand))
                .expect("active reservations must remain representable");
        }
        self.released = true;
    }
}
impl Drop for ResourceAdmissionLease {
    fn drop(&mut self) { self.release_inner(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vector(cpu: f64, memory: u64, gpu: u32, vram: u64, caps: &[&str]) -> ResourceVector {
        ResourceVector {
            cpu: Some(cpu), memory_bytes: Some(memory), gpu_count: Some(gpu),
            vram_bytes: Some(vram), disk_bytes: Some(0), io_weight: Some(0.0),
            capabilities: caps.iter().map(|value| (*value).to_string()).collect(),
        }
    }
    fn envelope() -> ResourceEnvelope {
        ResourceEnvelope {
            id: "host".into(), observed_at_unix_ms: 1, source: "fixture".into(),
            capacity: vector(4.0, 8_000, 1, 24_000, &["cuda"]),
        }
    }
    #[test]
    fn lease_identity_is_not_pid_and_drop_returns_capacity() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        let lease = pool.try_acquire("compile", &vector(2.0, 4_000, 0, 0, &[])).unwrap();
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
        let _a = pool.try_acquire("a", &vector(2.0, 4_000, 0, 0, &[])).unwrap();
        let _b = pool.try_acquire("b", &vector(2.0, 4_000, 1, 20_000, &["cuda"])).unwrap();
        assert!(matches!(pool.try_acquire("c", &vector(1.0, 1_000, 0, 0, &[])), Err(AdmissionRejection::WouldExceed(_))));
    }
    #[test]
    fn unknown_demand_and_missing_capability_fail_closed() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        let mut unknown = vector(1.0, 1_000, 0, 0, &[]);
        unknown.memory_bytes = None;
        assert!(matches!(pool.try_acquire("unknown", &unknown), Err(AdmissionRejection::Unknown(_))));
        assert!(matches!(pool.try_acquire("rocm", &vector(1.0, 1_000, 1, 4_000, &["rocm"])), Err(AdmissionRejection::MissingCapability(_))));
    }
    #[test]
    fn nonfinite_and_negative_demand_or_capacity_cannot_create_credit() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            for cpu_field in [true, false] {
                let mut demand = vector(0.0, 0, 0, 0, &[]);
                if cpu_field { demand.cpu = Some(value); } else { demand.io_weight = Some(value); }
                assert!(matches!(pool.try_acquire("invalid", &demand), Err(AdmissionRejection::Invalid(_))));
                let mut env = envelope();
                env.capacity = demand;
                assert!(matches!(ResourceAdmissionPool::new(&env), Err(AdmissionRejection::Invalid(_))));
            }
        }
        assert_eq!(pool.active_count(), 0);
    }
    #[test]
    fn integer_overflow_is_not_saturated_into_a_valid_budget() {
        let mut env = envelope();
        env.capacity.memory_bytes = Some(u64::MAX);
        let pool = ResourceAdmissionPool::new(&env).unwrap();
        let first = pool.try_acquire("full", &vector(0.0, u64::MAX, 0, 0, &[])).unwrap();
        assert!(matches!(pool.try_acquire("extra", &vector(0.0, 1, 0, 0, &[])), Err(AdmissionRejection::WouldExceed(_))));
        assert_eq!(pool.active_count(), 1);
        drop(first);
        assert!(pool.try_acquire("returned", &vector(0.0, u64::MAX, 0, 0, &[])).is_ok());
    }
    #[test]
    fn positive_float_demand_that_cannot_be_represented_is_rejected() {
        let mut env = envelope();
        env.capacity.cpu = Some(1.0e20);
        let pool = ResourceAdmissionPool::new(&env).unwrap();
        let _large = pool.try_acquire("large", &vector(1.0e20, 0, 0, 0, &[])).unwrap();
        assert!(matches!(
            pool.try_acquire("tiny", &vector(1.0, 0, 0, 0, &[])),
            Err(AdmissionRejection::WouldExceed(_))
        ));
    }

    #[test]
    fn release_recomputes_usage_from_surviving_leases() {
        let mut env = envelope();
        env.capacity.cpu = Some(10.0);
        let pool = ResourceAdmissionPool::new(&env).unwrap();
        let first = pool.try_acquire("first", &vector(6.0, 0, 0, 0, &[])).unwrap();
        let second = pool.try_acquire("second", &vector(4.0, 0, 0, 0, &[])).unwrap();
        drop(first);
        assert_eq!(pool.active_count(), 1);
        assert!(pool.try_acquire("replacement", &vector(6.0, 0, 0, 0, &[])).is_ok());
        drop(second);
    }

    #[test]
    fn blank_work_identity_is_rejected_before_reservation() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        assert!(matches!(
            pool.try_acquire("   ", &vector(1.0, 1_000, 0, 0, &[])),
            Err(AdmissionRejection::Invalid(_))
        ));
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn simultaneous_callers_share_one_atomic_budget() {
        let pool = ResourceAdmissionPool::new(&envelope()).unwrap();
        let start = Arc::new(std::sync::Barrier::new(32));
        let acquired = Arc::new(std::sync::Barrier::new(32));
        let threads: Vec<_> = (0..32).map(|i| {
            let pool = pool.clone();
            let start = Arc::clone(&start);
            let acquired = Arc::clone(&acquired);
            std::thread::spawn(move || {
                start.wait();
                let result = pool.try_acquire(&format!("work-{i}"), &vector(1.0, 2_000, 0, 0, &[]));
                let ok = result.is_ok();
                // All successful reservations remain held until all contenders tried.
                acquired.wait();
                drop(result);
                ok
            })
        }).collect();
        let successes = threads.into_iter().map(|t| t.join().expect("admission worker panicked")).filter(|ok| *ok).count();
        assert_eq!(successes, 4);
        assert_eq!(pool.active_count(), 0);
    }
}
