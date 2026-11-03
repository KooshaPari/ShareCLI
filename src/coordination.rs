// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockStatus {
    Unlocked,
    Locked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandLock {
    pub cmd_hash: String,
    pub pid: u32,
    pub status: LockStatus,
    pub output_path: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
}

impl CommandLock {
    pub fn is_locked(&self) -> bool {
        self.pid != 0 && self.status == LockStatus::Locked
    }
}

#[derive(Clone, Debug)]
pub struct CommandLockStore {
    path: PathBuf,
}

impl CommandLockStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn acquire(
        &self,
        cmd_hash: impl Into<String>,
        pid: u32,
        output_path: Option<&str>,
    ) -> Result<CommandLock> {
        let cmd_hash = cmd_hash.into();
        let mut locks = self.read_locks()?;

        if let Some(lock) = locks.iter_mut().find(|lock| lock.cmd_hash == cmd_hash) {
            if lock.is_locked() && lock.pid != pid {
                bail!("already locked");
            }
            lock.acquire(pid, output_path);
            let acquired = lock.clone();
            self.write_locks(&locks)?;
            return Ok(acquired);
        }

        let lock = CommandLock {
            cmd_hash,
            pid,
            status: LockStatus::Locked,
            output_path: output_path.map(str::to_owned),
            start_time: Some(Utc::now()),
        };
        locks.push(lock.clone());
        self.write_locks(&locks)?;
        Ok(lock)
    }

    pub fn release(&self, cmd_hash: &str, pid: u32) -> Result<()> {
        let mut locks = self.read_locks()?;
        let lock = locks
            .iter_mut()
            .find(|lock| lock.cmd_hash == cmd_hash)
            .with_context(|| format!("No lock found for {cmd_hash}"))?;

        if lock.pid != pid {
            bail!("Lock held by PID {}, cannot release", lock.pid);
        }

        lock.pid = 0;
        lock.status = LockStatus::Unlocked;
        lock.start_time = None;
        self.write_locks(&locks)
    }

    pub fn get(&self, cmd_hash: &str) -> Result<Option<CommandLock>> {
        Ok(self.read_locks()?.into_iter().find(|lock| lock.cmd_hash == cmd_hash))
    }

    pub fn list_all(&self) -> Result<Vec<CommandLock>> {
        self.read_locks()
    }

    fn read_locks(&self) -> Result<Vec<CommandLock>> {
        read_json_array(&self.path)
    }

    fn write_locks(&self, locks: &[CommandLock]) -> Result<()> {
        write_json_array(&self.path, locks)
    }
}

impl CommandLock {
    fn acquire(&mut self, pid: u32, output_path: Option<&str>) {
        self.pid = pid;
        self.status = LockStatus::Locked;
        self.output_path = output_path.map(str::to_owned);
        self.start_time = Some(Utc::now());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Dequeued,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskQueueItem {
    pub id: String,
    pub command: String,
    pub priority: QueuePriority,
    pub created_at: DateTime<Utc>,
    pub status: TaskStatus,
}

#[derive(Clone, Debug)]
pub struct PriorityTaskQueue {
    path: PathBuf,
}

impl PriorityTaskQueue {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn enqueue(
        &self,
        command: impl Into<String>,
        priority: QueuePriority,
    ) -> Result<TaskQueueItem> {
        let mut items = self.read_items()?;
        let item = TaskQueueItem {
            id: format!("task-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default()),
            command: command.into(),
            priority,
            created_at: Utc::now(),
            status: TaskStatus::Pending,
        };
        items.push(item.clone());
        sort_by_priority(&mut items);
        self.write_items(&items)?;
        Ok(item)
    }

    pub fn dequeue(&self) -> Result<Option<TaskQueueItem>> {
        let mut items = self.read_items()?;
        sort_by_priority(&mut items);
        if items.is_empty() {
            return Ok(None);
        }

        let mut item = items.remove(0);
        item.status = TaskStatus::Dequeued;
        self.write_items(&items)?;
        Ok(Some(item))
    }

    pub fn peek(&self) -> Result<Option<TaskQueueItem>> {
        let mut items = self.read_items()?;
        sort_by_priority(&mut items);
        Ok(items.into_iter().next())
    }

    pub fn len(&self) -> Result<usize> {
        Ok(self.read_items()?.len())
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    pub fn clear(&self) -> Result<()> {
        self.write_items(&[])
    }

    pub fn list_all(&self) -> Result<Vec<TaskQueueItem>> {
        let mut items = self.read_items()?;
        sort_by_priority(&mut items);
        Ok(items)
    }

    fn read_items(&self) -> Result<Vec<TaskQueueItem>> {
        read_json_array(&self.path)
    }

    fn write_items(&self, items: &[TaskQueueItem]) -> Result<()> {
        write_json_array(&self.path, items)
    }
}

fn sort_by_priority(items: &mut [TaskQueueItem]) {
    items.sort_by_key(|item| priority_rank(item.priority));
}

fn priority_rank(priority: QueuePriority) -> u8 {
    match priority {
        QueuePriority::Critical => 0,
        QueuePriority::High => 1,
        QueuePriority::Normal => 2,
        QueuePriority::Low => 3,
    }
}

fn read_json_array<T>(path: &Path) -> Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn write_json_array<T>(path: &Path, items: &[T]) -> Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(items)?;
    fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))
}
