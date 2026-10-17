//! Subcommand enums — one per CLI surface area.
//!
//! Each enum below is a re-export of a typed clap subcommand declaration that
//! lives in its own module to keep individual files under the project's
//! ~500-line target. The root enum (`root::Commands`) composes them via
//! clap's `#[command(subcommand)]` pattern.

pub mod cast;
pub mod fleet;
pub mod fuse;
pub mod mesh;
pub mod proc_compose;
pub mod root;
pub mod session;
pub mod surface;
