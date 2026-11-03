// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Domain errors and stable CLI exit codes (audit-v38 C01 L14).
//!
//! Operators map `SHARECLI_ERROR_CODE` in stderr to runbooks. HTTP serve
//! surfaces use [`crate::error_envelope::ErrorEnvelope`] separately.
#![allow(dead_code)]

use std::fmt;
use std::process::ExitCode;

use thiserror::Error;

/// Stable machine-readable error codes (stderr + docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    ConfigInvalid,
    UserInput,
    NotFound,
    Auth,
    Io,
    Spawn,
    Serve,
    Internal,
}

impl ErrorCode {
    /// Snake-case identifier printed as `SHARECLI_ERROR_CODE=<code>`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigInvalid => "config_invalid",
            Self::UserInput => "user_input",
            Self::NotFound => "not_found",
            Self::Auth => "auth",
            Self::Io => "io",
            Self::Spawn => "spawn",
            Self::Serve => "serve",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Process exit codes (sysexits-style subset).
pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_CONFIG: u8 = 78;
pub const EXIT_USAGE: u8 = 64;
pub const EXIT_NOT_FOUND: u8 = 69;
pub const EXIT_AUTH: u8 = 77;
pub const EXIT_IO: u8 = 74;
pub const EXIT_SPAWN: u8 = 70;
pub const EXIT_SERVE: u8 = 75;
pub const EXIT_INTERNAL: u8 = 1;

/// Typed domain / CLI error.
#[derive(Debug, Error)]
pub enum SharecliError {
    #[error("{message}")]
    ConfigInvalid { message: String },

    #[error("{message}")]
    UserInput { message: String },

    #[error("{message}")]
    NotFound { message: String },

    #[error("{message}")]
    Auth { message: String },

    #[error("{message}")]
    Io {
        message: String,
        #[source]
        source: Option<std::io::Error>,
    },

    #[error("{message}")]
    Spawn { message: String },

    #[error("{message}")]
    Serve { message: String },

    #[error("{message}")]
    Internal {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },
}

pub type Result<T> = std::result::Result<T, SharecliError>;

impl SharecliError {
    pub fn config_invalid(message: impl Into<String>) -> Self {
        Self::ConfigInvalid { message: message.into() }
    }

    pub fn user_input(message: impl Into<String>) -> Self {
        Self::UserInput { message: message.into() }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound { message: message.into() }
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth { message: message.into() }
    }

    pub fn io(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io { message: message.into(), source: Some(source) }
    }

    pub fn spawn(message: impl Into<String>) -> Self {
        Self::Spawn { message: message.into() }
    }

    pub fn serve(message: impl Into<String>) -> Self {
        Self::Serve { message: message.into() }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal { message: message.into(), source: None }
    }

    /// Stable code for operator runbooks / support.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::ConfigInvalid { .. } => ErrorCode::ConfigInvalid,
            Self::UserInput { .. } => ErrorCode::UserInput,
            Self::NotFound { .. } => ErrorCode::NotFound,
            Self::Auth { .. } => ErrorCode::Auth,
            Self::Io { .. } => ErrorCode::Io,
            Self::Spawn { .. } => ErrorCode::Spawn,
            Self::Serve { .. } => ErrorCode::Serve,
            Self::Internal { .. } => ErrorCode::Internal,
        }
    }

    /// Sysexits-style exit status for the CLI process.
    pub fn exit_code(&self) -> u8 {
        match self.code() {
            ErrorCode::ConfigInvalid => EXIT_CONFIG,
            ErrorCode::UserInput => EXIT_USAGE,
            ErrorCode::NotFound => EXIT_NOT_FOUND,
            ErrorCode::Auth => EXIT_AUTH,
            ErrorCode::Io => EXIT_IO,
            ErrorCode::Spawn => EXIT_SPAWN,
            ErrorCode::Serve => EXIT_SERVE,
            ErrorCode::Internal => EXIT_INTERNAL,
        }
    }

    pub fn exit_status(&self) -> ExitCode {
        ExitCode::from(self.exit_code())
    }

    /// Print `SHARECLI_ERROR_CODE` + message to stderr (no panic paths).
    pub fn eprint(&self) {
        eprintln!("SHARECLI_ERROR_CODE={} error: {self}", self.code());
        if let Self::Io { source: Some(src), .. } = self {
            eprintln!("  caused by: {src}");
        }
        if let Self::Internal { source: Some(src), .. } = self {
            eprintln!("  caused by: {src:#}");
        }
    }

    /// Print and terminate the process (for pre-async validation helpers).
    pub fn report_and_exit(self) -> ! {
        self.eprint();
        std::process::exit(i32::from(self.exit_code()));
    }
}

impl From<std::io::Error> for SharecliError {
    fn from(source: std::io::Error) -> Self {
        Self::io(source.to_string(), source)
    }
}

impl From<anyhow::Error> for SharecliError {
    fn from(source: anyhow::Error) -> Self {
        Self::Internal { message: source.to_string(), source: Some(source) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_sysexits_subset() {
        assert_eq!(SharecliError::config_invalid("bad").exit_code(), EXIT_CONFIG);
        assert_eq!(SharecliError::user_input("bad").exit_code(), EXIT_USAGE);
        assert_eq!(SharecliError::not_found("x").exit_code(), EXIT_NOT_FOUND);
        assert_eq!(SharecliError::auth("denied").exit_code(), EXIT_AUTH);
        assert_eq!(SharecliError::internal("boom").exit_code(), EXIT_INTERNAL);
    }

    #[test]
    fn constructors_map_to_codes() {
        let cases = [
            (SharecliError::config_invalid("x"), ErrorCode::ConfigInvalid),
            (SharecliError::user_input("x"), ErrorCode::UserInput),
            (SharecliError::not_found("x"), ErrorCode::NotFound),
            (SharecliError::auth("x"), ErrorCode::Auth),
            (SharecliError::spawn("x"), ErrorCode::Spawn),
            (SharecliError::serve("x"), ErrorCode::Serve),
            (SharecliError::internal("x"), ErrorCode::Internal),
        ];
        for (err, code) in cases {
            assert_eq!(err.code(), code);
        }
    }

    #[test]
    fn exit_status_round_trips_exit_code() {
        let err = SharecliError::user_input("bad flag");
        assert_eq!(err.exit_status(), ExitCode::from(EXIT_USAGE));
    }
}
