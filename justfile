# /Users/<REDACTED>/CodeProjects/Phenotype/repos/phenotype-teamcomm/justfile
# phenotype-teamcomm — Phenotype-org standard task runner
# See https://just.systems/man/en/

set shell := ["bash", "-uc"]
set dotenv-load

# ---- Default: list available recipes ----
_default:
    @just --list

# ---- Build: produce release artifacts ----
build:
    cargo build --workspace --all-targets

# ---- Test: run the test suite ----
test:
    cargo test --workspace --all-features

# ---- Lint: clippy + fmt --check ----
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo fmt --all -- --check

# ---- Format: apply formatter ----
fmt:
    cargo fmt --all

# ---- Audit: security advisories (cargo-audit) ----
audit:
    @command -v cargo-audit >/dev/null && cargo audit || echo "cargo-audit not installed; install with: cargo install cargo-audit --locked"

# ---- Deny: license + advisory + ban + source checks (cargo-deny) ----
deny:
    @command -v cargo-deny >/dev/null && cargo deny check || echo "cargo-deny not installed; install with: cargo install cargo-deny --locked"

# ---- Grade: fleet-wide grading gate (uses vendored or central grade.sh) ----
grade:
    @if [ -f grade.sh ]; then ./grade.sh; \
    elif [ -f ../grade.sh ]; then bash ../grade.sh; \
    else echo "no grade.sh found (vendored or central)"; exit 1; \
    fi

grade-fast:
    @if [ -f grade.sh ]; then ./grade.sh --fast; \
    elif [ -f ../grade.sh ]; then bash ../grade.sh --fast; \
    else echo "no grade.sh found"; exit 1; \
    fi

# ---- CI: full local CI sweep ----
ci: lint test audit deny
    @echo "✓ CI checks pass"

# ---- Bonus recipes ----

# Type-check (cargo check) — fast subset of build
typecheck:
    cargo check --workspace --all-targets

# Find unused dependencies
unused:
    @command -v cargo-machete >/dev/null && cargo machete || echo "cargo-machete not installed"

# Generate docs
docs:
    cargo doc --workspace --no-deps

# Remove build artifacts
clean:
    cargo clean

# Test a single workspace crate
test-crate crate:
    @cargo metadata --no-deps --format-version 1 | jq -e --arg crate "{{crate}}" 'any(.packages[].name; . == $crate)' >/dev/null
    cargo test -p "{{crate}}" --all-features

# List all workspace crates
crates:
    @cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name' | sort
