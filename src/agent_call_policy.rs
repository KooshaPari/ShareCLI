//! Pure admission policy for agent-issued commands.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_DEADLINE: Duration = Duration::from_secs(30);

/// The reason an agent call must wait before it can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseCode {
    /// The command would search a host-level or otherwise unsafe root.
    HazardousRoot,
    /// The configured per-project call limit has been reached.
    ProjectLimit,
    /// The host has no thermal headroom for a new call.
    Thermal,
    /// The configured build-command slot limit has been reached.
    BuildSlot,
}

/// An admitted command or a structured pause instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCallDecision {
    command: String,
    pause_code: Option<PauseCode>,
    resume_condition: Option<String>,
    deadline: Duration,
}

impl AgentCallDecision {
    /// The command after policy normalization.
    pub fn command(&self) -> &str {
        &self.command
    }

    /// The reason the command is paused, when admission was refused.
    pub fn pause_code(&self) -> Option<PauseCode> {
        self.pause_code
    }

    /// A human-readable condition that permits retrying a paused command.
    pub fn resume_condition(&self) -> Option<&str> {
        self.resume_condition.as_deref()
    }

    /// The bounded execution deadline for this decision.
    pub fn deadline(&self) -> Duration {
        self.deadline
    }
}

/// Deterministic, local-only admission policy for agent calls.
#[derive(Debug)]
pub struct AgentCallPolicy {
    project_root: PathBuf,
    project_limit: usize,
    admitted_calls: Cell<usize>,
    thermal_headroom: bool,
    build_slots: usize,
    admitted_builds: Cell<usize>,
}

impl AgentCallPolicy {
    /// Create a policy scoped to `project_root` with unrestricted local limits.
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            project_limit: usize::MAX,
            admitted_calls: Cell::new(0),
            thermal_headroom: true,
            build_slots: usize::MAX,
            admitted_builds: Cell::new(0),
        }
    }

    /// Set the maximum number of admitted calls for this project.
    pub fn with_project_limit(mut self, limit: usize) -> Self {
        self.project_limit = limit;
        self
    }

    /// Set whether the host has headroom for another call.
    pub fn with_thermal_headroom(mut self, available: bool) -> Self {
        self.thermal_headroom = available;
        self
    }

    /// Set the number of available build-command slots.
    pub fn with_build_slots(mut self, slots: usize) -> Self {
        self.build_slots = slots;
        self
    }

    /// Normalize and admit a command, or return a pause decision.
    pub fn admit(&self, command: &str) -> AgentCallDecision {
        let normalized = self.normalize(command);

        if targets_hazardous_root(&normalized) {
            return self.paused(
                normalized,
                PauseCode::HazardousRoot,
                "use a path inside the project root",
            );
        }
        if !self.thermal_headroom {
            return self.paused(normalized, PauseCode::Thermal, "wait for thermal headroom");
        }
        if self.admitted_calls.get() >= self.project_limit {
            return self.paused(
                normalized,
                PauseCode::ProjectLimit,
                "wait for an active project call to finish",
            );
        }

        let build = is_build_command(&normalized);
        if build && self.admitted_builds.get() >= self.build_slots {
            return self.paused(
                normalized,
                PauseCode::BuildSlot,
                "wait for an available build slot",
            );
        }

        self.admitted_calls.set(self.admitted_calls.get().saturating_add(1));
        if build {
            self.admitted_builds.set(self.admitted_builds.get().saturating_add(1));
        }

        AgentCallDecision {
            command: normalized,
            pause_code: None,
            resume_condition: None,
            deadline: DEFAULT_DEADLINE,
        }
    }

    fn normalize(&self, command: &str) -> String {
        let words: Vec<_> = command.split_whitespace().collect();
        let Some(program) = words.first() else {
            return command.to_owned();
        };

        if !matches!(*program, "grep" | "egrep") || !has_recursive_flag(&words[1..]) {
            return command.to_owned();
        }

        let mut positional = words[1..].iter().copied().filter(|word| !word.starts_with('-'));
        let pattern = positional.next().unwrap_or("");
        let target = match positional.next() {
            Some(".") | None => self.project_root.as_path(),
            Some(target) => Path::new(target),
        };
        format!(
            "rg --hidden --glob '!target' --glob '!node_modules' {pattern} {}",
            target.display()
        )
    }

    fn paused(
        &self,
        command: String,
        pause_code: PauseCode,
        resume_condition: &str,
    ) -> AgentCallDecision {
        AgentCallDecision {
            command,
            pause_code: Some(pause_code),
            resume_condition: Some(resume_condition.to_owned()),
            deadline: DEFAULT_DEADLINE,
        }
    }
}

fn has_recursive_flag(words: &[&str]) -> bool {
    words.iter().any(|word| {
        *word == "--recursive"
            || word.starts_with('-') && word[1..].chars().any(|flag| matches!(flag, 'r' | 'R'))
    })
}

fn is_build_command(command: &str) -> bool {
    matches!(command.split_whitespace().next(), Some("cargo" | "make" | "just"))
}

fn targets_hazardous_root(command: &str) -> bool {
    command.split_whitespace().any(|word| is_hazardous_root(Path::new(word)))
}

fn is_hazardous_root(path: &Path) -> bool {
    const ROOTS: &[&str] = &[
        "/",
        "/Applications",
        "/Library",
        "/System",
        "/Users",
        "/bin",
        "/dev",
        "/etc",
        "/opt",
        "/private",
        "/proc",
        "/sys",
        "/tmp",
        "/usr",
        "/var",
        "/Volumes",
    ];
    ROOTS.iter().any(|root| path == Path::new(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AgentCallPolicy {
        AgentCallPolicy::new(PathBuf::from("/work/proj"))
    }

    #[test]
    fn admits_plain_command_with_default_deadline() {
        let decision = policy().admit("ls -la");
        assert_eq!(decision.command(), "ls -la");
        assert_eq!(decision.pause_code(), None);
        assert_eq!(decision.resume_condition(), None);
        assert_eq!(decision.deadline(), Duration::from_secs(30));
    }

    #[test]
    fn empty_command_is_admitted_without_normalization() {
        let decision = policy().admit("");
        assert_eq!(decision.command(), "");
        assert_eq!(decision.pause_code(), None);
    }

    #[test]
    fn rewrites_recursive_grep_dot_to_project_root() {
        let decision = policy().admit("grep -r needle .");
        assert_eq!(
            decision.command(),
            "rg --hidden --glob '!target' --glob '!node_modules' needle /work/proj"
        );
        assert_eq!(decision.pause_code(), None);
    }

    #[test]
    fn rewrites_recursive_grep_without_target_to_project_root() {
        let decision = policy().admit("grep -R needle");
        assert_eq!(
            decision.command(),
            "rg --hidden --glob '!target' --glob '!node_modules' needle /work/proj"
        );
    }

    #[test]
    fn uses_explicit_grep_target_when_it_is_a_directory() {
        let decision = policy().admit("egrep -rn pattern src/lib");
        assert_eq!(
            decision.command(),
            "rg --hidden --glob '!target' --glob '!node_modules' pattern src/lib"
        );
    }

    #[test]
    fn leaves_non_recursive_grep_untouched() {
        let decision = policy().admit("grep needle .");
        assert_eq!(decision.command(), "grep needle .");
    }

    #[test]
    fn leaves_recursive_non_grep_untouched() {
        let decision = policy().admit("rm -r build");
        assert_eq!(decision.command(), "rm -r build");
    }

    #[test]
    fn recursive_flag_detection_accepts_long_and_bundled_flags() {
        assert!(has_recursive_flag(&["--recursive"]));
        assert!(has_recursive_flag(&["-r"]));
        assert!(has_recursive_flag(&["-R"]));
        assert!(has_recursive_flag(&["-rn"]));
        assert!(!has_recursive_flag(&["-l"]));
        assert!(!has_recursive_flag(&["-"]));
        assert!(!has_recursive_flag(&[]));
    }

    #[test]
    fn hazardous_root_is_paused_with_resume_condition() {
        let decision = policy().admit("rm -rf /");
        assert_eq!(decision.pause_code(), Some(PauseCode::HazardousRoot));
        assert_eq!(decision.resume_condition(), Some("use a path inside the project root"));
        assert_eq!(decision.deadline(), DEFAULT_DEADLINE);
    }

    #[test]
    fn hazardous_root_check_applies_to_any_argument() {
        assert!(targets_hazardous_root("ls /etc"));
        assert!(targets_hazardous_root("find /usr"));
        assert!(!targets_hazardous_root("ls /usr/local"));
        assert!(!targets_hazardous_root("ls /work/proj"));
        // A root with a trailing separator is still the same path.
        assert!(targets_hazardous_root("ls /var/"));
    }

    #[test]
    fn every_published_root_is_recognized() {
        for root in [
            "/",
            "/Applications",
            "/Library",
            "/System",
            "/Users",
            "/bin",
            "/dev",
            "/etc",
            "/opt",
            "/private",
            "/proc",
            "/sys",
            "/tmp",
            "/usr",
            "/var",
            "/Volumes",
        ] {
            assert!(is_hazardous_root(Path::new(root)), "{root} must be hazardous");
        }
        assert!(!is_hazardous_root(Path::new("/work/proj")));
        assert!(!is_hazardous_root(Path::new("relative/path")));
    }

    #[test]
    fn missing_thermal_headroom_pauses_before_limits() {
        let policy = policy().with_thermal_headroom(false);
        let decision = policy.admit("ls");
        assert_eq!(decision.pause_code(), Some(PauseCode::Thermal));
        assert_eq!(decision.resume_condition(), Some("wait for thermal headroom"));
    }

    #[test]
    fn project_limit_is_enforced_once_reached() {
        let policy = policy().with_project_limit(1);
        assert_eq!(policy.admit("ls").pause_code(), None);
        let refused = policy.admit("ls");
        assert_eq!(refused.pause_code(), Some(PauseCode::ProjectLimit));
        assert_eq!(refused.resume_condition(), Some("wait for an active project call to finish"));
    }

    #[test]
    fn build_slots_are_enforced_for_build_commands_only() {
        let policy = policy().with_build_slots(1);
        assert_eq!(policy.admit("cargo build").pause_code(), None);
        let refused = policy.admit("make all");
        assert_eq!(refused.pause_code(), Some(PauseCode::BuildSlot));
        // A non-build command is not blocked by the exhausted build budget.
        assert_eq!(policy.admit("ls").pause_code(), None);
    }

    #[test]
    fn build_command_detection_covers_known_tools() {
        assert!(is_build_command("cargo test"));
        assert!(is_build_command("make"));
        assert!(is_build_command("just lint"));
        assert!(!is_build_command("npm test"));
        assert!(!is_build_command(""));
        assert!(!is_build_command("  "));
    }

    #[test]
    fn hazardous_check_runs_before_project_limit() {
        let policy = policy().with_project_limit(0);
        let decision = policy.admit("ls /");
        assert_eq!(decision.pause_code(), Some(PauseCode::HazardousRoot));
    }

    #[test]
    fn admitted_counts_saturate_at_project_limit() {
        let policy = policy().with_project_limit(2);
        assert_eq!(policy.admit("ls").pause_code(), None);
        assert_eq!(policy.admit("ls").pause_code(), None);
        assert_eq!(policy.admit("ls").pause_code(), Some(PauseCode::ProjectLimit));
        // The refused call must not have consumed budget on a retry path either.
        assert_eq!(policy.admitted_calls.get(), 2);
    }

    #[test]
    fn normalized_recursive_grep_still_honors_project_limit() {
        let policy = policy().with_project_limit(1);
        assert_eq!(policy.admit("grep -r needle .").pause_code(), None);
        assert_eq!(policy.admit("grep -r needle .").pause_code(), Some(PauseCode::ProjectLimit));
    }
}
