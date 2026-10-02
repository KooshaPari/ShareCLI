//! N-slot priority queue for mutating / nocache command paths.
//!
//! FR: FR-008 / SC-WP-B03. Slot ownership and waiter ownership are separate
//! kernel-held file locks. A filename or a live PID is not a waiter lease.
//! Existing slot paths are retained. Waiter files are uniquely published only
//! after their lease is held. Unleased files are ignored, not attributed to a
//! process and not unlinked by a peer. Mixed-version ordering and remote-file-
//! system locking are not qualified by the local Linux recovery tests.

use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use fs2::FileExt;
use sharecli_fleet::{record_slot_acquire, record_slot_timeout, record_slot_wait};

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

pub const QUEUE_PRIORITY_ENV: &str = "SHARECLI_QUEUE_PRIORITY";

pub fn resolve_operator_queue_priority(rule_priority: Option<&str>) -> QueuePriority {
    if let Ok(raw) = std::env::var(QUEUE_PRIORITY_ENV) {
        if !raw.trim().is_empty() {
            return QueuePriority::parse(&raw);
        }
    }
    rule_priority.map(QueuePriority::parse).unwrap_or(QueuePriority::Normal)
}

pub struct SlotQueue {
    root: PathBuf,
    max_concurrent: usize,
    timeout: Duration,
    poll: Duration,
}

impl SlotQueue {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
    pub const DEFAULT_POLL: Duration = Duration::from_millis(100);

    pub fn new(root: impl Into<PathBuf>, max_concurrent: usize) -> Self {
        Self::with_options(root, max_concurrent, Self::DEFAULT_TIMEOUT, Self::DEFAULT_POLL)
    }

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

    fn enqueue_waiter(&self, lane: &str, priority: QueuePriority) -> Result<(WaiterTicketGuard, String)> {
        let dir = self.waiting_dir(lane);
        fs::create_dir_all(&dir)
            .with_context(|| format!("create waiting dir {}", dir.display()))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        // Serialize publication and sequence allocation across processes.
        // PID and wall-clock ties cannot decide FIFO. The append-only counter
        // never resets a malformed tail to zero after a publisher crash.
        let (order_guard, seq) = self.next_waiter_sequence(lane)?;
        let mut tmp = tempfile::Builder::new().prefix(".lease-").tempfile_in(&dir)?;
        FileExt::try_lock_exclusive(tmp.as_file()).context("hold waiter lease before publication")?;
        writeln!(tmp, "{}", priority.as_u8())?;
        let nonce = tmp.path().file_name().unwrap().to_string_lossy().trim_start_matches('.').to_owned();
        let ticket = format!("{:02}.{now}.{}.{seq}.{nonce}", priority.as_u8(), std::process::id());
        let path = dir.join(&ticket);
        // Never replace an existing waiter, even on clock/PID/sequence reuse.
        let file = tmp.persist_noclobber(&path).map_err(|e| anyhow::anyhow!("publish waiter lease: {e}"))?;
        drop(order_guard);
        Ok((WaiterTicketGuard { path, file: Some(file) }, ticket))
    }

    fn next_waiter_sequence(&self, lane: &str) -> Result<(fs::File, u64)> {
        let path = self.root.join(format!("{lane}.enqueue.lock"));
        let mut options = fs::OpenOptions::new();
        options.create(true).read(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&path).context("open queue ordering journal")?;
        let deadline = Instant::now() + self.timeout;
        loop {
            match FileExt::try_lock_exclusive(&file) {
                Ok(()) => break,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        anyhow::bail!("queue timeout: waiter publication is busy");
                    }
                    thread::sleep(self.poll);
                }
                Err(e) => return Err(e).context("lock queue ordering journal"),
            }
        }
        let len = file.metadata()?.len();
        let previous = if len == 0 {
            0
        } else {
            // A decimal u64 plus newline fits within 21 bytes. Reading the
            // last 64 bytes also tolerates a partial older line at the start.
            file.seek(SeekFrom::Start(len.saturating_sub(64)))?;
            let mut tail = String::new();
            file.read_to_string(&mut tail)?;
            if !tail.ends_with('\n') {
                anyhow::bail!("queue ordering journal has an incomplete tail");
            }
            tail.lines().last().unwrap_or("").parse::<u64>()
                .context("invalid queue ordering journal sequence")?
        };
        let next = previous.checked_add(1).context("queue waiter sequence exhausted")?;
        writeln!(file, "{next}")?;
        file.flush()?;
        Ok((file, next))
    }

    fn ticket_priority(ticket: &str) -> u8 {
        ticket.split('.').next().and_then(|head| head.parse::<u8>().ok())
            .unwrap_or(QueuePriority::Normal.as_u8())
    }

    const AGING_STEP_MS: u128 = 1_000;

    fn effective_rank(_ticket: &str, base_priority: QueuePriority, waited: Duration) -> u8 {
        let decay_steps = (waited.as_millis() / Self::AGING_STEP_MS).min(u128::from(u8::MAX)) as u8;
        base_priority.as_u8().saturating_add(decay_steps)
    }

    fn ticket_fifo_key(ticket: &str) -> (u64, &str) {
        let mut parts = ticket.split('.');
        let _rank = parts.next();
        let _secs = parts.next();
        let _pid = parts.next();
        let seq = parts.next().and_then(|v| v.parse().ok()).unwrap_or(u64::MAX);
        (seq, ticket)
    }

    /// A separate open file description must conflict with the owner's lock.
    /// No PID, age, filename or file content can manufacture this authority.
    fn ticket_has_lease(path: &Path) -> Result<bool> {
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = match options.open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e).with_context(|| format!("open waiter lease {}", path.display())),
        };
        if !file.metadata()?.is_file() {
            anyhow::bail!("waiter lease is not a regular file: {}", path.display());
        }
        match FileExt::try_lock_shared(&file) {
            Ok(()) => Ok(false), // shared probes do not impersonate an exclusive owner
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(true),
            Err(e) => Err(e).with_context(|| format!("probe waiter lease {}", path.display())),
        }
    }

    fn is_my_turn(
        &self,
        lane: &str,
        my_priority: QueuePriority,
        my_ticket: &str,
        my_enqueued_at: Instant,
    ) -> Result<bool> {
        let dir = self.waiting_dir(lane);
        let entries = fs::read_dir(&dir)
            .with_context(|| format!("read waiting dir {}", dir.display()))?;
        let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let my_effective = Self::effective_rank(my_ticket, my_priority, my_enqueued_at.elapsed());
        let mut best_effective = u8::MAX;
        let mut tickets_at_best = Vec::new();
        let mut own_claim_seen = false;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            if !kind.is_file() {
                continue;
            }
            let ticket = entry.file_name().to_string_lossy().into_owned();
            if ticket.starts_with('.') || !Self::ticket_has_lease(&entry.path())? {
                continue;
            }
            let is_mine = ticket == my_ticket;
            own_claim_seen |= is_mine;
            let base_priority = match Self::ticket_priority(&ticket) {
                0 => QueuePriority::Critical,
                1 => QueuePriority::High,
                2 => QueuePriority::Normal,
                3 => QueuePriority::Low,
                _ => QueuePriority::Background,
            };
            let ticket_secs = ticket.split('.').nth(1).and_then(|v| v.parse::<u64>().ok()).unwrap_or(now_secs);
            let waited = Duration::from_secs(now_secs.saturating_sub(ticket_secs));
            let effective = if is_mine { my_effective } else { Self::effective_rank(&ticket, base_priority, waited) };
            if effective < best_effective {
                best_effective = effective;
                tickets_at_best.clear();
                tickets_at_best.push(ticket);
            } else if effective == best_effective {
                tickets_at_best.push(ticket);
            }
        }
        if !own_claim_seen {
            anyhow::bail!("published waiter lease disappeared: {my_ticket}");
        }
        // Rank 255 is a real cohort, not an empty-cohort sentinel.
        let winner = tickets_at_best.iter().min_by_key(|t| Self::ticket_fifo_key(t));
        Ok(winner.map(String::as_str) == Some(my_ticket))
    }

    pub fn with_slot<T>(
        &self,
        lane: &str,
        priority: QueuePriority,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        if lane.trim().is_empty() || lane == "." || lane == ".." || lane.contains(['/', '\\', '\0']) {
            anyhow::bail!("invalid queue lane");
        }
        self.ensure_root()?;
        let (mut guard, ticket) = self.enqueue_waiter(lane, priority)?;
        let enqueued_at = Instant::now();
        let deadline = enqueued_at + self.timeout;
        loop {
            if Instant::now() >= deadline {
                record_slot_timeout();
                anyhow::bail!("queue timeout: no free slot for lane `{lane}` within {}s (max_concurrent={})", self.timeout.as_secs(), self.max_concurrent);
            }
            if !self.is_my_turn(lane, priority, &ticket, enqueued_at)? {
                record_slot_wait();
                thread::sleep(self.poll);
                continue;
            }
            for slot in 0..self.max_concurrent {
                let lock_path = self.slot_path(lane, slot);
                let lock_file = fs::OpenOptions::new().create(true).write(true).truncate(false)
                    .open(&lock_path).with_context(|| format!("open slot lock {}", lock_path.display()))?;
                match FileExt::try_lock_exclusive(&lock_file) {
                    Ok(()) => {
                        if !self.is_my_turn(lane, priority, &ticket, enqueued_at)? {
                            drop(lock_file);
                            continue;
                        }
                        guard.release();
                        record_slot_acquire();
                        let value = f()?;
                        drop(lock_file);
                        return Ok(value);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                    Err(e) => return Err(e).context("acquire execution slot lock"),
                }
            }
            record_slot_wait();
            thread::sleep(self.poll);
        }
    }
}

struct WaiterTicketGuard {
    path: PathBuf,
    file: Option<fs::File>,
}

impl WaiterTicketGuard {
    fn release(&mut self) {
        if let Some(file) = self.file.take() {
            // Unlink while ownership is held on Unix. Retry after close for
            // platforms that cannot remove an open file. Names are unique and
            // publication never replaces an existing path.
            let removed = fs::remove_file(&self.path).is_ok();
            drop(file);
            if !removed {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

impl Drop for WaiterTicketGuard {
    fn drop(&mut self) {
        self.release();
    }
}

pub type PriorityQueue = SlotQueue;

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
