//! `spotifai` CLI surface (clap-derived).
//!
//! Today the only user-facing subcommand is `install`, which fetches the
//! pinned zad release into `~/.spotifai/bin/zad`. Forward-routing
//! subcommands (`spotifai api …` → `zad spotify …`) will live alongside
//! it and call [`crate::install::ensure_installed`] before exec'ing zad.

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{api, ask, install, output, permissions};

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

    /// Forward to `zad spotify …` after verifying the pinned zad
    /// binary is installed.
    ///
    /// Everything after `api` is passed through verbatim, so
    /// `spotifai api playlists list` becomes
    /// `~/.spotifai/bin/zad spotify playlists list`. The pinned
    /// version from `.zadrc` is checked (and downloaded if missing
    /// or stale) on every invocation.
    Api(ApiArgs),

    /// Start an interactive zag session pre-loaded with a system
    /// prompt that tells the agent how to use `spotifai api …` and
    /// injects `~/.spotifai/permissions.toml` so the agent
    /// self-restricts to the verbs the user has allowed.
    ///
    /// The optional positional argument becomes the agent's first
    /// turn; with no argument the session opens empty and waits for
    /// the user to type.
    Ask(AskArgs),
}

#[derive(Debug, clap::Args)]
pub struct InstallArgs {
    /// Re-download even if the existing binary already matches the pinned version.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, clap::Args)]
pub struct ApiArgs {
    /// Arguments forwarded as-is to `zad spotify`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct AskArgs {
    /// Optional question. Joined with spaces and used as the agent's
    /// first turn. Omit to drop straight into the interactive
    /// session with no opener.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub query: Vec<String>,
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
            let path = permissions::default_path()?;
            if permissions::ensure_default(&path)? {
                output::status(&format!(
                    "wrote default read-only permissions to {}",
                    path.display()
                ));
            } else {
                output::info(&format!(
                    "permissions already present at {}",
                    path.display()
                ));
            }
            Ok(())
        }
        Some(Command::Api(args)) => api::forward(&args.args),
        Some(Command::Ask(args)) => {
            let query = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            ask::run(query.as_deref())
        }
    }
}
