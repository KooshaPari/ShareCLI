//! GNU make jobserver discovery model.
//!
//! Parsing/detection only. This does not acquire tokens yet.

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
}
