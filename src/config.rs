//! Configuration management for sharecli
//!
//! All configurable parameters are consolidated here. Hardcoded defaults
//! serve as fallbacks when no config file is present; users override via
//! `~/.config/sharecli/config.toml`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Registered projects (name → path)
    pub projects: HashMap<String, String>,

    /// Runtime settings (executable paths, resource caps)
    pub runtime: RuntimeConfig,

    /// Default harness settings (per harness type)
    pub defaults: HashMap<String, DefaultHarnessConfig>,

    /// Shared process pool settings
    pub pool: PoolConfig,

    /// Monitoring / health-check thresholds
    pub monitoring: MonitoringConfig,

    /// Port assignments for co-processes
    pub port: PortConfig,

    /// Default paths for discovery, output, etc.
    pub paths: PathsConfig,

    /// Default project resource limits
    pub project_limits: ProjectLimitsConfig,

    /// Spawn / timing parameters
    pub spawn: SpawnConfig,

    /// Build-contention throttle policy
    pub spawn_policy: SpawnPolicyConfig,

    /// Cross-machine text-injection (`cast`) settings
    pub cast: CastConfig,

    /// Per-process health-check schedules (process name → config).
    /// Each entry spawns a background poller when `sharecli serve` runs.
    pub health_checks: HashMap<String, crate::health_check::HealthCheckConfig>,

    /// Notification channels (desktop + webhooks).
    pub notifications: crate::notifier::NotifierConfig,

    /// HTTP serve AuthN / bind policy.
    pub serve: ServeConfig,

    /// Optional override for the agent patterns TOML path.
    /// Default: `None` (uses `~/.config/sharecli/agent_patterns.toml`).
    pub agent_patterns_path: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            projects: default_projects(),
            runtime: RuntimeConfig::default(),
            defaults: default_harness_configs(),
            pool: PoolConfig::default(),
            monitoring: MonitoringConfig::default(),
            port: PortConfig::default(),
            paths: PathsConfig::default(),
            project_limits: ProjectLimitsConfig::default(),
            spawn: SpawnConfig::default(),
            spawn_policy: SpawnPolicyConfig::default(),
            cast: CastConfig::default(),
            health_checks: HashMap::new(),
            notifications: crate::notifier::NotifierConfig::default(),
            serve: ServeConfig::default(),
            agent_patterns_path: None,
        }
    }
}

/// `sharecli serve` AuthN settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ServeConfig {
    /// Optional Bearer token. Prefer env `SHARECLI_SERVE_TOKEN` in production.
    /// When empty/absent, serve stays open (loopback trust model).
    pub bearer_token: Option<String>,
    /// Auth mode: `open` | `bearer` | `jwt`. When unset: bearer_token ⇒ bearer,
    /// else `[serve.jwt]` ⇒ jwt, else open. Env `SHARECLI_SERVE_AUTH_MODE` overrides.
    pub auth_mode: Option<String>,
    /// Federated JWT (OAuth2 resource-server) settings for `auth_mode = "jwt"`.
    pub jwt: Option<ServeJwtConfig>,
    /// Max HTTP requests per sliding window (0 or unset = disabled). Env
    /// `SHARECLI_SERVE_RATE_LIMIT_MAX` overrides.
    pub rate_limit_max: Option<usize>,
    /// Sliding window length in seconds (default 60). Env
    /// `SHARECLI_SERVE_RATE_LIMIT_WINDOW_SECS` overrides.
    pub rate_limit_window_secs: Option<u64>,
}

/// JWT resource-server config for `sharecli serve` (FR-012 / W5.1).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ServeJwtConfig {
    /// Expected `iss` claim (e.g. `https://login.microsoftonline.com/{tenant}/v2.0`).
    pub issuer: String,
    /// Expected `aud` claim (API audience / client id).
    pub audience: String,
    /// Path to a JWKS JSON document (`{"keys":[...]}`).
    pub jwks_path: Option<String>,
    /// Inline JWKS JSON (tests / air-gapped). Prefer `jwks_path` in production.
    pub jwks: Option<String>,
}

// ---------------------------------------------------------------------------
// Sub-configs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Path to node executable
    pub node_path: Option<String>,
    /// Path to bun executable
    pub bun_path: Option<String>,
    /// Maximum memory per process (MB)
    pub max_memory_mb: Option<u64>,
    /// Maximum number of processes
    pub max_processes: Option<usize>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            node_path: None,
            bun_path: None,
            max_memory_mb: Some(4096),
            max_processes: Some(100),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Enable shared process pool
    pub enabled: bool,
    /// Max pooled processes per harness type (node, bun)
    pub max_per_type: usize,
    /// Idle timeout before a pooled process is eligible for recycling (seconds)
    pub idle_timeout_secs: u64,
    /// Max age before a pooled process is force-recycled (seconds)
    pub max_age_secs: u64,
    /// Delay between spawn and health check (milliseconds)
    pub spawn_delay_ms: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_per_type: 5,
            idle_timeout_secs: 300,
            max_age_secs: 3600,
            spawn_delay_ms: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Interval between health checks (seconds)
    pub health_check_interval_secs: u64,
    /// Seconds of inactivity before a process is considered idle
    pub idle_threshold_secs: u64,
    /// Memory threshold above which a warning is emitted (MB)
    pub high_memory_threshold_mb: u64,
    /// Number of idle processes that triggers a pruning recommendation
    pub idle_process_threshold: usize,
    /// Per-process memory limit for health-check warnings (bytes)
    pub per_process_warn_memory_bytes: u64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            health_check_interval_secs: 30,
            idle_threshold_secs: 300,
            high_memory_threshold_mb: 4096,
            idle_process_threshold: 5,
            per_process_warn_memory_bytes: 1024 * 1024 * 1024, // 1 GiB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    /// Port for the ShareWei co-process
    pub sharewei_port: u16,
}

impl Default for PortConfig {
    fn default() -> Self {
        Self { sharewei_port: 3100 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Default directory to scan when `project discover` has no argument
    pub discovery_path: String,
    /// Default output path for `project generate`
    pub default_compose_output: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            discovery_path: "~/CodeProjects/Phenotype/repos".into(),
            default_compose_output: "process-compose.yml".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultHarnessConfig {
    pub enabled: bool,
    pub max_instances: usize,
    pub memory_limit_mb: u64,
}

impl Default for DefaultHarnessConfig {
    fn default() -> Self {
        Self { enabled: true, max_instances: 10, memory_limit_mb: 256 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLimitsConfig {
    /// Default memory limit per project (MB)
    pub memory_limit_mb: u64,
    /// Default max processes per project
    pub max_processes: usize,
}

impl Default for ProjectLimitsConfig {
    fn default() -> Self {
        Self { memory_limit_mb: 1024, max_processes: 10 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnConfig {
    /// Default harness type when none is specified
    pub default_harness: String,
    /// Default idle threshold for prune command (seconds)
    pub prune_idle_seconds: u64,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self { default_harness: "claude".into(), prune_idle_seconds: 300 }
    }
}

// ---------------------------------------------------------------------------
// Spawn-policy / build-contention throttle
// ---------------------------------------------------------------------------

/// Controls how sharecli-managed build harnesses (cargo/rustc) compete for CPU.
///
/// All settings are **opt-in**: defaults are conservative and safe. The policy
/// only affects processes that sharecli itself spawns — it never touches the
/// operator's existing sessions.
///
/// Add to `~/.config/sharecli/config.toml`:
///
/// ```toml
/// [spawn_policy]
/// nice_level = 10           # 0 = disabled; >0 = apply background QoS (macOS: taskpolicy -b)
/// max_concurrent_builds = 2 # semaphore cap across all sharecli build spawns
/// use_sccache = false       # set RUSTC_WRAPPER=sccache when sccache is on PATH
/// ```
///
/// Teardown: the semaphore is in-process only. When sharecli exits all permits
/// are released automatically; no persistent state is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpawnPolicyConfig {
    /// nice/priority level applied to build harness processes via `taskpolicy -b`
    /// on macOS. Set to 0 to disable background-QoS wrapping entirely.
    pub nice_level: u8,
    /// Maximum number of cargo/rustc build harnesses that may execute
    /// concurrently under sharecli. Additional spawns queue until a slot is free.
    pub max_concurrent_builds: usize,
    /// When `true` and `sccache` is found on PATH, set `RUSTC_WRAPPER=sccache`
    /// for every build harness sharecli spawns.
    pub use_sccache: bool,
}

impl Default for SpawnPolicyConfig {
    fn default() -> Self {
        Self { nice_level: 10, max_concurrent_builds: 2, use_sccache: false }
    }
}

// ---------------------------------------------------------------------------
// Default projects (machine-local — overridden by config file)
// ---------------------------------------------------------------------------

fn default_projects() -> HashMap<String, String> {
    let mut projects = HashMap::new();
    projects
        .insert("helios-cli".to_string(), "~/CodeProjects/Phenotype/repos/helios-cli".to_string());
    projects.insert("portage".to_string(), "~/CodeProjects/Phenotype/repos/portage".to_string());
    projects.insert(
        "agentapi".to_string(),
        "~/CodeProjects/Phenotype/repos/agentapi-plusplus".to_string(),
    );
    projects.insert(
        "cliproxy".to_string(),
        "~/CodeProjects/Phenotype/repos/cliproxyapi-plusplus".to_string(),
    );
    projects.insert("colab".to_string(), "~/CodeProjects/Phenotype/repos/colab".to_string());
    projects
}

fn default_harness_configs() -> HashMap<String, DefaultHarnessConfig> {
    let mut m = HashMap::new();
    m.insert(
        "claude".into(),
        DefaultHarnessConfig { enabled: true, max_instances: 11, memory_limit_mb: 512 },
    );
    m.insert(
        "forge".into(),
        DefaultHarnessConfig { enabled: true, max_instances: 20, memory_limit_mb: 256 },
    );
    m.insert(
        "node".into(),
        DefaultHarnessConfig { enabled: true, max_instances: 30, memory_limit_mb: 256 },
    );
    m.insert(
        "bun".into(),
        DefaultHarnessConfig { enabled: true, max_instances: 10, memory_limit_mb: 384 },
    );
    m
}

// ---------------------------------------------------------------------------
// Global config singleton
// ---------------------------------------------------------------------------

static GLOBAL_CONFIG: OnceLock<Config> = OnceLock::new();

/// Initialise the global config from the default config file path.
/// Safe to call multiple times — only the first call takes effect.
pub fn init_global() -> &'static Config {
    GLOBAL_CONFIG.get_or_init(|| Config::load().unwrap_or_default())
}

/// Return a reference to the global config (panics if not initialised).
pub fn global() -> &'static Config {
    GLOBAL_CONFIG.get().expect("Config not initialised — call config::init_global() first")
}

// ---------------------------------------------------------------------------
// File-based loading / saving
// ---------------------------------------------------------------------------

impl Config {
    /// Load configuration from `~/.config/sharecli/config.toml`
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Initialize default configuration file
    pub fn init() -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config = Config::default();
        let contents = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, contents)?;

        Ok(())
    }

    /// Save configuration
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)?;

        Ok(())
    }

    /// Get config file path
    fn config_path() -> Result<PathBuf> {
        let base =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        Ok(base.join("sharecli").join("config.toml"))
    }
}

// ---------------------------------------------------------------------------
// CLI command enums (defined here to avoid circular dependencies)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Cast configuration
// ---------------------------------------------------------------------------

/// Settings for the `cast` subcommand (cross-machine text injection into
/// registered terminal panes).
///
/// Add to `~/.config/sharecli/config.toml`:
///
/// ```toml
/// [cast]
/// default_transport = "wezterm"   # or "ghostty" / "clipboard"
/// pane_map_path = "~/.config/sharecli/pane-map.toml"
/// handshake_timeout_ms = 250
/// max_retry_attempts = 3
/// retry_backoff_ms = 200
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CastConfig {
    /// Default transport to use when `cast send` is invoked.
    /// One of: `wezterm`, `ghostty`, `clipboard`.
    pub default_transport: String,
    /// Override the on-disk path for the pane registry. When `None`, defaults
    /// to `<config_dir>/sharecli/pane-map.toml`.
    pub pane_map_path: Option<String>,
    /// Time (ms) to wait for an OSC 9;4 echo confirmation before declaring
    /// the send failed and triggering a retry.
    pub handshake_timeout_ms: u64,
    /// Max retries for transient send failures.
    pub max_retry_attempts: u32,
    /// Initial backoff (ms) between retry attempts (doubles each retry).
    pub retry_backoff_ms: u64,
}

impl Default for CastConfig {
    fn default() -> Self {
        Self {
            default_transport: "wezterm".into(),
            pane_map_path: None,
            handshake_timeout_ms: 250,
            max_retry_attempts: 3,
            retry_backoff_ms: 200,
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ConfigCmd {
    /// Initialize default configuration
    Init,
    /// Validate configuration
    Validate,
    /// Show current configuration
    Show,
    /// Get a configuration value
    Get { key: String },
    /// Set a configuration value
    Set { key: String, value: String },
}

#[derive(clap::Subcommand, Debug)]
pub enum ProjectCmd {
    /// Add a project to the registry
    Add { name: String, path: String },
    /// Remove a project from the registry
    Remove { name: String },
    /// List all registered projects
    List,
    /// Show project details
    Show { name: String },
    /// Discover projects in a directory
    Discover { path: Option<String> },
    /// Generate process-compose.yml from registered projects
    Generate { output: Option<String> },
    /// Start all stopped processes belonging to a project group
    Start {
        /// Project name
        name: String,
        /// Harness type to start (e.g. cargo, node, bun)
        #[arg(long)]
        harness: Option<String>,
    },
    /// Stop all running processes in a project group
    Stop {
        /// Project name
        name: String,
        /// Force-kill processes instead of graceful stop
        #[arg(long)]
        force: bool,
        /// Confirm destructive force-kill without interactive prompt
        #[arg(long)]
        yes: bool,
    },
    /// Stop then start all processes in a project group
    Restart {
        /// Project name
        name: String,
        /// Harness type to restart (e.g. cargo, node, bun)
        #[arg(long)]
        harness: Option<String>,
        /// Force-kill on stop phase
        #[arg(long)]
        force: bool,
        /// Confirm destructive force-kill on stop phase
        #[arg(long)]
        yes: bool,
    },
    /// Show status table for all processes in a project group
    Status {
        /// Project name
        name: String,
        /// Output machine-readable JSON
        #[arg(long)]
        json: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Default values
    // -----------------------------------------------------------------------

    #[test]
    fn config_default_has_expected_projects() {
        let cfg = Config::default();
        assert!(cfg.projects.contains_key("helios-cli"));
        assert!(cfg.projects.contains_key("portage"));
        assert!(cfg.projects.contains_key("agentapi"));
        assert!(cfg.projects.contains_key("cliproxy"));
        assert!(cfg.projects.contains_key("colab"));
        assert_eq!(cfg.projects.len(), 5);
    }

    #[test]
    fn runtime_config_defaults() {
        let rt = RuntimeConfig::default();
        assert_eq!(rt.node_path, None);
        assert_eq!(rt.bun_path, None);
        assert_eq!(rt.max_memory_mb, Some(4096));
        assert_eq!(rt.max_processes, Some(100));
    }

    #[test]
    fn pool_config_defaults() {
        let pool = PoolConfig::default();
        assert!(pool.enabled);
        assert_eq!(pool.max_per_type, 5);
        assert_eq!(pool.idle_timeout_secs, 300);
        assert_eq!(pool.max_age_secs, 3600);
        assert_eq!(pool.spawn_delay_ms, 100);
    }

    #[test]
    fn monitoring_config_defaults() {
        let m = MonitoringConfig::default();
        assert_eq!(m.health_check_interval_secs, 30);
        assert_eq!(m.idle_threshold_secs, 300);
        assert_eq!(m.high_memory_threshold_mb, 4096);
        assert_eq!(m.idle_process_threshold, 5);
        assert_eq!(m.per_process_warn_memory_bytes, 1024 * 1024 * 1024);
    }

    #[test]
    fn cast_config_defaults() {
        let c = CastConfig::default();
        assert_eq!(c.default_transport, "wezterm");
        assert_eq!(c.pane_map_path, None);
        assert_eq!(c.handshake_timeout_ms, 250);
        assert_eq!(c.max_retry_attempts, 3);
        assert_eq!(c.retry_backoff_ms, 200);
    }

    #[test]
    fn spawn_policy_config_defaults() {
        let s = SpawnPolicyConfig::default();
        assert_eq!(s.nice_level, 10);
        assert_eq!(s.max_concurrent_builds, 2);
        assert!(!s.use_sccache);
    }

    #[test]
    fn serve_config_defaults() {
        let s = ServeConfig::default();
        assert_eq!(s.bearer_token, None);
        assert_eq!(s.auth_mode, None);
        assert!(s.jwt.is_none());
        assert_eq!(s.rate_limit_max, None);
        assert_eq!(s.rate_limit_window_secs, None);
    }

    #[test]
    fn default_harness_config_values() {
        let c = DefaultHarnessConfig::default();
        assert!(c.enabled);
        assert_eq!(c.max_instances, 10);
        assert_eq!(c.memory_limit_mb, 256);
    }

    #[test]
    fn project_limits_config_defaults() {
        let c = ProjectLimitsConfig::default();
        assert_eq!(c.memory_limit_mb, 1024);
        assert_eq!(c.max_processes, 10);
    }

    #[test]
    fn spawn_config_defaults() {
        let c = SpawnConfig::default();
        assert_eq!(c.default_harness, "claude");
        assert_eq!(c.prune_idle_seconds, 300);
    }

    #[test]
    fn paths_config_defaults() {
        let c = PathsConfig::default();
        assert_eq!(c.discovery_path, "~/CodeProjects/Phenotype/repos");
        assert_eq!(c.default_compose_output, "process-compose.yml");
    }

    // -----------------------------------------------------------------------
    // TOML parsing
    // -----------------------------------------------------------------------

    #[test]
    fn config_from_full_toml() {
        let toml_str = r#"
            [runtime]
            node_path = "/usr/local/bin/node"
            bun_path = "/usr/local/bin/bun"
            max_memory_mb = 8192
            max_processes = 50

            [pool]
            enabled = false
            max_per_type = 10
            idle_timeout_secs = 600
            max_age_secs = 7200
            spawn_delay_ms = 200

            [monitoring]
            health_check_interval_secs = 60
            idle_threshold_secs = 600
            high_memory_threshold_mb = 8192
            idle_process_threshold = 10
            per_process_warn_memory_bytes = 2147483648

            [port]
            sharewei_port = 4100

            [paths]
            discovery_path = "/tmp/repos"
            default_compose_output = "output.yml"

            [project_limits]
            memory_limit_mb = 2048
            max_processes = 20

            [spawn]
            default_harness = "forge"
            prune_idle_seconds = 600

            [spawn_policy]
            nice_level = 20
            max_concurrent_builds = 4
            use_sccache = true

            [cast]
            default_transport = "ghostty"
            handshake_timeout_ms = 500
            max_retry_attempts = 5
            retry_backoff_ms = 400

            [notifications]
            desktop = false
            webhooks = ["https://example.com/hook"]

            [serve]
            bearer_token = "secret-token"
            auth_mode = "bearer"
            rate_limit_max = 120
            rate_limit_window_secs = 30
        "#;

        let cfg: Config = toml::from_str(toml_str).expect("parse full TOML");

        assert_eq!(cfg.runtime.node_path.as_deref(), Some("/usr/local/bin/node"));
        assert_eq!(cfg.runtime.bun_path.as_deref(), Some("/usr/local/bin/bun"));
        assert_eq!(cfg.runtime.max_memory_mb, Some(8192));
        assert_eq!(cfg.runtime.max_processes, Some(50));

        assert!(!cfg.pool.enabled);
        assert_eq!(cfg.pool.max_per_type, 10);

        assert_eq!(cfg.monitoring.health_check_interval_secs, 60);
        assert_eq!(cfg.monitoring.high_memory_threshold_mb, 8192);

        assert_eq!(cfg.port.sharewei_port, 4100);
        assert_eq!(cfg.paths.discovery_path, "/tmp/repos");
        assert_eq!(cfg.project_limits.memory_limit_mb, 2048);
        assert_eq!(cfg.spawn.default_harness, "forge");
        assert_eq!(cfg.spawn.prune_idle_seconds, 600);

        assert_eq!(cfg.spawn_policy.nice_level, 20);
        assert_eq!(cfg.spawn_policy.max_concurrent_builds, 4);
        assert!(cfg.spawn_policy.use_sccache);

        assert_eq!(cfg.cast.default_transport, "ghostty");
        assert_eq!(cfg.cast.handshake_timeout_ms, 500);
        assert_eq!(cfg.cast.max_retry_attempts, 5);

        assert!(!cfg.notifications.desktop);
        assert_eq!(cfg.notifications.webhooks, vec!["https://example.com/hook"]);

        assert_eq!(cfg.serve.bearer_token.as_deref(), Some("secret-token"));
        assert_eq!(cfg.serve.auth_mode.as_deref(), Some("bearer"));
        assert_eq!(cfg.serve.rate_limit_max, Some(120));
        assert_eq!(cfg.serve.rate_limit_window_secs, Some(30));
    }

    #[test]
    fn config_from_partial_toml_uses_defaults() {
        // Only override port; everything else should fall back to defaults.
        let toml_str = r#"
            [port]
            sharewei_port = 9999
        "#;

        let cfg: Config = toml::from_str(toml_str).expect("parse partial TOML");

        // Port field from the TOML.
        assert_eq!(cfg.port.sharewei_port, 9999);

        // Other sub-configs remain default.
        assert_eq!(cfg.cast.default_transport, "wezterm");
        assert!(cfg.notifications.desktop);
        assert!(cfg.pool.enabled);
        assert_eq!(cfg.pool.max_per_type, 5);
        assert_eq!(cfg.monitoring.health_check_interval_secs, 30);
        assert_eq!(cfg.spawn_policy.nice_level, 10);
    }

    #[test]
    fn config_from_empty_toml_is_default() {
        let cfg: Config = toml::from_str("").expect("parse empty TOML");
        let default = Config::default();

        // Compare field-by-field since Config doesn't derive PartialEq.
        assert_eq!(cfg.pool.enabled, default.pool.enabled);
        assert_eq!(cfg.pool.max_per_type, default.pool.max_per_type);
        assert_eq!(cfg.pool.idle_timeout_secs, default.pool.idle_timeout_secs);
        assert_eq!(cfg.pool.max_age_secs, default.pool.max_age_secs);
        assert_eq!(cfg.pool.spawn_delay_ms, default.pool.spawn_delay_ms);
        assert_eq!(
            cfg.monitoring.health_check_interval_secs,
            default.monitoring.health_check_interval_secs
        );
        assert_eq!(
            cfg.monitoring.high_memory_threshold_mb,
            default.monitoring.high_memory_threshold_mb
        );
        assert_eq!(cfg.port.sharewei_port, default.port.sharewei_port);
        assert_eq!(cfg.cast.default_transport, default.cast.default_transport);
        assert_eq!(cfg.cast.handshake_timeout_ms, default.cast.handshake_timeout_ms);
        assert_eq!(cfg.spawn_policy.nice_level, default.spawn_policy.nice_level);
        assert_eq!(
            cfg.spawn_policy.max_concurrent_builds,
            default.spawn_policy.max_concurrent_builds
        );
    }

    #[test]
    fn config_project_map_override() {
        let toml_str = r#"
            [projects]
            my-project = "/home/user/my-project"
        "#;

        let cfg: Config = toml::from_str(toml_str).expect("parse project override");
        assert_eq!(cfg.projects.len(), 1);
        assert_eq!(
            cfg.projects.get("my-project").map(String::as_str),
            Some("/home/user/my-project")
        );
    }

    // -----------------------------------------------------------------------
    // JSON parsing
    // -----------------------------------------------------------------------

    #[test]
    fn config_from_json() {
        let json_str = r#"{
            "runtime": { "max_memory_mb": 2048 },
            "port": { "sharewei_port": 9090 },
            "cast": { "default_transport": "clipboard" }
        }"#;

        let cfg: Config = serde_json::from_str(json_str).expect("parse JSON");
        assert_eq!(cfg.runtime.max_memory_mb, Some(2048));
        assert_eq!(cfg.port.sharewei_port, 9090);
        assert_eq!(cfg.cast.default_transport, "clipboard");
        // Untouched sub-configs use defaults.
        assert!(cfg.pool.enabled);
        assert_eq!(cfg.pool.max_per_type, 5);
    }

    // -----------------------------------------------------------------------
    // Round-trip (serialize -> deserialize)
    // -----------------------------------------------------------------------

    #[test]
    fn config_round_trip_toml() {
        let original = Config::default();
        let serialized = toml::to_string(&original).expect("serialize to TOML");
        let restored: Config = toml::from_str(&serialized).expect("deserialize round-trip");

        assert_eq!(original.projects.len(), restored.projects.len());
        for (k, v) in &original.projects {
            assert_eq!(restored.projects.get(k).unwrap(), v);
        }
        assert_eq!(original.pool.enabled, restored.pool.enabled);
        assert_eq!(original.pool.max_per_type, restored.pool.max_per_type);
        assert_eq!(original.pool.idle_timeout_secs, restored.pool.idle_timeout_secs);
        assert_eq!(
            original.monitoring.health_check_interval_secs,
            restored.monitoring.health_check_interval_secs
        );
        assert_eq!(
            original.monitoring.high_memory_threshold_mb,
            restored.monitoring.high_memory_threshold_mb
        );
        assert_eq!(original.port.sharewei_port, restored.port.sharewei_port);
        assert_eq!(original.cast.default_transport, restored.cast.default_transport);
        assert_eq!(original.cast.handshake_timeout_ms, restored.cast.handshake_timeout_ms);
        assert_eq!(original.spawn_policy.nice_level, restored.spawn_policy.nice_level);
        assert_eq!(
            original.spawn_policy.max_concurrent_builds,
            restored.spawn_policy.max_concurrent_builds
        );
    }

    // -----------------------------------------------------------------------
    // Error handling
    // -----------------------------------------------------------------------

    #[test]
    fn config_invalid_toml_wrong_type_returns_error() {
        let bad = r#"
            [runtime]
            max_memory_mb = "not a number"
        "#;

        let result = toml::from_str::<Config>(bad);
        assert!(result.is_err(), "expected parse error for wrong type");
    }

    #[test]
    fn config_malformed_toml_returns_error() {
        let bad = r#"
            [unclosed_section
        "#;

        let result = toml::from_str::<Config>(bad);
        assert!(result.is_err(), "expected parse error for malformed TOML");
    }

    #[test]
    fn config_unknown_fields_are_ignored() {
        let toml_str = r#"
            [nonexistent_section]
            foo = "bar"

            [port]
            sharewei_port = 5555
        "#;

        let cfg: Config = toml::from_str(toml_str).expect("unknown fields should be ignored");
        assert_eq!(cfg.port.sharewei_port, 5555);
    }

    // -----------------------------------------------------------------------
    // Sub-config specifics
    // -----------------------------------------------------------------------

    #[test]
    fn serve_jwt_config_from_toml() {
        let toml_str = r#"
            [serve.jwt]
            issuer = "https://login.microsoftonline.com/tenant/v2.0"
            audience = "api://my-api"
            jwks_path = "/etc/jwks.json"
        "#;

        let cfg: Config = toml::from_str(toml_str).expect("parse JWT config");
        let jwt = cfg.serve.jwt.expect("jwt should be present");
        assert_eq!(jwt.issuer, "https://login.microsoftonline.com/tenant/v2.0");
        assert_eq!(jwt.audience, "api://my-api");
        assert_eq!(jwt.jwks_path.as_deref(), Some("/etc/jwks.json"));
    }

    #[test]
    fn health_checks_from_toml() {
        let toml_str = r#"
            [health_checks.sharewei]
            interval_secs = 15
            timeout_secs = 3
            failure_threshold = 5
            endpoints = ["http://localhost:3100/health"]
        "#;

        let cfg: Config = toml::from_str(toml_str).expect("parse health check config");
        let hc = cfg.health_checks.get("sharewei").expect("sharewei health check");
        assert_eq!(hc.interval_secs, 15);
        assert_eq!(hc.timeout_secs, 3);
        assert_eq!(hc.failure_threshold, 5);
        assert_eq!(hc.endpoints, vec!["http://localhost:3100/health"]);
    }

    // -----------------------------------------------------------------------
    // Merge / override behavior
    // -----------------------------------------------------------------------

    #[test]
    fn config_partial_override_preserves_other_defaults() {
        let toml_str = r#"
            [cast]
            default_transport = "clipboard"

            [notifications]
            desktop = false
        "#;

        let cfg: Config = toml::from_str(toml_str).expect("parse partial override");

        // Cast overridden.
        assert_eq!(cfg.cast.default_transport, "clipboard");
        // Cast defaults preserved for non-overridden fields.
        assert_eq!(cfg.cast.handshake_timeout_ms, 250);
        assert_eq!(cfg.cast.max_retry_attempts, 3);

        // Notifications overridden.
        assert!(!cfg.notifications.desktop);
        // Defaults still hold.
        assert!(cfg.notifications.webhooks.is_empty());

        // Completely untouched sub-configs.
        assert_eq!(cfg.port.sharewei_port, 3100);
        assert_eq!(cfg.pool.max_per_type, 5);
        assert_eq!(cfg.spawn_policy.nice_level, 10);
    }

    // -----------------------------------------------------------------------
    // Additional coverage tests
    // -----------------------------------------------------------------------

    #[test]
    fn serve_jwt_config_defaults() {
        let jwt = ServeJwtConfig::default();
        assert!(jwt.issuer.is_empty());
        assert!(jwt.audience.is_empty());
        assert!(jwt.jwks_path.is_none());
        assert!(jwt.jwks.is_none());
    }

    #[test]
    fn config_from_toml_with_agent_patterns_path() {
        let toml_str = r#"
            agent_patterns_path = "/custom/patterns.toml"
        "#;
        let cfg: Config = toml::from_str(toml_str).expect("parse");
        assert_eq!(
            cfg.agent_patterns_path.as_ref().map(|p| p.to_string_lossy().into_owned()).as_deref(),
            Some("/custom/patterns.toml")
        );
    }

    #[test]
    fn default_projects_values() {
        let projects = Config::default().projects;
        assert_eq!(projects.len(), 5);
        assert!(projects.contains_key("helios-cli"));
        assert!(projects.contains_key("portage"));
        assert!(projects.contains_key("agentapi"));
        assert!(projects.contains_key("cliproxy"));
        assert!(projects.contains_key("colab"));
    }

    #[test]
    fn default_harness_configs_values() {
        let defaults = Config::default().defaults;
        assert_eq!(defaults.len(), 4);
        let claude = defaults.get("claude").unwrap();
        assert!(claude.enabled);
        assert_eq!(claude.max_instances, 11);
        assert_eq!(claude.memory_limit_mb, 512);
        let forge = defaults.get("forge").unwrap();
        assert_eq!(forge.max_instances, 20);
        let node = defaults.get("node").unwrap();
        assert_eq!(node.max_instances, 30);
        let bun = defaults.get("bun").unwrap();
        assert_eq!(bun.memory_limit_mb, 384);
    }

    #[test]
    fn config_health_checks_empty_by_default() {
        let cfg = Config::default();
        assert!(cfg.health_checks.is_empty());
    }

    #[test]
    fn config_serve_jwt_inline_jwks() {
        let toml_str = r#"
            [serve.jwt]
            issuer = "https://issuer.example.com"
            audience = "my-audience"
            jwks = "{\"keys\":[]}"
        "#;
        let cfg: Config = toml::from_str(toml_str).expect("parse inline jwks");
        let jwt = cfg.serve.jwt.unwrap();
        assert_eq!(jwt.issuer, "https://issuer.example.com");
        assert_eq!(jwt.audience, "my-audience");
        assert!(jwt.jwks.is_some());
        assert!(jwt.jwks_path.is_none());
    }

    #[test]
    fn config_serve_rate_limit_from_toml() {
        let toml_str = r#"
            [serve]
            rate_limit_max = 200
            rate_limit_window_secs = 120
        "#;
        let cfg: Config = toml::from_str(toml_str).expect("parse");
        assert_eq!(cfg.serve.rate_limit_max, Some(200));
        assert_eq!(cfg.serve.rate_limit_window_secs, Some(120));
    }

    #[test]
    fn config_json_round_trip() {
        let original = Config::default();
        let serialized = serde_json::to_string(&original).expect("serialize JSON");
        let restored: Config = serde_json::from_str(&serialized).expect("deserialize JSON");
        assert_eq!(original.port.sharewei_port, restored.port.sharewei_port);
        assert_eq!(original.cast.default_transport, restored.cast.default_transport);
        assert_eq!(original.pool.enabled, restored.pool.enabled);
        assert_eq!(original.spawn_policy.nice_level, restored.spawn_policy.nice_level);
    }

    #[test]
    fn config_override_multiple_subsections() {
        let toml_str = r#"
            [runtime]
            max_memory_mb = 16384

            [monitoring]
            health_check_interval_secs = 10
            idle_threshold_secs = 30
            high_memory_threshold_mb = 4096
            idle_process_threshold = 10
            per_process_warn_memory_bytes = 2147483648

            [spawn]
            default_harness = "codex"
            prune_idle_seconds = 300
        "#;
        let cfg: Config = toml::from_str(toml_str).expect("parse multi-override");
        assert_eq!(cfg.runtime.max_memory_mb, Some(16384));
        assert_eq!(cfg.monitoring.health_check_interval_secs, 10);
        assert_eq!(cfg.monitoring.idle_threshold_secs, 30);
        assert_eq!(cfg.spawn.default_harness, "codex");
        assert_eq!(cfg.port.sharewei_port, 3100);
        assert_eq!(cfg.cast.default_transport, "wezterm");
    }
}
