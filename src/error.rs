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

/// Process exit codes.
///
/// Operational failures keep the sysexits-style spacing (`EX_CONFIG` 78,
/// `EX_USAGE` 64, `EX_IOERR` 74, `EX_SOFTWARE` 70 …). `EXIT_NOT_FOUND` is `2`
/// because the audit exit-code taxonomy mandates `NotFound → 2`
/// (docs/audit/2026-09-20/PLAN.md task 1.10), not the sysexits `EX_UNAVAILABLE`.
pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_CONFIG: u8 = 78;
pub const EXIT_USAGE: u8 = 64;
pub const EXIT_NOT_FOUND: u8 = 2;
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

    /// Process exit status for the CLI (see the `EXIT_*` table above).
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

    /// Render the operator-facing stderr block.
    ///
    /// Every distinct fact appears once. The cause continuation is dropped when
    /// it only repeats the top frame (PLAN.md task 1.10 line 181 — "stop
    /// double-printing") and is debug-only otherwise, per FINDINGS.md:84-85
    /// (`↳ caused by`, gated on `RUST_BACKTRACE`).
    pub fn render_stderr(&self, debug_detail: bool) -> String {
        let mut rendered = format!("SHARECLI_ERROR_CODE={} error: {self}", self.code());
        if debug_detail {
            if let Some(detail) = self.cause_detail() {
                rendered.push_str("\n  ↳ caused by: ");
                rendered.push_str(&detail);
            }
        }
        rendered
    }

    /// Cause text that adds information beyond the top frame, if any.
    fn cause_detail(&self) -> Option<String> {
        match self {
            Self::Io { message, source: Some(src) } => distinct_cause(message, &src.to_string()),
            Self::Internal { message, source: Some(src) } => {
                distinct_cause(message, &format!("{src:#}"))
            }
            _ => None,
        }
    }

    /// Print [`Self::render_stderr`] to stderr (no panic paths).
    pub fn eprint(&self) {
        eprintln!("{}", self.render_stderr(debug_causes_enabled()));
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

/// True when `RUST_BACKTRACE` asks for debug detail (FINDINGS.md:84-85).
fn debug_causes_enabled() -> bool {
    std::env::var_os("RUST_BACKTRACE").is_some_and(|value| !value.is_empty() && value != "0")
}

/// The part of a cause rendering the top frame does not already state.
///
/// `From<anyhow::Error>` stores the anyhow `Display` as both `message` and
/// `source`, so the cause renders identically to the message and adds nothing
/// (PLAN.md:181). A chained cause keeps only the frames below the top one.
fn distinct_cause(message: &str, rendered: &str) -> Option<String> {
    let rest = match rendered.strip_prefix(message) {
        Some(tail)
            if tail.is_empty()
                || tail.starts_with(':')
                || tail.starts_with(char::is_whitespace) =>
        {
            tail.trim_start_matches(|ch: char| ch == ':' || ch.is_whitespace())
        }
        _ => rendered,
    };
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

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

    #[test]
    fn codes_render_as_snake_case_identifiers() {
        let cases = [
            (ErrorCode::ConfigInvalid, "config_invalid"),
            (ErrorCode::UserInput, "user_input"),
            (ErrorCode::NotFound, "not_found"),
            (ErrorCode::Auth, "auth"),
            (ErrorCode::Io, "io"),
            (ErrorCode::Spawn, "spawn"),
            (ErrorCode::Serve, "serve"),
            (ErrorCode::Internal, "internal"),
        ];
        for (code, rendered) in cases {
            assert_eq!(code.as_str(), rendered);
            assert_eq!(code.to_string(), rendered, "Display must match as_str");
        }
    }

    #[test]
    fn io_and_spawn_serve_exit_codes_are_stable() {
        let io = SharecliError::io("disk", std::io::Error::other("boom"));
        assert_eq!(io.code(), ErrorCode::Io);
        assert_eq!(io.exit_code(), EXIT_IO);
        assert_eq!(io.exit_status(), ExitCode::from(EXIT_IO));

        assert_eq!(SharecliError::spawn("x").exit_code(), EXIT_SPAWN);
        assert_eq!(SharecliError::serve("x").exit_code(), EXIT_SERVE);
    }

    #[test]
    fn from_io_error_preserves_the_message_as_the_source() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err: SharecliError = source.into();
        assert_eq!(err.code(), ErrorCode::Io);
        assert_eq!(err.to_string(), "no such file");
    }

    #[test]
    fn from_anyhow_error_drops_the_duplicated_cause() {
        // `From<anyhow::Error>` stores the same Display in both message and
        // source, so the rendered cause must not repeat the top frame.
        let err: SharecliError = anyhow::anyhow!("single frame").into();
        assert_eq!(err.code(), ErrorCode::Internal);
        let rendered = err.render_stderr(true);
        assert_eq!(rendered, "SHARECLI_ERROR_CODE=internal error: single frame");
        assert!(!rendered.contains("caused by"), "a repeated frame must not be printed twice");
    }

    #[test]
    fn render_stderr_omits_the_cause_without_debug_detail() {
        let err =
            SharecliError::io("cannot open config", std::io::Error::other("permission denied"));
        let rendered = err.render_stderr(false);
        assert_eq!(rendered, "SHARECLI_ERROR_CODE=io error: cannot open config");
        assert!(!rendered.contains("permission denied"));
    }

    #[test]
    fn render_stderr_appends_a_distinct_cause_in_debug_mode() {
        let err =
            SharecliError::io("cannot open config", std::io::Error::other("permission denied"));
        let rendered = err.render_stderr(true);
        assert!(rendered.starts_with("SHARECLI_ERROR_CODE=io error: cannot open config"));
        assert!(rendered.contains("↳ caused by: permission denied"), "{rendered}");
    }

    #[test]
    fn rendered_cause_strips_a_source_that_repeats_the_message_prefix() {
        let err = SharecliError::io("loading config", std::io::Error::other("loading config: bad"));
        let rendered = err.render_stderr(true);
        assert!(
            rendered.ends_with("↳ caused by: bad"),
            "the shared prefix must be dropped: {rendered}"
        );
    }

    #[test]
    fn empty_and_structured_sources_have_no_distinct_cause() {
        let plain = SharecliError::internal("no source");
        assert_eq!(plain.render_stderr(true), "SHARECLI_ERROR_CODE=internal error: no source");

        let anyhow_like: SharecliError = anyhow::Error::msg("same text").into();
        assert!(!anyhow_like.render_stderr(true).contains("caused by"));
    }

    #[test]
    fn distinct_cause_classifies_each_render_shape() {
        // Nothing beyond the message.
        assert_eq!(distinct_cause("msg", "msg"), None);
        assert_eq!(distinct_cause("msg", ""), None);
        // A separated continuation keeps the tail.
        assert_eq!(distinct_cause("msg", "msg: detail"), Some("detail".to_string()));
        assert_eq!(
            distinct_cause("msg", "msg whitespace detail"),
            Some("whitespace detail".to_string())
        );
        // A prefix that is not a frame separator belongs to the same frame.
        assert_eq!(
            distinct_cause("msg", "message but longer"),
            Some("message but longer".to_string())
        );
    }

    #[test]
    fn debug_causes_enabled_reads_the_backtrace_variable() {
        let prev = std::env::var_os("RUST_BACKTRACE");
        std::env::set_var("RUST_BACKTRACE", "1");
        assert!(debug_causes_enabled());
        std::env::set_var("RUST_BACKTRACE", "0");
        assert!(!debug_causes_enabled(), "`0` explicitly disables debug detail");
        std::env::set_var("RUST_BACKTRACE", "");
        assert!(!debug_causes_enabled(), "an empty value disables debug detail");
        match prev {
            Some(v) => std::env::set_var("RUST_BACKTRACE", v),
            None => std::env::remove_var("RUST_BACKTRACE"),
        }
    }

    #[test]
    fn io_chain_writes_through_a_generic_writer() {
        // Exercises `std::io::Write` imports used by the module's helpers.
        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "{}", SharecliError::auth("denied").render_stderr(false)).expect("write");
        assert!(String::from_utf8(buf).expect("utf8").contains("error: denied"));
    }
}
