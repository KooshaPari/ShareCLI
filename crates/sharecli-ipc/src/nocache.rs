// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Mutating-flag (`nocache_args`) detection from the Feb agent-harness.
//!
//! Origin: `~/Downloads/files/rules.conf` + `harness` dispatcher —
//! when a coalesce rule lists `nocache_args=X,Y` and any of those tokens
//! appear in argv, strategy falls back from coalesce → queue (never cache).

/// Default mutating flags that MUST bypass coalesce cache when present in argv.
///
/// Drawn from Feb `rules.conf` (`--fix`, `--unsafe-fixes`) plus common
/// write/force flags used by mutating tools.
pub const DEFAULT_NOCACHE_ARGS: &[&str] =
    &["--fix", "--unsafe-fixes", "--force", "--write", "--in-place", "-w", "-i"];

/// Return `true` when any argv token exactly matches a configured nocache flag.
///
/// Matching is exact (Feb harness: `[[ "$arg" == "$unsafe" ]]`), not prefix.
pub fn has_nocache_arg(argv: &[impl AsRef<str>], nocache_args: &[impl AsRef<str>]) -> bool {
    if nocache_args.is_empty() {
        return false;
    }
    for arg in argv {
        let arg = arg.as_ref();
        for flag in nocache_args {
            if arg == flag.as_ref() {
                return true;
            }
        }
    }
    false
}

/// Whether a would-be coalesce invocation MUST route to the queue instead.
///
/// Equivalent to Feb harness:
/// `if nocache_args match while STRATEGY=coalesce → STRATEGY=queue`.
pub fn should_bypass_coalesce(argv: &[impl AsRef<str>], nocache_args: &[impl AsRef<str>]) -> bool {
    has_nocache_arg(argv, nocache_args)
}

/// Parse a comma-separated `nocache_args=X,Y` option value (rules.conf style).
pub fn parse_nocache_args_csv(csv: &str) -> Vec<String> {
    csv.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_fix_flag() {
        let argv = ["ruff", "check", "--fix", "."];
        assert!(has_nocache_arg(&argv, DEFAULT_NOCACHE_ARGS));
        assert!(should_bypass_coalesce(&argv, DEFAULT_NOCACHE_ARGS));
    }

    #[test]
    fn read_only_check_does_not_bypass() {
        let argv = ["ruff", "check", "."];
        assert!(!has_nocache_arg(&argv, DEFAULT_NOCACHE_ARGS));
    }

    #[test]
    fn parse_csv_from_rules() {
        let flags = parse_nocache_args_csv("--fix,--unsafe-fixes");
        assert_eq!(flags, vec!["--fix", "--unsafe-fixes"]);
        assert!(has_nocache_arg(&["ruff", "--unsafe-fixes"], &flags));
    }

    #[test]
    fn empty_list_never_matches() {
        let empty: &[&str] = &[];
        assert!(!has_nocache_arg(&["--force"], empty));
    }
}
