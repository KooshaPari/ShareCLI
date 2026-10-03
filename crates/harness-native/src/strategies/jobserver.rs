use std::env;
use std::path::Path;
use std::process::Command;

use sharecli_core::ChildCommandConfigurator;
use sharecli_ipc::{parse_makeflags_jobserver, NativeJobserverClient};

use super::process;
use super::RuleOpts;

struct JobserverChildConfigurator {
    client: NativeJobserverClient,
}

impl ChildCommandConfigurator for JobserverChildConfigurator {
    fn configure(&self, command: &mut Command) {
        self.client.configure_make_child(command);
    }
}

fn inherited_descriptor() -> Option<String> {
    for key in ["MAKEFLAGS", "CARGO_MAKEFLAGS", "MFLAGS"] {
        if let Ok(value) = env::var(key) {
            if let Some(descriptor) = parse_makeflags_jobserver(&value, cfg!(windows)) {
                return Some(descriptor.auth);
            }
        }
    }
    None
}

fn validate_requested_auth(requested: &str, inherited: Option<&str>) -> Result<(), String> {
    if requested.is_empty() {
        return Ok(());
    }
    match inherited {
        Some(auth) if auth == requested => Ok(()),
        Some(auth) => Err(format!(
            "jobserver_auth mismatch: rule requested {:?}, inherited provider is {:?}",
            requested, auth
        )),
        None => Err(
            "jobserver_auth was configured but no inherited native provider is available"
                .to_string(),
        ),
    }
}

/// Execute a command under ShareCLI's normal Hypervisor while interoperating
/// with an inherited GNU-make/Cargo jobserver when policy enables borrowing.
///
/// A single wrapped child consumes ShareCLI's inherited implicit slot rather
/// than acquiring an extra token. The native client is propagated into the
/// exact child command so nested jobserver-aware tools share the same pool.
/// Extra-token acquisition is exposed separately by NativeJobserverClient for
/// scheduler use when ShareCLI actually runs more than one work item.
pub fn run(
    harness_home: &Path,
    real_cmd: &Path,
    args: &[&str],
    opts: &RuleOpts,
) -> Result<i32, String> {
    if !opts.jobserver_borrow {
        return process::run_status(harness_home, real_cmd, args, opts);
    }

    let inherited_auth = inherited_descriptor();
    validate_requested_auth(&opts.jobserver_auth, inherited_auth.as_deref())?;

    // SAFETY: dispatcher strategy execution happens before it opens provider-
    // specific jobserver handles. The opt-in jobserver_borrow gate prevents
    // ordinary harness strategy tests/callers from adopting Cargo/Make FDs.
    let client = unsafe { NativeJobserverClient::from_environment()? };
    let Some(client) = client else {
        // No native provider to coordinate with. Keep ShareCLI's own admission
        // path rather than fabricating tokens or treating absence as capacity.
        return process::run_status(harness_home, real_cmd, args, opts);
    };

    let configurator = JobserverChildConfigurator { client };
    process::run_status_with_command_configurator(
        harness_home,
        real_cmd,
        args,
        opts,
        &configurator,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowing_disabled_does_not_touch_native_environment() {
        let opts = RuleOpts {
            jobserver_borrow: false,
            ..RuleOpts::default()
        };
        let dir = tempfile::TempDir::new().expect("tempdir");
        let code = run(dir.path(), Path::new("/bin/echo"), &["jobserver-disabled"], &opts)
            .expect("disabled jobserver strategy must retain hypervisor execution");
        assert_eq!(code, 0);
    }

    #[test]
    fn explicit_auth_validation_fails_closed_without_matching_provider() {
        assert!(validate_requested_auth("fifo:/tmp/a", None)
            .expect_err("missing provider must fail")
            .contains("no inherited native provider"));
        assert!(validate_requested_auth("fifo:/tmp/a", Some("fifo:/tmp/b"))
            .expect_err("wrong provider must fail")
            .contains("mismatch"));
        validate_requested_auth("fifo:/tmp/a", Some("fifo:/tmp/a"))
            .expect("exact inherited provider should pass");
    }
}
