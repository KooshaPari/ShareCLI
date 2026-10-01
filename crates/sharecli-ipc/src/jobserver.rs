//! GNU make jobserver discovery model.
//!
//! Discovery plus a bounded native-provider adapter.
//!
//! The provider delegates transport details to the maintained `jobserver` crate;
//! policy remains outside this module.

use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobserverTransport {
    PosixFifo(String),
    PosixFileDescriptors { read_fd: i32, write_fd: i32 },
    WindowsSemaphore(String),
    Opaque(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobserverDescriptor {
    pub auth: String,
    pub style: Option<String>,
    pub transport: JobserverTransport,
}

pub fn parse_makeflags_jobserver(makeflags: &str, windows: bool) -> Option<JobserverDescriptor> {
    let mut auth: Option<String> = None;
    let mut style: Option<String> = None;

    for token in makeflags.split_whitespace() {
        if let Some(v) = token.strip_prefix("--jobserver-auth=") {
            auth = Some(v.to_string());
        } else if let Some(v) = token.strip_prefix("--jobserver-style=") {
            style = Some(v.to_string());
        }
    }

    let auth = auth?;
    let transport = if windows {
        JobserverTransport::WindowsSemaphore(auth.clone())
    } else if let Some(path) = auth.strip_prefix("fifo:") {
        JobserverTransport::PosixFifo(path.to_string())
    } else if let Some((r,w)) = auth.split_once(',') {
        match (r.parse::<i32>(), w.parse::<i32>()) {
            (Ok(read_fd), Ok(write_fd)) => JobserverTransport::PosixFileDescriptors { read_fd, write_fd },
            _ => JobserverTransport::Opaque(auth.clone()),
        }
    } else {
        JobserverTransport::Opaque(auth.clone())
    };

    Some(JobserverDescriptor { auth, style, transport })
}


/// Connected native GNU-make jobserver provider.
///
/// A ShareCLI process invoked by GNU make already owns one implicit slot.
/// Calls to `acquire_extra_tokens` therefore represent only additional slots.
#[derive(Debug, Clone)]
pub struct NativeJobserverClient {
    client: ::jobserver::Client,
}

/// RAII holder for extra native jobserver tokens. Dropping the lease returns
/// every acquired token through the upstream client's exact transport semantics.
#[derive(Debug)]
pub struct NativeJobserverLease {
    tokens: Vec<::jobserver::Acquired>,
}

impl NativeJobserverLease {
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

impl NativeJobserverClient {
    /// Connect to the inherited GNU-make/Cargo jobserver.
    ///
    /// # Safety
    ///
    /// The upstream API is unsafe on Unix because inherited file descriptors
    /// are adopted. Call this early in process lifetime, before unrelated file
    /// descriptor manipulation. No caller should create multiple independently
    /// owned clients from the same inherited descriptors.
    pub unsafe fn from_environment() -> Result<Option<Self>, String> {
        let discovered = ::jobserver::Client::from_env_ext(true);
        if discovered.var.is_none() {
            return Ok(None);
        }
        discovered
            .client
            .map(|client| Some(Self { client }))
            .map_err(|err| format!("native jobserver environment is present but unusable: {err}"))
    }

    /// Create an isolated provider for deterministic tests or a ShareCLI-owned
    /// token pool. This is not evidence that an inherited GNU-make jobserver exists.
    pub fn new_owned(limit: usize) -> Result<Self, String> {
        ::jobserver::Client::new(limit)
            .map(|client| Self { client })
            .map_err(|err| format!("create native jobserver: {err}"))
    }

    /// Acquire exactly `count` extra slots.
    ///
    /// Partial acquisition is fail-safe: if a later acquisition errors, the
    /// already-acquired RAII tokens are dropped before the error returns.
    pub fn acquire_extra_tokens(&self, count: usize) -> Result<NativeJobserverLease, String> {
        let mut tokens = Vec::with_capacity(count);
        for _ in 0..count {
            match self.client.acquire() {
                Ok(token) => tokens.push(token),
                Err(err) => {
                    drop(tokens);
                    return Err(format!("acquire native jobserver token: {err}"));
                }
            }
        }
        Ok(NativeJobserverLease { tokens })
    }

    /// Forward this same native token pool to a nested make-compatible child.
    /// This is required for pipe-style jobservers where inherited descriptors
    /// would otherwise be closed on exec.
    pub fn configure_make_child(&self, command: &mut Command) {
        self.client.configure_make(command);
    }

    pub fn available_tokens(&self) -> Result<usize, String> {
        self.client.available().map_err(|err| format!("query native jobserver availability: {err}"))
    }
}

/// Convert a desired total parallelism into *extra* jobserver tokens.
///
/// GNU make gives every invoked command one implicit slot; borrowing a token for
/// that first unit of work would double-throttle nested tools.
pub fn extra_tokens_for_total_parallelism(total_parallelism: usize) -> usize {
    total_parallelism.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_jobserver_auth_wins() {
        let d = parse_makeflags_jobserver(
            "-j --jobserver-auth=3,4 --jobserver-auth=fifo:/tmp/new --jobserver-style=fifo",
            false,
        ).unwrap();
        assert_eq!(d.auth, "fifo:/tmp/new");
        assert_eq!(d.style.as_deref(), Some("fifo"));
        assert_eq!(d.transport, JobserverTransport::PosixFifo("/tmp/new".into()));
    }

    #[test]
    fn parses_posix_fd_pair() {
        let d = parse_makeflags_jobserver("--jobserver-auth=7,8", false).unwrap();
        assert_eq!(d.transport, JobserverTransport::PosixFileDescriptors { read_fd: 7, write_fd: 8 });
    }

    #[test]
    fn windows_auth_is_named_semaphore() {
        let d = parse_makeflags_jobserver("--jobserver-auth=Global\\make123 --jobserver-style=sem", true).unwrap();
        assert_eq!(d.transport, JobserverTransport::WindowsSemaphore("Global\\make123".into()));
        assert_eq!(d.style.as_deref(), Some("sem"));
    }

    #[test]
    fn no_auth_means_no_native_provider() {
        assert!(parse_makeflags_jobserver("-j8 --output-sync", false).is_none());
    }
    #[test]
    fn implicit_slot_is_not_double_counted() {
        assert_eq!(extra_tokens_for_total_parallelism(0), 0);
        assert_eq!(extra_tokens_for_total_parallelism(1), 0);
        assert_eq!(extra_tokens_for_total_parallelism(4), 3);
    }

    #[cfg(unix)]
    #[test]
    fn native_provider_raii_returns_acquired_tokens() {
        let client = NativeJobserverClient::new_owned(2).expect("owned jobserver");
        let before = client.available_tokens().expect("available before");
        assert_eq!(before, 2);

        {
            let lease = client.acquire_extra_tokens(1).expect("acquire one extra token");
            assert_eq!(lease.token_count(), 1);
            assert_eq!(client.available_tokens().expect("available while leased"), 1);
        }

        assert_eq!(client.available_tokens().expect("available after drop"), 2);
    }

    #[cfg(unix)]
    #[test]
    fn child_configuration_exports_same_jobserver() {
        let client = NativeJobserverClient::new_owned(2).expect("owned jobserver");
        let mut command = Command::new("true");
        client.configure_make_child(&mut command);
        let names: Vec<String> = command
            .get_envs()
            .filter_map(|(name, _)| name.to_str().map(ToOwned::to_owned))
            .collect();
        assert!(
            names.iter().any(|name| name == "MAKEFLAGS" || name == "CARGO_MAKEFLAGS"),
            "configured child did not receive a jobserver environment: {names:?}"
        );
    }

}
