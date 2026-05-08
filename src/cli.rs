//! `spotifai` CLI surface (clap-derived).
//!
//! Subcommands break down into two categories:
//!
//! - `install` provisions the pinned zad binary into
//!   `~/.spotifai/bin/zad` and scaffolds + signs one permissions file
//!   per profile under `~/.spotifai/permissions/`.
//! - `auth`, `api`, `ask`, and `playlist` are forwarders. They each
//!   call [`crate::install::ensure_installed`] first, then exec the
//!   managed zad binary (or hand control to zag for `ask` /
//!   `playlist`). `auth` registers credentials at zad's global scope;
//!   `api` runs `zad spotify …` with `ZAD_PERMISSIONS_PATH` pinned to
//!   the policy file backing the active profile.

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::permissions::Profile;
use crate::{api, ask, auth, export, install, output, permissions, playlist};

#[derive(Debug, Parser)]
#[command(name = "spotifai", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ensure the pinned zad binary is installed at `~/.spotifai/bin/zad`,
    /// scaffold every permission profile, and sign each one.
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
    /// `~/.spotifai/bin/zad spotify playlists list`. Requires a
    /// parent spotifai command (`ask` or `playlist`) to have selected
    /// a profile — direct shell invocations exit with a usage error.
    Api(ApiArgs),

    /// Start an interactive zag session pre-loaded with a system
    /// prompt that tells the agent how to use `spotifai api …` and
    /// injects `~/.spotifai/permissions/ask.toml` so the agent
    /// self-restricts to the verbs the user has allowed.
    ///
    /// The optional positional argument becomes the agent's first
    /// turn; with no argument the session opens empty and waits for
    /// the user to type.
    Ask(AskArgs),

    /// Start an interactive zag session that helps the user build a
    /// new Spotify playlist conversationally.
    ///
    /// Loads `~/.spotifai/permissions/playlist.toml` so the agent
    /// can search the catalogue, create one new playlist, and add
    /// tracks to it — but cannot delete or remove anything. The
    /// optional positional argument becomes the agent's first turn.
    Playlist(PlaylistArgs),

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

    /// Dump the user's Spotify library — liked tracks, saved
    /// albums, and playlists with full track lists and ordering —
    /// into one structured JSON document.
    ///
    /// Designed to be portable enough to re-import on another
    /// music service later. Reuses the read-only `ask` permission
    /// profile. Defaults to stdout; `--output` redirects to a
    /// file. Status messages always go to stderr so the JSON on
    /// stdout stays pipe-clean.
    Export(ExportArgs),
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
pub struct PlaylistArgs {
    /// Optional brief. Joined with spaces and used as the agent's
    /// first turn (e.g. `"a 30-minute focus playlist with no
    /// vocals"`). Omit to drop straight into the interactive session
    /// with no opener.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub query: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct AuthArgs {
    /// Arguments forwarded as-is to `zad service create spotify`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct ExportArgs {
    /// Write the JSON document to this path instead of stdout.
    /// Parent directories are created if needed.
    #[arg(long, short = 'o')]
    pub output: Option<std::path::PathBuf>,

    /// Pretty-print the JSON with two-space indent. Without this
    /// flag the document is one dense line, which is what most
    /// downstream tooling (importers, diffs) prefers.
    #[arg(long)]
    pub pretty: bool,
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
        Some(Command::Playlist(args)) => {
            let query = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            playlist::run(query.as_deref())
        }
        Some(Command::Auth(args)) => auth::run(&args.args),
        Some(Command::Export(args)) => export::run(args.output.as_deref(), args.pretty),
    }
}

/// Walk the user through the four steps that make `spotifai api …`
/// usable: install zad, mint the signing key, scaffold every
/// permission profile, sign each one. Each step prints a header so a
/// first-time user can see what is happening.
fn guided_install(force: bool) -> Result<()> {
    output::header("spotifai setup");

    output::header("Step 1/4 · Installing zad binary");
    let zad = install::ensure_installed(force)?;

    output::header("Step 2/4 · Bootstrapping signing key");
    match install::bootstrap_signing_key(&zad)? {
        Some(fp) => output::status(&format!("signing key ready (fingerprint: {fp})")),
        None => output::status("signing key ready"),
    }

    output::header("Step 3/4 · Writing default permission profiles");
    let mut paths: Vec<(Profile, std::path::PathBuf)> = Vec::with_capacity(Profile::ALL.len());
    for &profile in Profile::ALL {
        let (path, wrote) = permissions::ensure_default_for(profile)?;
        if wrote {
            output::status(&format!(
                "wrote default {} permissions to {}",
                profile.as_str(),
                path.display()
            ));
        } else {
            output::info(&format!(
                "{} permissions already present at {}",
                profile.as_str(),
                path.display()
            ));
        }
        paths.push((profile, path));
    }

    output::header("Step 4/4 · Signing permission profiles");
    for (profile, path) in &paths {
        install::sign_permissions_file(&zad, path)?;
        output::status(&format!(
            "signed {} profile at {}",
            profile.as_str(),
            path.display()
        ));
    }

    output::info("");
    output::info("You're set up. Next:");
    output::info("  • Register Spotify credentials:  spotifai auth");
    output::info("  • Try a read-only API call:      spotifai ask \"list my playlists\"");
    output::info(
        "  • Build a new playlist:          spotifai playlist \"a 30-min chill playlist\"",
    );
    Ok(())
}
