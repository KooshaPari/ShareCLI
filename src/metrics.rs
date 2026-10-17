use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct Counter {
    v: AtomicU64,
}
impl Counter {
    pub fn new() -> Self {
        Self { v: AtomicU64::new(0) }
    }
    pub fn inc(&self) {
        self.v.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_by(&self, n: u64) {
        self.v.fetch_add(n, Ordering::Relaxed);
    }
    pub fn get(&self) -> u64 {
        self.v.load(Ordering::Relaxed)
    }
}
impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Gauge {
    v: AtomicI64,
}
impl Gauge {
    pub fn new() -> Self {
        Self { v: AtomicI64::new(0) }
    }
    pub fn set(&self, n: i64) {
        self.v.store(n, Ordering::Relaxed);
    }
    pub fn get(&self) -> i64 {
        self.v.load(Ordering::Relaxed)
    }
}
impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MetricsRegistry {
    counters: Mutex<HashMap<String, Arc<Counter>>>,
    gauges: Mutex<HashMap<String, Arc<Gauge>>>,
}
impl MetricsRegistry {
    pub fn new() -> Self {
        Self { counters: Mutex::new(HashMap::new()), gauges: Mutex::new(HashMap::new()) }
    }
    pub fn counter(&self, name: &str) -> Arc<Counter> {
        let mut m = self.counters.lock().expect("MetricsRegistry counters mutex poisoned");
        m.entry(name.to_string()).or_insert_with(|| Arc::new(Counter::new())).clone()
    }
    pub fn gauge(&self, name: &str) -> Arc<Gauge> {
        let mut m = self.gauges.lock().expect("MetricsRegistry gauges mutex poisoned");
        m.entry(name.to_string()).or_insert_with(|| Arc::new(Gauge::new())).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counter_inc() {
        let c = Counter::new();
        c.inc();
        c.inc();
        assert_eq!(c.get(), 2);
    }
    #[test]
    fn counter_inc_by() {
        let c = Counter::new();
        c.inc_by(5);
        assert_eq!(c.get(), 5);
    }
    #[test]
    fn gauge_set() {
        let g = Gauge::new();
        g.set(42);
        assert_eq!(g.get(), 42);
    }
    #[test]
    fn gauge_negative() {
        let g = Gauge::new();
        g.set(-7);
        assert_eq!(g.get(), -7);
    }
    #[test]
    fn registry_same_arc() {
        let r = MetricsRegistry::new();
        let a = r.counter("x");
        let b = r.counter("x");
        assert!(Arc::ptr_eq(&a, &b));
    }
    #[test]
    fn counters_and_gauges_default_to_zero() {
        assert_eq!(Counter::default().get(), 0);
        assert_eq!(Gauge::default().get(), 0);
    }

    #[test]
    fn inc_by_accumulates_on_top_of_inc() {
        let c = Counter::new();
        c.inc();
        c.inc_by(4);
        c.inc();
        assert_eq!(c.get(), 6);
    }

    #[test]
    fn gauge_set_overwrites_the_previous_value() {
        let g = Gauge::new();
        g.set(5);
        g.set(-3);
        assert_eq!(g.get(), -3, "set must replace, not accumulate");
    }

    #[test]
    fn registry_gauges_are_shared_by_name() {
        let r = MetricsRegistry::new();
        let a = r.gauge("lvl");
        a.set(9);
        assert_eq!(r.gauge("lvl").get(), 9);
        assert!(Arc::ptr_eq(&a, &r.gauge("lvl")), "same name must resolve to the same gauge");
    }

    #[test]
    fn two_counters_independent() {
        let r = MetricsRegistry::new();
        r.counter("a").inc_by(3);
        r.counter("b").inc_by(7);
        assert_eq!(r.counter("a").get(), 3);
        assert_eq!(r.counter("b").get(), 7);
    }
}
