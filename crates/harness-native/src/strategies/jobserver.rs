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

    if !opts.jobserver_auth.is_empty() {
        match inherited_descriptor() {
            Some(auth) if auth == opts.jobserver_auth => {}
            Some(auth) => {
                return Err(format!(
                    "jobserver_auth mismatch: rule requested {:?}, inherited provider is {:?}",
                    opts.jobserver_auth, auth
                ));
            }
            None => {
                return Err(
                    "jobserver_auth was configured but no inherited native provider is available"
                        .to_string(),
                );
            }
        }
    }

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
    fn explicit_auth_without_inherited_provider_fails_closed_when_environment_is_clean() {
        let saved_make = env::var_os("MAKEFLAGS");
        let saved_cargo = env::var_os("CARGO_MAKEFLAGS");
        let saved_m = env::var_os("MFLAGS");
        unsafe {
            env::remove_var("MAKEFLAGS");
            env::remove_var("CARGO_MAKEFLAGS");
            env::remove_var("MFLAGS");
        }

        let opts = RuleOpts {
            jobserver_borrow: true,
            jobserver_auth: "fifo:/tmp/not-present".into(),
            ..RuleOpts::default()
        };
        let dir = tempfile::TempDir::new().expect("tempdir");
        let err = run(dir.path(), Path::new("/bin/echo"), &["should-not-run"], &opts)
            .expect_err("configured auth without inherited provider must fail");

        unsafe {
            match saved_make {
                Some(value) => env::set_var("MAKEFLAGS", value),
                None => env::remove_var("MAKEFLAGS"),
            }
            match saved_cargo {
                Some(value) => env::set_var("CARGO_MAKEFLAGS", value),
                None => env::remove_var("CARGO_MAKEFLAGS"),
            }
            match saved_m {
                Some(value) => env::set_var("MFLAGS", value),
                None => env::remove_var("MFLAGS"),
            }
        }

        assert!(err.contains("no inherited native provider"), "{err}");
    }
}
