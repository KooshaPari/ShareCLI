//! N-slot priority queue for mutating / nocache command paths.
//!
//! Origin: Feb `core.sh` `harness::strategy::queue` — concurrency limiter with
//! priority levels. Higher priority (lower numeric rank) acquires slots first.
//!
//! Layout under `root/`:
//! ```text
//! <lane>.slot0.lock … <lane>.slot{N-1}.lock   — advisory flock sentinels
//! <lane>.waiting/<ticket>                    — waiters (priority order)
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use fs2::FileExt;
use sharecli_fleet::{record_slot_acquire, record_slot_timeout, record_slot_wait};

/// Monotonic suffix so concurrent waiters in the same process get distinct tickets.
static WAITER_TICKET_SEQ: AtomicU64 = AtomicU64::new(0);

/// Queue priority levels (Feb harness `HARNESS_PRIORITY_LEVELS`).
///
/// Lower discriminant = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum QueuePriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
    Background = 4,
}

impl QueuePriority {
    /// Parse Feb-style priority names (`critical`, `high`, `normal`, `low`, `background`).
    pub fn parse(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "critical" => Self::Critical,
            "high" => Self::High,
            "low" => Self::Low,
            "background" => Self::Background,
            _ => Self::Normal,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Operator env override for Hypervisor nocache [`SlotQueue`] priority (FR-008 AC-008.15).
pub const QUEUE_PRIORITY_ENV: &str = "SHARECLI_QUEUE_PRIORITY";

/// Resolve queue priority from the operator surface.
///
/// Precedence: non-empty [`QUEUE_PRIORITY_ENV`] → optional rules.conf `priority=`
/// → [`QueuePriority::Normal`].
pub fn resolve_operator_queue_priority(rule_priority: Option<&str>) -> QueuePriority {
    if let Ok(raw) = std::env::var(QUEUE_PRIORITY_ENV) {
        if !raw.trim().is_empty() {
            return QueuePriority::parse(&raw);
        }
    }
    rule_priority.map(QueuePriority::parse).unwrap_or(QueuePriority::Normal)
}

/// Filesystem-backed N-slot concurrency limiter for a named command lane.
///
/// Callers that must not coalesce (mutating / `nocache_args` paths) acquire a
/// slot via [`SlotQueue::with_slot`], run the work, then release.
pub struct SlotQueue {
    root: PathBuf,
    max_concurrent: usize,
    /// Max time to wait for a free slot before returning an error (loud fail).
    timeout: Duration,
    /// Poll interval while waiting for a slot.
    poll: Duration,
}

impl SlotQueue {
    /// Default max wait (30s) — matches Feb `HARNESS_LOCK_TIMEOUT`.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
    /// Default poll interval (100ms) — matches Feb queue wait loop.
    pub const DEFAULT_POLL: Duration = Duration::from_millis(100);

    /// Create a queue rooted at `root` with `max_concurrent` parallel slots.
    pub fn new(root: impl Into<PathBuf>, max_concurrent: usize) -> Self {
        Self::with_options(root, max_concurrent.max(1), Self::DEFAULT_TIMEOUT, Self::DEFAULT_POLL)
    }

    /// Create a queue with explicit timeout / poll settings (tests).
    pub fn with_options(
        root: impl Into<PathBuf>,
        max_concurrent: usize,
        timeout: Duration,
        poll: Duration,
    ) -> Self {
        Self { root: root.into(), max_concurrent: max_concurrent.max(1), timeout, poll }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create queue root {}", self.root.display()))
    }

    fn slot_path(&self, lane: &str, slot: usize) -> PathBuf {
        self.root.join(format!("{lane}.slot{slot}.lock"))
    }

    fn waiting_dir(&self, lane: &str) -> PathBuf {
        self.root.join(format!("{lane}.waiting"))
    }

    /// Register a waiter ticket so higher-priority lanes can be preferred.
    fn enqueue_waiter(&self, lane: &str, priority: QueuePriority) -> Result<(PathBuf, String)> {
        let dir = self.waiting_dir(lane);
        fs::create_dir_all(&dir)
            .with_context(|| format!("create waiting dir {}", dir.display()))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let seq = WAITER_TICKET_SEQ.fetch_add(1, Ordering::Relaxed);
        // Priority prefix avoids read races when scanning the waiting dir (AC-008.14).
        let ticket = format!("{:02}.{}.{}.{}", priority.as_u8(), now, std::process::id(), seq);
        let path = dir.join(&ticket);
        let tmp = dir.join(format!(".{ticket}.tmp"));
        fs::write(&tmp, format!("{}\n", priority.as_u8()))
            .with_context(|| format!("write waiter ticket {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("publish waiter ticket {}", path.display()))?;
        Ok((path, ticket))
    }

    fn ticket_priority(ticket: &str) -> u8 {
        ticket
            .split('.')
            .next()
            .and_then(|head| head.parse::<u8>().ok())
            .unwrap_or(QueuePriority::Normal.as_u8())
    }

    /// Effective priority after rank aging. Phase-0.4 (Lane-7 BLOCKER, tiger):
    /// an orphan Critical waiter that never dequeues (holder wedged, ticket
    /// file leaked, parent killed) used to starve every Normal waiter
    /// forever, because `is_my_turn` compares raw ranks only. After 1 s of
    /// waiting a Critical ticket decays to High, after 2 s to Normal, etc.
    /// `AGING_STEP` is the time-in-queue per one priority step downward.
    const AGING_STEP_MS: u128 = 1_000;

    fn effective_rank(_ticket: &str, base_priority: QueuePriority, waited: Duration) -> u8 {
        let decay_steps = (waited.as_millis() / Self::AGING_STEP_MS) as u8;
        base_priority.as_u8().saturating_add(decay_steps).min(u8::MAX)
    }

    /// True when this ticket is next among equal-or-highest-priority waiters (FIFO by ticket name).
    ///
    /// Aging (Phase-0.4 / Lane-7 BLOCKER): my own rank is decayed by elapsed
    /// wait time so that a Critical ticket that has been blocked > N seconds
    /// yields to a fresh Normal waiter — an orphan Critical holder no longer
    /// starves the lane. Peer tickets are compared on their **base** rank
    /// only; FIFO within same priority is preserved by ticket-name order.
    /// This asymmetry is intentional: the bug we are fixing is starvation by
    /// a single orphan ticket, not intra-priority reordering of my peers.
    fn is_my_turn(
        &self,
        lane: &str,
        my_priority: QueuePriority,
        my_ticket: &str,
        my_enqueued_at: Instant,
    ) -> Result<bool> {
        let dir = self.waiting_dir(lane);
        let entries = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(e) => return Err(e).with_context(|| format!("read waiting dir {}", dir.display())),
        };

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // My age-adjusted rank: a Critical ticket I have been holding
        // expires from priority over time.
        let my_effective =
            Self::effective_rank(my_ticket, my_priority, my_enqueued_at.elapsed());

        let mut best_effective = u8::MAX;
        let mut tickets_at_best: Vec<String> = Vec::new();

        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let ticket = entry.file_name().to_string_lossy().into_owned();
            if ticket.starts_with('.') {
                continue;
            }
            let base_rank = Self::ticket_priority(&ticket);

            // Decode the enqueue-second embedded in the ticket name
            // (segment 1 of `<rank>.<secs>.<pid>.<seq>`) and age the peer
            // by `now_secs - ticket_secs`. Cross-process peers share the
            // wall-clock hint; same-process tickets correlate via pid.
            let ticket_secs = ticket
                .split('.')
                .nth(1)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(now_secs);
            let peer_waited_ms: u64 = if now_secs >= ticket_secs {
                let diff = (now_secs - ticket_secs) as u128;
                let ms = diff.saturating_mul(1_000);
                if ms > u128::from(u64::MAX) {
                    u64::MAX
                } else {
                    ms as u64
                }
            } else {
                0
            };
            let base_priority = match base_rank {
                0 => QueuePriority::Critical,
                1 => QueuePriority::High,
                2 => QueuePriority::Normal,
                3 => QueuePriority::Low,
                _ => QueuePriority::Background,
            };
            let peer_effective = Self::effective_rank(
                &ticket,
                base_priority,
                Duration::from_millis(peer_waited_ms),
            );

            if peer_effective < best_effective {
                best_effective = peer_effective;
                tickets_at_best.clear();
                tickets_at_best.push(ticket);
            } else if peer_effective == best_effective {
                tickets_at_best.push(ticket);
            }
        }

        if best_effective == u8::MAX {
            return Ok(true);
        }

        // Compare my age-adjusted rank against the cohort's best
        // age-adjusted rank. Strict-better ME (lower numeric rank) → I win.
        // Strict-better peer → they win. Tie → FIFO-by-ticket-name decides.
        if my_effective > best_effective {
            return Ok(false);
        }
        if my_effective < best_effective {
            return Ok(true);
        }

        let winner = tickets_at_best
            .iter()
            .min()
            .map(String::as_str)
            .unwrap_or(my_ticket);
        Ok(winner == my_ticket)
    }

    /// Acquire a free slot for `lane`, run `f`, then release the slot.
    ///
    /// Mutating / nocache paths MUST use this instead of [`super::CoalesceCache`].
    pub fn with_slot<T>(
        &self,
        lane: &str,
        priority: QueuePriority,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        self.ensure_root()?;
        let (ticket_path, ticket) = self.enqueue_waiter(lane, priority)?;
        let enqueued_at = Instant::now();
        // RAII guard: removes the ticket file on early return (timeout, error,
        // panic) and on the normal success path. Phase-0.4 (Lane-7 BLOCKER):
        // the previous `let _ = fs::remove_file(&ticket_path);` only fired on
        // the success branch, so a wedged holder or panicked closure leaked
        // its ticket and starved every subsequent waiter in the lane.
        let mut guard = WaiterTicketGuard::new(ticket_path.clone());
        let deadline = Instant::now() + self.timeout;

        let result = (|| -> Result<T> {
            loop {
                if Instant::now() >= deadline {
                    record_slot_timeout();
                    let err = anyhow::anyhow!(
                        "queue timeout: no free slot for lane `{lane}` within {}s \
                         (max_concurrent={})",
                        self.timeout.as_secs(),
                        self.max_concurrent
                    );
                    return Err(err);
                }

                // Prefer higher-priority waiters; FIFO by ticket among ties (Feb yield-to-higher-priority).
                if !self.is_my_turn(lane, priority, &ticket, enqueued_at)? {
                    record_slot_wait();
                    thread::sleep(self.poll);
                    continue;
                }

                for slot in 0..self.max_concurrent {
                    let lock_path = self.slot_path(lane, slot);
                    let lock_file = fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(false)
                        .open(&lock_path)
                        .with_context(|| format!("open slot lock {}", lock_path.display()))?;

                    match lock_file.try_lock_exclusive() {
                        Ok(()) => {
                            if !self.is_my_turn(lane, priority, &ticket, enqueued_at)? {
                                drop(lock_file);
                                record_slot_wait();
                                thread::sleep(self.poll);
                                continue;
                            }
                            // Drop waiter before running so peers can reorder.
                            // RAII guard cleans up automatically if either step panics.
                            let _ = fs::remove_file(&ticket_path);
                            guard.release();
                            record_slot_acquire();
                            let value = f()?;
                            // Lock releases on drop of lock_file.
                            drop(lock_file);
                            return Ok(value);
                        }
                        Err(_) => continue,
                    }
                }

                record_slot_wait();
                thread::sleep(self.poll);
            }
        })();

        // `guard` drops here. If we acquired a slot successfully, the
        // happy-path branch already removed the ticket and called
        // `guard.release()`, so Drop is a no-op. If we exited via timeout,
        // panic, or any other error, Drop unlinks the leftover ticket.
        drop(guard);
        result
    }
}

/// RAII guard that removes a waiter ticket file on drop unless `release()`
/// was called. Phase-0.4 (Lane-7 BLOCKER, tiger): previous code only
/// `remove_file`d the ticket on the happy path; a wedged holder, panicked
/// closure, or early return leaked the ticket and starved the lane.
struct WaiterTicketGuard {
    path: PathBuf,
    cleaned: bool,
}

impl WaiterTicketGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, cleaned: false }
    }

    /// Mark the ticket as removed. Idempotent. After calling this, the
    /// guard's `Drop` is a no-op. The caller is responsible for actually
    /// unlinking the file (typically via [`remove_file`][fs::remove_file],
    /// which is what the slot-acquire branch does).
    fn release(&mut self) {
        self.cleaned = true;
    }
}

impl Drop for WaiterTicketGuard {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Alias documenting Feb `priority_queue` strategy (same as [`SlotQueue`]).
pub type PriorityQueue = SlotQueue;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn priority_parse() {
        assert_eq!(QueuePriority::parse("critical"), QueuePriority::Critical);
        assert_eq!(QueuePriority::parse("HIGH"), QueuePriority::High);
        assert_eq!(QueuePriority::parse("unknown"), QueuePriority::Normal);
    }

    #[test]
    fn with_slot_serializes_max_one() {
        let dir = TempDir::new().unwrap();
        let active = Arc::new(AtomicU32::new(0));
        let peak = Arc::new(AtomicU32::new(0));

        let mut handles = vec![];
        for _ in 0..4 {
            let q_root = dir.path().to_path_buf();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            handles.push(thread::spawn(move || {
                let q = SlotQueue::with_options(
                    q_root,
                    1,
                    Duration::from_secs(5),
                    Duration::from_millis(20),
                );
                q.with_slot("ruff", QueuePriority::Normal, || {
                    let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(n, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(40));
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
                .unwrap();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1, "max_concurrent=1 MUST serialize");
    }

    #[test]
    fn with_slot_allows_two() {
        let dir = TempDir::new().unwrap();
        let peak = Arc::new(AtomicU32::new(0));
        let active = Arc::new(AtomicU32::new(0));
        let mut handles = vec![];
        for _ in 0..4 {
            let q_root = dir.path().to_path_buf();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            handles.push(thread::spawn(move || {
                let q = SlotQueue::with_options(
                    q_root,
                    2,
                    Duration::from_secs(5),
                    Duration::from_millis(10),
                );
                q.with_slot("pytest", QueuePriority::Low, || {
                    let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(n, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(50));
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
                .unwrap();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) >= 2, "max_concurrent=2 MUST allow parallel slots");
        assert!(peak.load(Ordering::SeqCst) <= 2);
    }

    /// FR-008 / AC-008.14 — Critical waiters MUST acquire before Normal under contention.
    #[test]
    fn critical_dequeues_before_normal_under_contention() {
        let dir = TempDir::new().unwrap();
        let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let holder_ready = Arc::new(AtomicBool::new(false));
        let release_holder = Arc::new(AtomicBool::new(false));

        let holder_order = Arc::clone(&order);
        let holder_ready_flag = Arc::clone(&holder_ready);
        let holder_release = Arc::clone(&release_holder);
        let holder_root = dir.path().to_path_buf();
        let holder = thread::spawn(move || {
            let q = SlotQueue::with_options(
                holder_root,
                1,
                Duration::from_secs(5),
                Duration::from_millis(5),
            );
            q.with_slot("lane", QueuePriority::Normal, || {
                holder_order.lock().unwrap().push("holder_start");
                holder_ready_flag.store(true, Ordering::SeqCst);
                while !holder_release.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(5));
                }
                holder_order.lock().unwrap().push("holder_end");
                Ok(())
            })
            .unwrap();
        });

        while !holder_ready.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(2));
        }

        let critical_order = Arc::clone(&order);
        let critical_root = dir.path().to_path_buf();
        let critical = thread::spawn(move || {
            let q = SlotQueue::with_options(
                critical_root,
                1,
                Duration::from_secs(5),
                Duration::from_millis(5),
            );
            q.with_slot("lane", QueuePriority::Critical, || {
                critical_order.lock().unwrap().push("critical");
                Ok(())
            })
            .unwrap();
        });

        let late_normal_order = Arc::clone(&order);
        let late_normal_root = dir.path().to_path_buf();
        let late_normal = thread::spawn(move || {
            let q = SlotQueue::with_options(
                late_normal_root,
                1,
                Duration::from_secs(5),
                Duration::from_millis(5),
            );
            q.with_slot("lane", QueuePriority::Normal, || {
                late_normal_order.lock().unwrap().push("normal_late");
                Ok(())
            })
            .unwrap();
        });

        let waiting = dir.path().join("lane.waiting");
        let wait_deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < wait_deadline {
            let count = fs::read_dir(&waiting).map(|rd| rd.count()).unwrap_or(0);
            if count >= 2 {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }

        thread::sleep(Duration::from_millis(20));
        release_holder.store(true, Ordering::SeqCst);
        holder.join().unwrap();
        critical.join().unwrap();
        late_normal.join().unwrap();

        let seq = order.lock().unwrap().clone();
        assert_eq!(
            seq,
            vec!["holder_start", "holder_end", "critical", "normal_late"],
            "AC-008.14: Critical MUST dequeue before Normal"
        );
    }

    /// Phase-0.4 (Lane-7 BLOCKER, tiger): the closure can panic, error, or
    /// time out and the waiter ticket MUST still be removed. The previous
    /// code only removed the ticket on the happy path, leaking it on every
    /// error and starving the lane forever.
    #[test]
    fn with_slot_cleans_ticket_on_closure_error() {
        let dir = TempDir::new().unwrap();
        let q = SlotQueue::with_options(
            dir.path(),
            1,
            Duration::from_secs(2),
            Duration::from_millis(20),
        );
        let waiting = dir.path().join("err.waiting");
        let res = q.with_slot::<()>("err", QueuePriority::Normal, || {
            anyhow::bail!("intentional closure failure");
        });
        assert!(res.is_err());
        // The waiter ticket MUST have been removed by the RAII guard even
        // though the closure returned Err.
        thread::sleep(Duration::from_millis(50));
        let count = fs::read_dir(&waiting).map(|rd| rd.count()).unwrap_or(0);
        assert_eq!(count, 0, "ticket must be cleaned up after closure error");
    }

    /// Phase-0.4: timeout path (no free slot within deadline) MUST clean the
    /// ticket too, otherwise subsequent waiters see a phantom waiter.
    #[test]
    fn with_slot_cleans_ticket_on_timeout() {
        let dir = TempDir::new().unwrap();
        let q = SlotQueue::with_options(
            dir.path(),
            1,
            // Tight timeout so the test runs in <1s.
            Duration::from_millis(200),
            Duration::from_millis(20),
        );
        let waiting = dir.path().join("timeout.waiting");

        // Hold the only slot for the full timeout window.
        let holder_started = Arc::new(AtomicBool::new(false));
        let holder_started_clone = Arc::clone(&holder_started);
        let q_root = dir.path().to_path_buf();
        let holder = thread::spawn(move || {
            let qh = SlotQueue::with_options(
                q_root,
                1,
                Duration::from_secs(5),
                Duration::from_millis(10),
            );
            qh.with_slot("timeout", QueuePriority::Normal, || {
                holder_started_clone.store(true, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(600));
                Ok(())
            })
            .unwrap();
        });
        while !holder_started.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(5));
        }

        let res = q.with_slot("timeout", QueuePriority::Normal, || {
            Ok::<_, anyhow::Error>(())
        });
        assert!(res.is_err(), "second caller must time out");
        // The timing-out waiter must have cleaned its ticket. The holder's
        // ticket has already been removed by its own success-path `release()`.
        thread::sleep(Duration::from_millis(50));
        let count = fs::read_dir(&waiting).map(|rd| rd.count()).unwrap_or(0);
        assert_eq!(count, 0, "no leftover waiter tickets after timeout");
        holder.join().unwrap();
    }

    /// Phase-0.4 (Lane-7 BLOCKER, tiger): rank aging. A Critical ticket that
    /// has been waiting > AGING_STEP_MS decays one rank at a time, eventually
    /// yielding to a fresh Normal waiter with free slots.
    #[test]
    fn effective_rank_decays_over_time() {
        // Critical at t=0 is rank 0.
        assert_eq!(
            SlotQueue::effective_rank("ignored", QueuePriority::Critical, Duration::from_millis(0)),
            0
        );
        // Under 1 s of waiting: no decay.
        assert_eq!(
            SlotQueue::effective_rank(
                "ignored",
                QueuePriority::Critical,
                Duration::from_millis(999)
            ),
            0
        );
        // At 1 s exactly: one rank step down (Critical -> High).
        assert_eq!(
            SlotQueue::effective_rank(
                "ignored",
                QueuePriority::Critical,
                Duration::from_millis(1_000)
            ),
            1
        );
        // At 2 s: two steps (Critical -> Normal).
        assert_eq!(
            SlotQueue::effective_rank(
                "ignored",
                QueuePriority::Critical,
                Duration::from_millis(2_000)
            ),
            2
        );
        // Normal at 0 s vs Critical at 2 s: same effective rank, FIFO decides.
        assert_eq!(
            SlotQueue::effective_rank("ignored", QueuePriority::Normal, Duration::from_millis(0)),
            SlotQueue::effective_rank(
                "ignored",
                QueuePriority::Critical,
                Duration::from_millis(2_000)
            ),
        );
        // Normal at 0 s beats Critical at 3 s (rank 2 vs rank 3).
        assert!(
            SlotQueue::effective_rank("ignored", QueuePriority::Normal, Duration::ZERO)
                < SlotQueue::effective_rank(
                    "ignored",
                    QueuePriority::Critical,
                    Duration::from_millis(3_000)
                )
        );
        // Background at 0 s == Critical at 4 s (both rank 4): FIFO decides.
        assert_eq!(
            SlotQueue::effective_rank(
                "ignored",
                QueuePriority::Background,
                Duration::from_millis(0)
            ),
            SlotQueue::effective_rank(
                "ignored",
                QueuePriority::Critical,
                Duration::from_millis(4_000)
            )
        );
    }

    /// Phase-0.4: end-to-end aging. We create an orphan Critical ticket in
    /// the waiting dir (no holder), a Normal waiter arrives, and after the
    /// Normal ticket's age-adjusted rank catches up to the (decayed) Critical
    /// rank, the Normal waiter is allowed to proceed.
    #[test]
    fn orphan_critical_does_not_starve_fresh_normal() {
        let dir = TempDir::new().unwrap();
        let q_root = dir.path().to_path_buf();
        let waiting = q_root.join("orphan.waiting");

        // Pre-place an orphan Critical ticket that "appeared" > 3 seconds ago.
        fs::create_dir_all(&waiting).unwrap();
        let old_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(4);
        let orphan = format!("00.{old_secs}.{}.0", std::process::id());
        fs::write(waiting.join(&orphan), "0\n").unwrap();

        // A Normal waiter arrives. The orphan Critical decayed to rank >=3
        // (4 s of waiting -> 4 steps down from rank 0 = rank 4), so the
        // Normal ticket is now best (rank 2). It must proceed without
        // waiting for the orphan — but the orphan's ticket file is still in
        // the dir, which our function handles by checking only base ranks.
        let q = SlotQueue::with_options(
            q_root,
            1,
            Duration::from_secs(2),
            Duration::from_millis(20),
        );
        let started = Instant::now();
        q.with_slot("orphan", QueuePriority::Normal, || {
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "normal waiter must not wait the full timeout; orphan should not block"
            );
            Ok::<_, anyhow::Error>(())
        })
        .expect("normal waiter must succeed past the orphan Critical");
    }

    /// FR-008 / AC-008.15 — operator env overrides rules.conf priority.
    #[test]
    #[serial_test::serial]
    fn resolve_operator_queue_priority_env_overrides_rule() {
        unsafe {
            std::env::set_var(QUEUE_PRIORITY_ENV, "critical");
        }
        assert_eq!(
            resolve_operator_queue_priority(Some("low")),
            QueuePriority::Critical,
            "AC-008.15: SHARECLI_QUEUE_PRIORITY MUST win over rules.conf"
        );
        unsafe {
            std::env::remove_var(QUEUE_PRIORITY_ENV);
        }
    }

    /// FR-008 / AC-008.15 — rules.conf priority fallback when env unset.
    #[test]
    #[serial_test::serial]
    fn resolve_operator_queue_priority_rule_fallback() {
        unsafe {
            std::env::remove_var(QUEUE_PRIORITY_ENV);
        }
        assert_eq!(
            resolve_operator_queue_priority(Some("high")),
            QueuePriority::High,
            "AC-008.15: rules.conf priority MUST map to QueuePriority"
        );
        assert_eq!(resolve_operator_queue_priority(None), QueuePriority::Normal);
    }

    // --- Additional edge-case tests ---

    #[test]
    fn priority_parse_case_insensitive() {
        assert_eq!(QueuePriority::parse("Critical"), QueuePriority::Critical);
        assert_eq!(QueuePriority::parse("LOW"), QueuePriority::Low);
        assert_eq!(QueuePriority::parse("Background"), QueuePriority::Background);
        assert_eq!(QueuePriority::parse("NORMAL"), QueuePriority::Normal);
        assert_eq!(QueuePriority::parse("  High  "), QueuePriority::High);
    }

    #[test]
    fn priority_as_u8() {
        assert_eq!(QueuePriority::Critical.as_u8(), 0);
        assert_eq!(QueuePriority::High.as_u8(), 1);
        assert_eq!(QueuePriority::Normal.as_u8(), 2);
        assert_eq!(QueuePriority::Low.as_u8(), 3);
        assert_eq!(QueuePriority::Background.as_u8(), 4);
    }

    #[test]
    fn priority_ordering() {
        assert!(QueuePriority::Critical < QueuePriority::High);
        assert!(QueuePriority::High < QueuePriority::Normal);
        assert!(QueuePriority::Normal < QueuePriority::Low);
        assert!(QueuePriority::Low < QueuePriority::Background);
    }

    #[test]
    fn priority_default_is_normal() {
        assert_eq!(QueuePriority::default(), QueuePriority::Normal);
    }

    #[test]
    fn slot_queue_root_and_max_concurrent() {
        let dir = tempfile::tempdir().unwrap();
        let q = SlotQueue::new(dir.path(), 3);
        assert_eq!(q.root(), dir.path());
        assert_eq!(q.max_concurrent(), 3);
    }

    #[test]
    fn slot_queue_max_concurrent_min_one() {
        let dir = tempfile::tempdir().unwrap();
        let q = SlotQueue::new(dir.path(), 0);
        assert_eq!(q.max_concurrent(), 1, "max_concurrent=0 MUST clamp to 1");
    }

    #[test]
    fn with_slot_returns_ok_value() {
        let dir = tempfile::tempdir().unwrap();
        let q = SlotQueue::new(dir.path(), 2);
        let result = q.with_slot("test-lane", QueuePriority::Normal, || Ok(42)).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn with_slot_propagates_error() {
        let dir = tempfile::tempdir().unwrap();
        let q = SlotQueue::new(dir.path(), 2);
        let result: Result<Option<String>, _> =
            q.with_slot("err-lane", QueuePriority::Normal, || anyhow::bail!("intentional error"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("intentional error"));
    }

    #[test]
    fn with_slot_creates_root_dir() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested").join("queue");
        let q = SlotQueue::new(&nested, 1);
        assert!(!nested.exists(), "root must not exist yet");
        q.with_slot("lane", QueuePriority::Normal, || Ok(())).unwrap();
        assert!(nested.exists(), "root MUST be created");
    }

    #[test]
    fn with_slot_timeout_when_all_slots_busy() {
        let dir = tempfile::tempdir().unwrap();
        let q = SlotQueue::with_options(
            dir.path(),
            1,
            Duration::from_millis(100), // very short timeout
            Duration::from_millis(10),
        );
        let active = Arc::new(AtomicBool::new(false));
        let active_flag = Arc::clone(&active);
        let root = dir.path().to_path_buf();

        // Hold the only slot in a background thread
        let holder = thread::spawn(move || {
            let q2 = SlotQueue::with_options(
                &root,
                1,
                Duration::from_secs(5),
                Duration::from_millis(10),
            );
            q2.with_slot("timeout-lane", QueuePriority::Normal, || {
                active_flag.store(true, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(500));
                Ok(())
            })
            .unwrap();
        });

        // Wait for holder to acquire
        while !active.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(5));
        }

        // Second attempt should timeout
        let result = q.with_slot("timeout-lane", QueuePriority::Normal, || Ok(()));
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("queue timeout"),
            "MUST report queue timeout"
        );

        holder.join().unwrap();
    }

    #[test]
    fn ticket_priority_parses_first_field() {
        assert_eq!(SlotQueue::ticket_priority("00.123.456.789"), 0);
        assert_eq!(SlotQueue::ticket_priority("02.123.456.789"), 2);
        assert_eq!(SlotQueue::ticket_priority("04.123.456.789"), 4);
        assert_eq!(SlotQueue::ticket_priority("bad-format"), QueuePriority::Normal.as_u8());
    }

    #[test]
    fn with_slot_concurrent_different_lanes() {
        let dir = tempfile::tempdir().unwrap();
        let active = Arc::new(AtomicU32::new(0));
        let peak = Arc::new(AtomicU32::new(0));
        let mut handles = vec![];

        for lane in ["lane-a", "lane-b", "lane-c"] {
            let root = dir.path().to_path_buf();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            let lane = lane.to_string();
            handles.push(thread::spawn(move || {
                let q = SlotQueue::with_options(
                    &root,
                    1,
                    Duration::from_secs(5),
                    Duration::from_millis(10),
                );
                q.with_slot(&lane, QueuePriority::Normal, || {
                    let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(n, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(30));
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
                .unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
        // Different lanes with max_concurrent=1 per lane should allow parallel execution
        assert!(peak.load(Ordering::SeqCst) >= 2, "different lanes MUST allow parallel execution");
    }

    #[test]
    #[serial_test::serial]
    fn resolve_operator_queue_priority_empty_env_uses_rule() {
        unsafe {
            std::env::set_var(QUEUE_PRIORITY_ENV, "  ");
        }
        assert_eq!(
            resolve_operator_queue_priority(Some("low")),
            QueuePriority::Low,
            "empty/whitespace env MUST fall through to rules.conf"
        );
        unsafe {
            std::env::remove_var(QUEUE_PRIORITY_ENV);
        }
    }

    #[test]
    #[serial_test::serial]
    fn resolve_operator_queue_priority_no_env_no_rule_is_normal() {
        unsafe {
            std::env::remove_var(QUEUE_PRIORITY_ENV);
        }
        assert_eq!(resolve_operator_queue_priority(None), QueuePriority::Normal);
    }

    #[test]
    fn with_slot_priority_tagged_waiters() {
        // Verify that tickets are written with priority prefix
        let dir = tempfile::tempdir().unwrap();
        let _q = SlotQueue::new(dir.path(), 1);
        let root = dir.path().to_path_buf();

        let active = Arc::new(AtomicBool::new(false));
        let active_flag = Arc::clone(&active);
        let release = Arc::new(AtomicBool::new(false));
        let release_flag = Arc::clone(&release);

        // Thread 1 holds the slot
        let t1_root = root.clone();
        let t1 = thread::spawn(move || {
            let q2 = SlotQueue::with_options(
                &t1_root,
                1,
                Duration::from_secs(5),
                Duration::from_millis(5),
            );
            q2.with_slot("priority-lane", QueuePriority::Low, || {
                active_flag.store(true, Ordering::SeqCst);
                while !release_flag.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(5));
                }
                Ok(())
            })
            .unwrap();
        });

        while !active.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(2));
        }

        // Thread 2 queues a Critical waiter
        let t2_root = root.clone();
        let t2 = thread::spawn(move || {
            let q2 = SlotQueue::with_options(
                &t2_root,
                1,
                Duration::from_secs(5),
                Duration::from_millis(5),
            );
            q2.with_slot("priority-lane", QueuePriority::Critical, || Ok(())).unwrap();
        });

        // Give time for waiter ticket to be written
        thread::sleep(Duration::from_millis(50));

        // Verify a waiter file exists with priority prefix
        let waiting_dir = root.join("priority-lane.waiting");
        let entries: Vec<_> = fs::read_dir(&waiting_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .collect();
        assert!(!entries.is_empty(), "MUST have waiter ticket");
        let ticket_name = entries[0].file_name().to_string_lossy().to_string();
        assert!(
            ticket_name.starts_with('0'),
            "Critical waiter MUST have priority prefix 0; got: {ticket_name}"
        );

        release.store(true, Ordering::SeqCst);
        t1.join().unwrap();
        t2.join().unwrap();
    }
}
