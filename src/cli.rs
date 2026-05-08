//! `spotifai` CLI surface (clap-derived).
//!
//! Today the only user-facing subcommand is `install`, which fetches the
//! pinned zad release into `~/.spotifai/bin/zad`. Forward-routing
//! subcommands (`spotifai api …` → `zad spotify …`) will live alongside
//! it and call [`crate::install::ensure_installed`] before exec'ing zad.

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::install;

#[derive(Debug, Parser)]
#[command(name = "spotifai", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ensure the pinned zad binary is installed at `~/.spotifai/bin/zad`.
    ///
    /// Reads the target tag from `.zadrc` baked in at build time.
    /// Idempotent: a no-op when the existing binary already reports
    /// the pinned version. Pass `--force` to re-download anyway.
    Install(InstallArgs),
}

#[derive(Debug, clap::Args)]
pub struct InstallArgs {
    /// Re-download even if the existing binary already matches the pinned version.
    #[arg(long)]
    pub force: bool,
}

/// Entry point invoked by `main.rs`.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            println!("spotifai {}", crate::version());
            println!("zad pinned to {}", install::pinned_version());
            println!("\nRun `spotifai --help` for available commands.");
            Ok(())
        }
        Some(Command::Install(args)) => {
            install::ensure_installed(args.force)?;
            Ok(())
        }
    }
}
