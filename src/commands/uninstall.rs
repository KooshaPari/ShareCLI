// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli uninstall` — document and optionally purge local data dirs (C11 L121).

use anyhow::Result;

use crate::paths::{purge_data_dirs, well_known_paths};

/// Print uninstall guidance and optionally purge config/state with `--purge-data`.
pub fn run(purge_data: bool, dry_run: bool) -> Result<()> {
    let paths = well_known_paths();
    println!("sharecli uninstall");
    println!();
    println!("Remove the binary with your package manager:");
    println!("  cargo uninstall sharecli");
    println!("  brew uninstall sharecli   # when installed from a tap");
    println!("  rm \"$(which sharecli)\"    # manual install only");
    println!();
    println!("Config and data locations:");
    println!("  config:  {}", paths.config_dir.display());
    println!("  state:   {}", paths.state_dir.display());
    println!("  runtime: {} (ephemeral locks)", paths.runtime_dir.display());

    if !purge_data {
        println!();
        println!("Re-run with --purge-data to delete config/state (and stale runtime locks).");
        println!("Add --dry-run to preview deletions without removing files.");
        return Ok(());
    }

    let targets = purge_data_dirs(true, dry_run)?;
    println!();
    if dry_run {
        println!("Dry run — would remove:");
        for path in &targets {
            if path.exists() {
                println!("  {}", path.display());
            }
        }
    } else {
        println!("Purged sharecli data directories.");
        for path in &targets {
            if !path.exists() {
                println!("  (skipped missing) {}", path.display());
            } else {
                println!("  removed {}", path.display());
            }
        }
    }
    Ok(())
}
