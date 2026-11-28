// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Persisted per-project resource limits (`~/.sharecli/limits.json`) and the
//! measured comparison used by `limits check` / `optimize`.
//!
//! Before this module a limit lived in an in-memory `RwLock<HashMap>` inside
//! [`crate::runtime::ProjectResources`], so nothing survived process exit and
//! "check" could only compare defaults against defaults. A limit that cannot
//! outlive the invocation that set it is not a limit, so the operator-set
//! values now live on disk:
//!
//! ```text
//! { "version": 1,
//!   "global":   { "memory_limit_mb": 512, "max_processes": 4 },
//!   "projects": { "sharecli": { "memory_limit_mb": 768, "max_processes": 6 } } }
//! ```
//!
//! Resolution order for a project is project override → global override →
//! `[project_limits]` config defaults. [`DetectionSnapshot`] measures usage
//! through the same detection path `sharecli proc` uses
//! ([`sharecli_fleet::HostProcSource`] + [`sharecli_fleet::match_known_agent`]),
//! and [`evaluate`] turns (limit, measurement) pairs into
//! [`LimitVerdict`]s. No measurement yields [`LimitVerdict::NoData`]; a
//! default is never reported as if it were an observation.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sharecli_fleet::{AgentResourceSample, HostProcSource, ProcSource};

use crate::config::ProjectLimitsConfig;

/// Environment override for the limits file path (test + ops escape hatch).
pub const LIMITS_PATH_ENV: &str = "SHARECLI_LIMITS_PATH";

fn schema_version() -> u32 {
    1
}

/// One memory + process-count cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitEntry {
    pub memory_limit_mb: u64,
    pub max_processes: usize,
}

impl LimitEntry {
    pub fn from_defaults(defaults: &ProjectLimitsConfig) -> Self {
        Self {
            memory_limit_mb: defaults.memory_limit_mb,
            max_processes: defaults.max_processes,
        }
    }

    /// MB above the memory cap (`0` when within it).
    pub fn over_memory_mb(&self, used_mb: u64) -> u64 {
        used_mb.saturating_sub(self.memory_limit_mb)
    }

    /// Processes above the process cap (`0` when within it).
    pub fn over_processes(&self, procs: usize) -> usize {
        procs.saturating_sub(self.max_processes)
    }
}

/// On-disk limits document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitsFile {
    #[serde(default = "schema_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global: Option<LimitEntry>,
    #[serde(default)]
    pub projects: BTreeMap<String, LimitEntry>,
}

impl Default for LimitsFile {
    fn default() -> Self {
        Self { version: schema_version(), global: None, projects: BTreeMap::new() }
    }
}

/// Where an effective limit came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitSource {
    /// Persisted per-project override.
    ProjectOverride,
    /// Persisted global override.
    GlobalOverride,
    /// `[project_limits]` config default — *not* a measurement, and not persisted.
    Default,
}

impl LimitSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::ProjectOverride => "project-override",
            Self::GlobalOverride => "global-override",
            Self::Default => "default",
        }
    }
}

/// Handle to the limits document on disk.
#[derive(Debug, Clone)]
pub struct LimitsStore {
    path: PathBuf,
}

impl Default for LimitsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LimitsStore {
    /// `$SHARECLI_LIMITS_PATH` if set, else `~/.sharecli/limits.json`.
    pub fn new() -> Self {
        Self { path: default_limits_path() }
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the document; a missing file is an empty document, a corrupt file
    /// is an error (silently resetting operator intent would hide the fault).
    pub fn load(&self) -> Result<LimitsFile> {
        if !self.path.exists() {
            return Ok(LimitsFile::default());
        }
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("read limits file {}", self.path.display()))?;
        if raw.trim().is_empty() {
            return Ok(LimitsFile::default());
        }
        serde_json::from_str(&raw)
            .with_context(|| format!("parse limits file {}", self.path.display()))
    }

    /// Atomic write: temp file (0600) → rename over the target. The parent
    /// directory is created 0700 on unix.
    pub fn save(&self, file: &LimitsFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            create_private_dir(parent)?;
        }
        let tmp = self.path.with_extension(format!("json.tmp.{}", std::process::id()));
        let body = serde_json::to_vec_pretty(file).context("serialize limits")?;
        {
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                opts.mode(0o600);
            }
            let mut handle = opts
                .open(&tmp)
                .with_context(|| format!("create temp limits file {}", tmp.display()))?;
            handle.write_all(&body)?;
            handle.write_all(b"\n")?;
            handle.sync_all()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("rename into {}", self.path.display()))?;
        Ok(())
    }

    /// Effective limit for `project`, plus where it came from.
    pub fn effective(
        &self,
        file: &LimitsFile,
        project: &str,
        defaults: &ProjectLimitsConfig,
    ) -> (LimitEntry, LimitSource) {
        resolve(file, project, defaults)
    }
}

/// Project override → global override → config default.
pub fn resolve(
    file: &LimitsFile,
    project: &str,
    defaults: &ProjectLimitsConfig,
) -> (LimitEntry, LimitSource) {
    if let Some(entry) = file.projects.get(project) {
        return (*entry, LimitSource::ProjectOverride);
    }
    if let Some(entry) = file.global {
        return (entry, LimitSource::GlobalOverride);
    }
    (LimitEntry::from_defaults(defaults), LimitSource::Default)
}

pub fn default_limits_path() -> PathBuf {
    if let Ok(path) = std::env::var(LIMITS_PATH_ENV) {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    match dirs::home_dir() {
        Some(home) => home.join(".sharecli").join("limits.json"),
        None => PathBuf::from(".sharecli").join("limits.json"),
    }
}

fn create_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("create limits dir {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Verdict for one project row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitVerdict {
    /// Measured usage is within the effective limit.
    Ok,
    /// Measured usage exceeds the effective limit.
    Over,
    /// Detection produced no measurement for this project.
    NoData,
}

impl LimitVerdict {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Over => "OVER",
            Self::NoData => "NO_DATA",
        }
    }
}

/// Whether the host detection path could read a process table at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionState {
    Available,
    Unavailable,
}

/// Measured usage attributed to one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectUsage {
    pub project: String,
    pub procs: usize,
    pub memory_mb: u64,
}

/// Measured usage per detected agent family (`kcode`, `claude`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyUsage {
    pub family: String,
    pub procs: usize,
    pub memory_mb: u64,
}

/// One detection pass over the host, shaped for limit evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionSnapshot {
    pub state: DetectionState,
    pub agent_count: usize,
    pub total_memory_mb: u64,
    pub projects: Vec<ProjectUsage>,
    pub families: Vec<FamilyUsage>,
}

impl DetectionSnapshot {
    pub fn unavailable() -> Self {
        Self {
            state: DetectionState::Unavailable,
            agent_count: 0,
            total_memory_mb: 0,
            projects: Vec::new(),
            families: Vec::new(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.state == DetectionState::Available
    }
}

/// One evaluated table row: effective limit, measured usage (if any), verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitRow {
    pub project: String,
    pub limit: LimitEntry,
    pub source: LimitSource,
    pub usage: Option<ProjectUsage>,
    pub verdict: LimitVerdict,
}

impl LimitRow {
    pub fn over_memory_mb(&self) -> u64 {
        self.usage.as_ref().map(|u| self.limit.over_memory_mb(u.memory_mb)).unwrap_or(0)
    }

    pub fn over_processes(&self) -> usize {
        self.usage.as_ref().map(|u| self.limit.over_processes(u.procs)).unwrap_or(0)
    }
}

/// Measure the host through the `sharecli proc` detection path and attribute
/// detected agents to `projects` (name → optional configured path).
pub fn detect(projects: &[(String, Option<String>)]) -> DetectionSnapshot {
    let source = HostProcSource;
    let table = source.list();
    if table.is_empty() {
        return DetectionSnapshot::unavailable();
    }

    let mut agent_count = 0usize;
    let mut total_memory_mb = 0u64;
    let mut by_family: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    let mut by_project: HashMap<String, (usize, u64)> = HashMap::new();

    for row in &table {
        let Some(family) = sharecli_fleet::match_known_agent(&row.comm, &row.cmdline) else {
            continue;
        };
        let memory_mb = AgentResourceSample::capture_for_pid(row.pid)
            .map(|sample| sample.mem_rss_bytes / 1_048_576)
            .unwrap_or(0);

        agent_count += 1;
        total_memory_mb += memory_mb;
        let family_entry = by_family.entry(family.to_string()).or_insert((0, 0));
        family_entry.0 += 1;
        family_entry.1 += memory_mb;

        let joined = row.cmdline.join(" ").to_lowercase();
        for (name, path) in projects {
            let name_hit = name.len() >= 3 && joined.contains(&name.to_lowercase());
            let path_hit = path
                .as_deref()
                .filter(|p| p.len() >= 6)
                .map(|p| joined.contains(&p.to_lowercase()))
                .unwrap_or(false);
            if name_hit || path_hit {
                let entry = by_project.entry(name.clone()).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += memory_mb;
            }
        }
    }

    let mut project_usage: Vec<ProjectUsage> = by_project
        .into_iter()
        .map(|(project, (procs, memory_mb))| ProjectUsage { project, procs, memory_mb })
        .collect();
    project_usage.sort_by(|a, b| a.project.cmp(&b.project));

    let families = by_family
        .into_iter()
        .map(|(family, (procs, memory_mb))| FamilyUsage { family, procs, memory_mb })
        .collect();

    DetectionSnapshot {
        state: DetectionState::Available,
        agent_count,
        total_memory_mb,
        projects: project_usage,
        families,
    }
}

/// Evaluate every `project` against its effective limit, then append any
/// measured project that was not in the requested list so no observation is
/// hidden.
pub fn evaluate(
    projects: &[String],
    limits: &LimitsFile,
    defaults: &ProjectLimitsConfig,
    snapshot: &DetectionSnapshot,
) -> Vec<LimitRow> {
    let mut seen: Vec<String> = Vec::new();
    let mut rows: Vec<LimitRow> = Vec::new();

    let push_row = |project: &str, rows: &mut Vec<LimitRow>, seen: &mut Vec<String>| {
        if seen.iter().any(|s| s == project) {
            return;
        }
        seen.push(project.to_string());
        let (limit, source) = resolve(limits, project, defaults);
        let usage = snapshot.projects.iter().find(|u| u.project == project).cloned();
        let verdict = match (&usage, snapshot.state) {
            (_, DetectionState::Unavailable) => LimitVerdict::NoData,
            (Some(u), DetectionState::Available) => {
                if limit.over_memory_mb(u.memory_mb) > 0 || limit.over_processes(u.procs) > 0 {
                    LimitVerdict::Over
                } else {
                    LimitVerdict::Ok
                }
            }
            (None, DetectionState::Available) => LimitVerdict::NoData,
        };
        rows.push(LimitRow { project: project.to_string(), limit, source, usage, verdict });
    };

    for project in projects {
        push_row(project, &mut rows, &mut seen);
    }
    for usage in &snapshot.projects {
        push_row(&usage.project, &mut rows, &mut seen);
    }
    rows
}

/// Suggested limit derived from a measurement: next 256 MB step (min 256) and
/// the observed process count. Every number is traceable to the measurement.
pub fn suggested_entry(memory_mb: u64, procs: usize) -> LimitEntry {
    let step = 256u64;
    let rounded = ((memory_mb + step - 1) / step) * step;
    LimitEntry { memory_limit_mb: rounded.max(step), max_processes: procs.max(1) }
}

/// Actionable suggestions derived only from the measurement and the limit
/// state. An empty vec means "nothing actionable".
pub fn suggestions(
    limits: &LimitsFile,
    defaults: &ProjectLimitsConfig,
    snapshot: &DetectionSnapshot,
    rows: &[LimitRow],
) -> Vec<String> {
    let mut out = Vec::new();

    for row in rows.iter().filter(|r| r.verdict == LimitVerdict::Over) {
        let Some(usage) = row.usage.as_ref() else { continue };
        if row.over_memory_mb() > 0 {
            out.push(format!(
                "project {} is over its memory limit by {} MB (detected {} procs, {} MB > {} MB)",
                row.project,
                row.over_memory_mb(),
                usage.procs,
                usage.memory_mb,
                row.limit.memory_limit_mb
            ));
        }
        if row.over_processes() > 0 {
            out.push(format!(
                "project {} is over its process limit by {} (detected {} procs > {} max)",
                row.project,
                row.over_processes(),
                usage.procs,
                row.limit.max_processes
            ));
        }
    }

    for family in &snapshot.families {
        let persisted = limits.projects.contains_key(&family.family);
        if persisted || family.procs == 0 {
            continue;
        }
        if family.procs < defaults.max_processes {
            continue;
        }
        let entry = suggested_entry(family.memory_mb, family.procs);
        out.push(format!(
            "family {} has no persisted limit and {} procs ({} MB, at/over the {} proc default); \
             suggest: sharecli limits set {} --mem-mb {} --max-procs {}",
            family.family,
            family.procs,
            family.memory_mb,
            defaults.max_processes,
            family.family,
            entry.memory_limit_mb,
            entry.max_processes
        ));
    }

    if limits.global.is_none() && snapshot.agent_count > 0 {
        let entry = suggested_entry(snapshot.total_memory_mb, snapshot.agent_count);
        out.push(format!(
            "no global limit persisted; {} detected agents using {} MB total; \
             suggest: sharecli limits set --global --mem-mb {} --max-procs {}",
            snapshot.agent_count, snapshot.total_memory_mb, entry.memory_limit_mb, entry.max_processes
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ProjectLimitsConfig {
        ProjectLimitsConfig { memory_limit_mb: 1024, max_processes: 10 }
    }

    fn file_with_global(mem: u64, procs: usize) -> LimitsFile {
        LimitsFile {
            version: 1,
            global: Some(LimitEntry { memory_limit_mb: mem, max_processes: procs }),
            projects: BTreeMap::new(),
        }
    }

    #[test]
    fn round_trip_survives_a_new_store_instance() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("limits.json");
        let store = LimitsStore::at(&path);
        let mut file = LimitsFile::default();
        file.global = Some(LimitEntry { memory_limit_mb: 512, max_processes: 4 });
        file.projects
            .insert("alpha".into(), LimitEntry { memory_limit_mb: 256, max_processes: 2 });
        store.save(&file).expect("save");

        // A brand-new handle (same as a later process) reads the same values.
        let reloaded = LimitsStore::at(&path).load().expect("load");
        assert_eq!(reloaded, file);
        let (effective, source) = store.effective(&reloaded, "alpha", &defaults());
        assert_eq!(effective.memory_limit_mb, 256);
        assert_eq!(source, LimitSource::ProjectOverride);
        let (global, source) = store.effective(&reloaded, "beta", &defaults());
        assert_eq!(global.max_processes, 4);
        assert_eq!(source, LimitSource::GlobalOverride);
    }

    #[test]
    fn missing_file_is_empty_document_and_defaults_apply() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = LimitsStore::at(dir.path().join("nope.json"));
        let file = store.load().expect("load missing");
        assert_eq!(file, LimitsFile::default());
        let (limit, source) = store.effective(&file, "x", &defaults());
        assert_eq!(limit.memory_limit_mb, 1024);
        assert_eq!(source, LimitSource::Default);
    }

    fn snapshot_with(projects: Vec<ProjectUsage>) -> DetectionSnapshot {
        DetectionSnapshot {
            state: DetectionState::Available,
            agent_count: projects.iter().map(|p| p.procs).sum(),
            total_memory_mb: projects.iter().map(|p| p.memory_mb).sum(),
            projects,
            families: Vec::new(),
        }
    }

    #[test]
    fn verdict_ok_when_within_limit() {
        let file = file_with_global(1024, 10);
        let snapshot = snapshot_with(vec![ProjectUsage {
            project: "alpha".into(),
            procs: 3,
            memory_mb: 400,
        }]);
        let rows = evaluate(&["alpha".into()], &file, &defaults(), &snapshot);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].verdict, LimitVerdict::Ok);
        assert_eq!(rows[0].over_memory_mb(), 0);
    }

    #[test]
    fn verdict_over_on_each_axis() {
        let file = file_with_global(512, 4);
        let snapshot = snapshot_with(vec![ProjectUsage {
            project: "alpha".into(),
            procs: 9,
            memory_mb: 900,
        }]);
        let rows = evaluate(&["alpha".into()], &file, &defaults(), &snapshot);
        assert_eq!(rows[0].verdict, LimitVerdict::Over);
        assert_eq!(rows[0].over_memory_mb(), 388);
        assert_eq!(rows[0].over_processes(), 5);
    }

    #[test]
    fn verdict_nodata_when_unmeasured_or_detection_unavailable() {
        let file = file_with_global(512, 4);
        let empty = snapshot_with(Vec::new());
        let rows = evaluate(&["alpha".into()], &file, &defaults(), &empty);
        assert_eq!(rows[0].verdict, LimitVerdict::NoData);
        assert!(rows[0].usage.is_none());

        let unavailable = DetectionSnapshot::unavailable();
        let rows = evaluate(&["alpha".into()], &file, &defaults(), &unavailable);
        assert_eq!(rows[0].verdict, LimitVerdict::NoData);
    }

    #[test]
    fn measured_projects_outside_the_requested_list_are_appended() {
        let file = LimitsFile::default();
        let snapshot = snapshot_with(vec![ProjectUsage {
            project: "zeta".into(),
            procs: 1,
            memory_mb: 10,
        }]);
        let rows = evaluate(&["alpha".into()], &file, &defaults(), &snapshot);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].project, "zeta");
        assert_eq!(rows[1].verdict, LimitVerdict::Ok);
    }

    #[test]
    fn suggestions_empty_on_quiet_state() {
        let file = LimitsFile::default();
        let snapshot = DetectionSnapshot {
            state: DetectionState::Available,
            agent_count: 0,
            total_memory_mb: 0,
            projects: Vec::new(),
            families: Vec::new(),
        };
        let rows = evaluate(&["alpha".into()], &file, &defaults(), &snapshot);
        assert!(suggestions(&file, &defaults(), &snapshot, &rows).is_empty());
    }

    #[test]
    fn suggestions_report_a_measured_overage() {
        let file = file_with_global(512, 4);
        let snapshot = snapshot_with(vec![ProjectUsage {
            project: "alpha".into(),
            procs: 9,
            memory_mb: 900,
        }]);
        let rows = evaluate(&["alpha".into()], &file, &defaults(), &snapshot);
        let got = suggestions(&file, &defaults(), &snapshot, &rows);
        assert!(got.iter().any(|s| s.contains("over its memory limit by 388 MB")));
        assert!(got.iter().any(|s| s.contains("over its process limit by 5")));
        // The suggestion quotes the measurement it came from.
        assert!(got.iter().any(|s| s.contains("detected 9 procs, 900 MB")));
    }
}
