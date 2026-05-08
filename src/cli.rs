//! `spotifai` CLI surface (clap-derived).
//!
//! Subcommands break down into two categories:
//!
//! - `install` provisions the pinned zad binary into
//!   `~/.spotifai/bin/zad` and scaffolds + signs one permissions
//!   file per `(provider, profile)` pair under
//!   `~/.spotifai/permissions/<provider>/`.
//! - `auth`, `api`, `ask`, `playlist`, and `export` are forwarders.
//!   They each call [`crate::install::ensure_installed`] first, then
//!   exec the managed zad binary (or hand control to zag for `ask` /
//!   `playlist`). `auth` registers credentials at zad's global scope
//!   for the active provider; `api` runs `zad <provider> …` with
//!   `ZAD_PERMISSIONS_PATH` pinned to the policy file backing the
//!   active `(provider, profile)` pair.
//!
//! Every user-facing command takes `--provider <slug>` (default:
//! `spotify`). Adding a new provider is a single change in
//! [`crate::providers`] — the CLI surface picks it up automatically
//! through clap's [`clap::ValueEnum`] derive on [`ProviderArg`].

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::permissions::Profile;
use crate::providers::Provider;
use crate::{api, ask, auth, export, install, output, permissions, playlist};

#[derive(Debug, Parser)]
#[command(name = "spotifai", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// CLI value-enum mirror of [`crate::providers::Provider`].
///
/// Kept as a thin wrapper so clap's `derive(ValueEnum)` machinery
/// stays inside `cli.rs` and `providers.rs` only owns the canonical
/// enum. `--provider` defaults to `spotify`; other providers (today
/// `ymusic`) are listed automatically as new variants are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ProviderArg {
    Spotify,
    Ymusic,
}

impl ProviderArg {
    /// Convert into the canonical [`Provider`] variant used by the
    /// rest of the codebase.
    pub fn into_provider(self) -> Provider {
        match self {
            ProviderArg::Spotify => Provider::Spotify,
            ProviderArg::Ymusic => Provider::YouTubeMusic,
        }
    }
}

impl Default for ProviderArg {
    fn default() -> Self {
        match Provider::DEFAULT {
            Provider::Spotify => ProviderArg::Spotify,
            Provider::YouTubeMusic => ProviderArg::Ymusic,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ensure the pinned zad binary is installed at
    /// `~/.spotifai/bin/zad`, scaffold every permission profile for
    /// every supported provider, and sign each one.
    ///
    /// Reads the target tag from `.zadrc` baked in at build time.
    /// Idempotent: a no-op when the existing binary already reports
    /// the pinned version. Pass `--force` to re-download anyway.
    Install(InstallArgs),

    /// Forward to `zad <provider> …` after verifying the pinned zad
    /// binary is installed.
    ///
    /// Everything after `api` is passed through verbatim, so
    /// `spotifai api playlists list` becomes
    /// `~/.spotifai/bin/zad <provider> playlists list`. Requires a
    /// parent spotifai command (`ask` or `playlist`) to have
    /// selected a profile via `SPOTIFAI_PROFILE` — direct shell
    /// invocations exit with a usage error. The active provider is
    /// read from `SPOTIFAI_PROVIDER` (default: spotify).
    Api(ApiArgs),

    /// Start an interactive zag session pre-loaded with a system
    /// prompt that tells the agent how to use `spotifai api …` and
    /// injects `~/.spotifai/permissions/<provider>/ask.toml` so the
    /// agent self-restricts to the verbs the user has allowed.
    ///
    /// The optional positional argument becomes the agent's first
    /// turn; with no argument the session opens empty and waits for
    /// the user to type.
    Ask(AskArgs),

    /// Start an interactive zag session that helps the user build a
    /// new playlist conversationally on the active provider.
    ///
    /// Loads `~/.spotifai/permissions/<provider>/playlist.toml` so
    /// the agent can search the catalogue, create one new playlist,
    /// and add tracks/videos to it — but cannot delete or remove
    /// anything. The optional positional argument becomes the
    /// agent's first turn.
    Playlist(PlaylistArgs),

    /// Register OAuth credentials by forwarding to `zad service
    /// create <provider>` (global scope, no `--local`).
    ///
    /// Spotify only issues one developer app per user, so the
    /// resulting `client_id` + refresh token are stored at
    /// `~/.zad/services/spotify/...` and apply to every directory
    /// `spotifai api …` is invoked from. YouTube Music uses Google
    /// OAuth 2.0 "Desktop app" credentials at
    /// `~/.zad/services/ymusic/...`. Anything after `auth` is
    /// passed through verbatim to zad — `--client-id`,
    /// `--client-secret`, `--no-browser`, `--non-interactive`, etc.
    Auth(AuthArgs),

    /// Dump the user's library on the active provider — liked
    /// tracks/videos, saved albums (Spotify only), and playlists
    /// with full track lists and ordering — into one structured
    /// JSON document.
    ///
    /// Designed to be portable enough to re-import on another music
    /// service later. Reuses the read-only `ask` permission profile.
    /// Defaults to stdout; `--output` redirects to a file. Status
    /// messages always go to stderr so the JSON on stdout stays
    /// pipe-clean.
    Export(ExportArgs),
}

#[derive(Debug, clap::Args)]
pub struct InstallArgs {
    /// Re-download even if the existing binary already matches the
    /// pinned version.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, clap::Args)]
pub struct ApiArgs {
    /// Arguments forwarded as-is to `zad <provider>`. The active
    /// provider is read from `SPOTIFAI_PROVIDER` (set by the parent
    /// `ask`/`playlist`/`export` command) — `spotifai api` does not
    /// take its own `--provider` flag because trailing-var-arg
    /// parsing would swallow it before clap saw it.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct AskArgs {
    /// Backing music provider to query (default: `spotify`).
    #[arg(long, value_enum, default_value_t = ProviderArg::default())]
    pub provider: ProviderArg,

    /// Optional question. Joined with spaces and used as the
    /// agent's first turn. Omit to drop straight into the
    /// interactive session with no opener.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub query: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct PlaylistArgs {
    /// Backing music provider the playlist will be created on
    /// (default: `spotify`).
    #[arg(long, value_enum, default_value_t = ProviderArg::default())]
    pub provider: ProviderArg,

    /// Optional brief. Joined with spaces and used as the agent's
    /// first turn (e.g. `"a 30-minute focus playlist with no
    /// vocals"`). Omit to drop straight into the interactive
    /// session with no opener.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub query: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct AuthArgs {
    /// Provider to register credentials for (default: `spotify`).
    #[arg(long, value_enum, default_value_t = ProviderArg::default())]
    pub provider: ProviderArg,

    /// Arguments forwarded as-is to `zad service create <provider>`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct ExportArgs {
    /// Provider whose library to export (default: `spotify`).
    #[arg(long, value_enum, default_value_t = ProviderArg::default())]
    pub provider: ProviderArg,

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
            ask::run(args.provider.into_provider(), query.as_deref())
        }
        Some(Command::Playlist(args)) => {
            let query = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            playlist::run(args.provider.into_provider(), query.as_deref())
        }
        Some(Command::Auth(args)) => auth::run(args.provider.into_provider(), &args.args),
        Some(Command::Export(args)) => export::run(
            args.provider.into_provider(),
            args.output.as_deref(),
            args.pretty,
        ),
    }
}

/// Walk the user through the four steps that make `spotifai api …`
/// usable: install zad, mint the signing key, scaffold every
/// permission profile (per provider), sign each one. Each step
/// prints a header so a first-time user can see what is happening.
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
    let mut paths: Vec<(Provider, Profile, std::path::PathBuf)> =
        Vec::with_capacity(Provider::ALL.len() * Profile::ALL.len());
    for &provider in Provider::ALL {
        for &profile in Profile::ALL {
            let (path, wrote) = permissions::ensure_default_for(provider, profile)?;
            if wrote {
                output::status(&format!(
                    "wrote default {} × {} permissions to {}",
                    provider.as_str(),
                    profile.as_str(),
                    path.display()
                ));
            } else {
                output::info(&format!(
                    "{} × {} permissions already present at {}",
                    provider.as_str(),
                    profile.as_str(),
                    path.display()
                ));
            }
            paths.push((provider, profile, path));
        }
    }

    output::header("Step 4/4 · Signing permission profiles");
    for (provider, profile, path) in &paths {
        install::sign_permissions_file(&zad, *provider, path)?;
        output::status(&format!(
            "signed {} × {} profile at {}",
            provider.as_str(),
            profile.as_str(),
            path.display()
        ));
    }

    output::info("");
    output::info("You're set up. Next:");
    output::info("  • Register Spotify credentials:        spotifai auth");
    output::info("  • Or YouTube Music credentials:        spotifai auth --provider ymusic");
    output::info("  • Try a read-only API call:            spotifai ask \"list my playlists\"");
    output::info(
        "  • Build a new Spotify playlist:        spotifai playlist \"a 30-min chill playlist\"",
    );
    output::info(
        "  • Build a new YouTube Music playlist:  spotifai playlist --provider ymusic \"a 30-min chill playlist\"",
    );
    Ok(())
}
