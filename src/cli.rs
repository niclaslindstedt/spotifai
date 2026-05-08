//! `spotifai` CLI surface (clap-derived).
//!
//! Subcommands break down into two categories:
//!
//! - `install` provisions the pinned zad binary into
//!   `~/.spotifai/bin/zad` and scaffolds the read-only permissions file.
//! - `auth`, `api`, and `ask` are forwarders. They each call
//!   [`crate::install::ensure_installed`] first, then exec the managed
//!   zad binary (or hand control to zag for `ask`). `auth` registers
//!   credentials at zad's global scope; `api` runs `zad spotify …`
//!   with `ZAD_PERMISSIONS_PATH` pinned to spotifai's policy file.

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{api, ask, auth, install, output, permissions};

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

    /// Register Spotify OAuth credentials by forwarding to
    /// `zad service create spotify` (global scope, no `--local`).
    ///
    /// Spotify only issues one developer app per user, so the
    /// resulting `client_id` + refresh token are stored at
    /// `~/.zad/services/spotify/...` and apply to every directory
    /// `spotifai api …` is invoked from. Anything after `auth` is
    /// passed through verbatim to zad — `--client-id`,
    /// `--no-browser`, `--non-interactive`, etc.
    Auth(AuthArgs),
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

#[derive(Debug, clap::Args)]
pub struct AuthArgs {
    /// Arguments forwarded as-is to `zad service create spotify`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
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
        Some(Command::Install(args)) => guided_install(args.force),
        Some(Command::Api(args)) => api::forward(&args.args),
        Some(Command::Ask(args)) => {
            let query = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            ask::run(query.as_deref())
        }
        Some(Command::Auth(args)) => auth::run(&args.args),
    }
}

/// Walk the user through the four steps that make `spotifai api …`
/// usable: install zad, mint the signing key, scaffold the policy
/// file, sign it. Each step prints a header so a first-time user can
/// see what is happening.
fn guided_install(force: bool) -> Result<()> {
    output::header("spotifai setup");

    output::header("Step 1/4 · Installing zad binary");
    let zad = install::ensure_installed(force)?;

    output::header("Step 2/4 · Bootstrapping signing key");
    match install::bootstrap_signing_key(&zad)? {
        Some(fp) => output::status(&format!("signing key ready (fingerprint: {fp})")),
        None => output::status("signing key ready"),
    }

    output::header("Step 3/4 · Writing default permissions");
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

    output::header("Step 4/4 · Signing permissions file");
    install::sign_permissions_file(&zad, &path)?;
    output::status(&format!("signed {}", path.display()));

    output::info("");
    output::info("You're set up. Next:");
    output::info("  • Register Spotify credentials:  spotifai auth");
    output::info("  • Try a read-only API call:      spotifai api playlists list");
    output::info("  • Or chat with the agent:        spotifai ask \"list my playlists\"");
    Ok(())
}
