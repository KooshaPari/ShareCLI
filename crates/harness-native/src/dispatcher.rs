// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Helios Shield Rust dispatcher — hot path replacement for bin/harness.
//!
//! Handles:
//!   - Panic mode: bypass to real binary
//!   - CLI mode (harness/helios-shield): exec bash
//!   - Human path: exec real binary (0 bash)
//!   - Agent + passthrough: exec real binary
//!   - Agent + strategies: Rust implementations (coalesce, queue, etc.)
//!
//! Usage: invoked as symlink (e.g. proxy/ruff -> helios-shield) or as harness/helios-shield

use std::collections::HashMap;
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use harness_native::find_real;
use harness_native::strategies::{self, RuleOpts};
use sharecli_ipc::parse_nocache_args_csv;
use tracing::{debug, info, warn};

fn init_tracing() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("harness_native=info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .try_init();
}

fn harness_home() -> PathBuf {
    if let Ok(h) = env::var("HARNESS_HOME") {
        return PathBuf::from(h);
    }
    // Resolve from executable location: bin/helios-shield -> parent of bin
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.ends_with("bin") || parent.ends_with("proxy") {
                if let Some(grandparent) = parent.parent() {
                    return grandparent.to_path_buf();
                }
            }
            return parent.to_path_buf();
        }
    }
    PathBuf::from(".")
}

fn invoked_as() -> String {
    let argv0 = env::args().next().unwrap_or_default();
    Path::new(&argv0).file_name().and_then(|s| s.to_str()).unwrap_or("harness").to_string()
}

fn is_cli_mode(invoked: &str) -> bool {
    matches!(invoked, "harness" | "helios-shield" | "helios_shield")
}

fn panic_mode(harness_home: &Path) -> bool {
    let panic_file = harness_home.join("var").join("panic.lock");
    let meta = match fs::metadata(&panic_file) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let mtime = match meta.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };
    let age = match std::time::SystemTime::now().duration_since(mtime) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if age.as_secs() >= 60 {
        let _ = fs::remove_file(&panic_file);
        return false;
    }
    true
}

fn read_real_cached(proxy_dir: &Path, cmd: &str) -> Option<PathBuf> {
    let cache = proxy_dir.join(format!(".{}.real", cmd));
    let path = fs::read_to_string(&cache).ok()?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let p = PathBuf::from(path);
    if p.is_file() && is_executable(&p) {
        Some(p)
    } else {
        None
    }
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_p: &Path) -> bool {
    true
}

fn get_rule(etc_dir: &Path, cmd: &str, subcmd: &str) -> (String, String) {
    let rules_file = etc_dir.join("rules.conf");
    let Ok(content) = fs::read_to_string(&rules_file) else {
        return ("passthrough".to_string(), String::new());
    };
    let exact_key = format!("{}__{}", cmd, if subcmd.is_empty() { "STAR" } else { subcmd });
    let wild_key = format!("{}__STAR", cmd);

    let mut rules: HashMap<String, (String, String)> = HashMap::new();
    for line in content.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with("equivalence=") {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let pattern = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim();
        let strategy = rest.split_whitespace().next().unwrap_or("passthrough");
        let opts = rest.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
        let (pcmd, psub) = if pattern.contains(':') {
            let mut sp = pattern.splitn(2, ':');
            (sp.next().unwrap_or(""), sp.next().unwrap_or("*"))
        } else {
            (pattern, "*")
        };
        let rule_key = format!("{}__{}", pcmd, if psub == "*" { "STAR" } else { psub });
        rules.insert(rule_key, (strategy.to_string(), opts));
    }

    if let Some((s, o)) = rules.get(&exact_key) {
        return (s.clone(), o.clone());
    }
    if let Some((s, o)) = rules.get(&wild_key) {
        return (s.clone(), o.clone());
    }
    ("passthrough".to_string(), String::new())
}

fn parse_opts(opts_str: &str) -> RuleOpts {
    let mut opts = RuleOpts {
        ttl: 10,
        error_ttl: 10,
        max_concurrent: 1,
        priority: "normal".to_string(),
        cache_key: "time".to_string(),
        breaker_threshold: 0,
        breaker_window: 60,
        breaker_cooldown: 30,
        retry_max: 3,
        retry_backoff_ms: 100,
        retry_jitter: 0.1,
        jobserver_borrow: true,
        ..RuleOpts::default()
    };

    for part in opts_str.split_whitespace() {
        if let Some((k, v)) = part.split_once('=') {
            match k {
                "ttl" => opts.ttl = v.parse().unwrap_or(10),
                "error_ttl" => opts.error_ttl = v.parse().unwrap_or(10),
                "debounce_ms" => opts.debounce_ms = v.parse().unwrap_or(0),
                "max_concurrent" => opts.max_concurrent = v.parse().unwrap_or(1),
                "priority" => opts.priority = v.to_string(),
                "cache_key" => opts.cache_key = v.to_string(),
                "batch_key" => opts.batch_key = v.to_string(),
                "causal_domain" => opts.causal_domain = v.to_string(),
                "breaker_threshold" => opts.breaker_threshold = v.parse().unwrap_or(0),
                "breaker_window" => opts.breaker_window = v.parse().unwrap_or(60),
                "breaker_cooldown" => opts.breaker_cooldown = v.parse().unwrap_or(30),
                "retry_max" => opts.retry_max = v.parse().unwrap_or(3),
                "retry_backoff_ms" => opts.retry_backoff_ms = v.parse().unwrap_or(100),
                "retry_jitter" => opts.retry_jitter = v.parse().unwrap_or(0.1),
                "stale" => opts.stale_threshold = v.parse().unwrap_or(0),
                "semantic" => opts.semantic = v == "1",
                "nocache_args" => opts.nocache_args = Some(parse_nocache_args_csv(v)),
                "jobserver_auth" => opts.jobserver_auth = v.to_string(),
                "jobserver_tokens" => opts.jobserver_tokens = v.parse().unwrap_or(0),
                "jobserver_borrow" => opts.jobserver_borrow = v == "1",
                _ => {}
            }
        }
    }
    opts
}

fn compute_cache_key(harness_home: &Path, mode: &str, cmd: &str, args: &[String]) -> String {
    let cache_key_bin = harness_home.join("bin").join("harness-cache-key");
    let cache_key_bin = if cache_key_bin.is_file() {
        cache_key_bin
    } else {
        harness_home.join("harness-native").join("target").join("release").join("harness-cache-key")
    };
    if !cache_key_bin.is_file() {
        return format!("{}:{}", cmd, args.join(":"));
    }
    let mut cargs = vec![mode.to_string(), cmd.to_string()];
    cargs.extend(args.iter().cloned());
    let out = Command::new(&cache_key_bin).args(&cargs).output();
    let out = match out {
        Ok(o) => o,
        Err(_) => return format!("{}:{}", cmd, args.join(":")),
    };
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn get_agent_name(harness_var: &Path) -> String {
    let session_dir = harness_var.join("agent_sessions");
    let ppid = env::var("PPID").unwrap_or_else(|_| "1".to_string());
    let session_file = session_dir.join(&ppid);
    if let Ok(cached) = fs::read_to_string(&session_file) {
        let cached = cached.trim();
        if cached != "human" && cached != "agent" && !cached.is_empty() {
            return cached.to_string();
        }
    }
    "unknown".to_string()
}

fn first_non_flag_arg(args: &[String]) -> String {
    for a in args {
        if !a.starts_with('-') {
            return a.clone();
        }
    }
    String::new()
}

#[cfg(unix)]
fn exec_bash_harness(harness_home: &Path, invoked_as: &str, args: &[String]) -> ! {
    // Prefer harness.bash (when Rust binary replaced bin/harness), else bin/harness
    let bash_harness = harness_home.join("bin").join("harness.bash");
    let bash_harness = if bash_harness.is_file() {
        bash_harness
    } else {
        harness_home.join("bin").join("harness")
    };
    let bash_path = bash_harness.to_string_lossy().into_owned();
    // exec -a $invoked_as so harness sees correct INVOKED_AS in proxy mode
    let err = Command::new("bash")
        .arg(&bash_path)
        .args(args)
        .env("HARNESS_HOME", harness_home)
        .env("HARNESS_INVOKED_AS", invoked_as)
        .exec();
    eprintln!("helios-shield: exec bash failed: {}", err);
    std::process::exit(127);
}

#[cfg(unix)]
fn exec_real(real: &Path, args: &[String]) -> ! {
    let err = Command::new(real).args(args).exec();
    eprintln!("helios-shield: exec {:?} failed: {}", real, err);
    std::process::exit(127);
}

#[cfg(unix)]
fn main() {
    init_tracing();
    let harness_home = harness_home();
    env::set_var("HARNESS_HOME", harness_home.as_os_str());

    let invoked = invoked_as();
    let args: Vec<String> = env::args().skip(1).collect();
    info!(invoked = %invoked, "helios-shield dispatch");

    // CLI mode: delegate to bash (invoked_as=harness for CLI)
    if is_cli_mode(&invoked) {
        exec_bash_harness(&harness_home, "harness", &args);
    }

    // PROXY mode
    let proxy_dir = harness_home.join("proxy");
    let etc_dir = harness_home.join("etc");

    // Panic mode: bypass to real if we have cache
    if panic_mode(&harness_home) {
        if let Some(real) = read_real_cached(&proxy_dir, &invoked) {
            exec_real(&real, &args);
        }
        // No cache: fall through to bash
    }

    // Resolve real binary: .real cache first, then PATH scan (find_real)
    let harness_bin = env::current_exe().ok();
    let real = match read_real_cached(&proxy_dir, &invoked) {
        Some(p) => {
            debug!(cmd = %invoked, path = %p.display(), "resolved via .real cache");
            p
        }
        None => {
            match find_real::find_real(&proxy_dir, &harness_home, harness_bin.as_deref(), &invoked)
            {
                Some(p) => {
                    debug!(cmd = %invoked, path = %p.display(), "resolved via PATH scan");
                    p
                }
                None => {
                    warn!(cmd = %invoked, "find_real: binary not found, delegating to bash");
                    exec_bash_harness(&harness_home, &invoked, &args);
                }
            }
        }
    };

    // Agent detection: run harness-is-agent
    let is_agent = {
        let is_agent_bin = harness_home.join("bin").join("harness-is-agent");
        let is_agent_bin = if is_agent_bin.is_file() {
            is_agent_bin
        } else {
            harness_home
                .join("harness-native")
                .join("target")
                .join("release")
                .join("harness-is-agent")
        };
        if is_agent_bin.is_file() {
            let ppid = env::var("PPID").unwrap_or_else(|_| "1".to_string());
            Command::new(&is_agent_bin)
                .env("HARNESS_ETC", etc_dir.as_os_str())
                .env("HARNESS_VAR", harness_home.join("var").as_os_str())
                .env("PPID", &ppid)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            // No native binary: defer to bash
            exec_bash_harness(&harness_home, &invoked, &args);
        }
    };

    // Human: immediate exec
    if !is_agent {
        exec_real(&real, &args);
    }

    // Agent: check strategy and run Rust strategies
    let subcmd = first_non_flag_arg(&args);
    let (strategy, opts_str) = get_rule(&etc_dir, &invoked, &subcmd);
    let opts = parse_opts(&opts_str);

    if strategy == "passthrough" {
        exec_real(&real, &args);
    }

    // Compute cache_key for strategies that need it
    let needs_cache_key = matches!(
        strategy.as_str(),
        "coalesce" | "cache" | "incremental" | "circuit_breaker" | "speculative" | "proactive_warm"
    );
    let cache_key = if needs_cache_key {
        compute_cache_key(&harness_home, &opts.cache_key, &invoked, &args)
    } else {
        String::new()
    };

    let agent_name = get_agent_name(&harness_home.join("var"));

    match strategies::execute(strategies::ExecRequest {
        strategy: &strategy,
        harness_home: &harness_home,
        real_cmd: &real,
        cmd_name: &invoked,
        subcmd: &subcmd,
        cache_key: &cache_key,
        opts: &opts,
        args: &args,
        agent_name: &agent_name,
    }) {
        Ok(code) => std::process::exit(code),
        Err(_) => exec_bash_harness(&harness_home, &invoked, &args),
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("helios-shield: unix-only dispatcher; not supported on this platform");
    std::process::exit(1);
}
