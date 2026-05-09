//! `spotifai` CLI surface (clap-derived).
//!
//! Subcommands:
//!
//! - `install` mints the per-machine signing key and scaffolds +
//!   signs one permissions file per `(provider, profile)` pair
//!   under `~/.spotifai/permissions/<provider>/`.
//! - `auth` runs the in-process OAuth loopback flow for the active
//!   provider and writes the resulting tokens into the OS keychain.
//! - `api` parses the user-args grammar into typed zad library
//!   calls and prints JSON to stdout.
//! - `ask` and `playlist` open interactive zag sessions backed by
//!   per-profile permissions files and a system prompt that injects
//!   the active policy.
//! - `export` and `import` round-trip the user's library through
//!   the unified spotifai schema (see `docs/export_schema.md`).
//!
//! Every user-facing command takes `--provider <slug>` (default:
//! `spotify`). Adding a new provider is a single change in
//! [`crate::providers`] — the CLI surface picks it up automatically
//! through clap's [`clap::ValueEnum`] derive on [`ProviderArg`].

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::permissions::Profile;
use crate::providers::Provider;
use crate::{api, ask, auth, export, import, install, output, permissions, playlist};

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
    /// Mint the per-machine signing key, scaffold every permission
    /// profile for every supported provider, and sign each one.
    /// Idempotent — safe to re-run.
    Install(InstallArgs),

    /// Run a typed call against the active provider through the
    /// in-process zad library and print the JSON response.
    ///
    /// Grammar: `search "query"`, `playlists list`, `playlists show
    /// <id>`, `playlists create --name|--title <name>`, `playlists
    /// add <playlist-id> <id…>`, `library tracks list`,
    /// `library albums list` (Spotify), `library list` (YouTube
    /// Music). Requires a parent command (`ask` or `playlist`) to
    /// have selected a profile via `SPOTIFAI_PROFILE` — direct
    /// shell invocations exit with a usage error. The active
    /// provider is read from `SPOTIFAI_PROVIDER` (default: spotify).
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

    /// Run an in-process OAuth loopback flow for the active
    /// provider and write the resulting tokens into the OS
    /// keychain.
    ///
    /// Spotify uses PKCE (no `client_secret`); YouTube Music uses a
    /// Google OAuth 2.0 "Desktop app" client (with a
    /// `client_secret`). Both flows open the browser at the
    /// authorize URL and capture the redirect on a `127.0.0.1`
    /// loopback listener. Pass `--client-id` / `--client-secret`
    /// to skip the interactive prompt; `--no-browser` keeps the URL
    /// in stderr only.
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

    /// Recreate playlists from a `spotifai export` envelope on the
    /// active provider.
    ///
    /// Reads the envelope from stdin by default (so `spotifai export
    /// | spotifai import --provider …` works), or from `--input
    /// PATH`. When `source.service` in the envelope differs from the
    /// target `--provider`, each track is resolved on the target via
    /// the typed `search` request on the zad library (ISRC first on
    /// Spotify, then title + primary artist). Unresolvable items
    /// are skipped and reported. Playlists whose name already
    /// exists on the target are skipped with a warning. Liked
    /// tracks, liked videos, and saved albums in the envelope are
    /// intentionally ignored — only **playlists** are recreated.
    /// Reuses the `playlist` permission profile.
    Import(ImportArgs),
}

#[derive(Debug, clap::Args)]
pub struct InstallArgs {}

#[derive(Debug, clap::Args)]
pub struct ApiArgs {
    /// Verb plus arguments parsed by [`crate::api::parse_verb`].
    /// The active provider is read from `SPOTIFAI_PROVIDER` (set by
    /// the parent `ask`/`playlist`/`export` command) — `spotifai
    /// api` does not take its own `--provider` flag because
    /// trailing-var-arg parsing would swallow it before clap saw
    /// it.
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

    /// Optional flags: `--client-id`, `--client-secret` (YouTube
    /// Music only), `--no-browser`. Anything else errors out.
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

#[derive(Debug, clap::Args)]
pub struct ImportArgs {
    /// Provider to import the playlists onto (default: `spotify`).
    #[arg(long, value_enum, default_value_t = ProviderArg::default())]
    pub provider: ProviderArg,

    /// Read the envelope from this file instead of stdin.
    #[arg(long, short = 'i')]
    pub input: Option<std::path::PathBuf>,

    /// Print what would be created without making any zad write
    /// calls. Still spawns zad for the duplicate-name pre-fetch and
    /// for cross-provider track resolution so the preview is
    /// realistic.
    #[arg(long)]
    pub dry_run: bool,
}

/// Entry point invoked by `main.rs`.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            println!("spotifai {}", crate::version());
            println!("zad library {}", zad::version());
            println!("\nRun `spotifai --help` for available commands.");
            Ok(())
        }
        Some(Command::Install(_)) => guided_install(),
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
        Some(Command::Import(args)) => import::run(
            args.provider.into_provider(),
            args.input.as_deref(),
            args.dry_run,
        ),
    }
}

/// Walk the user through the three steps that make spotifai's
/// agent surface usable: mint the signing key, scaffold every
/// permission profile (per provider), sign each one. Each step
/// prints a header so a first-time user can see what is happening.
fn guided_install() -> Result<()> {
    output::header("spotifai setup");

    output::header("Step 1/3 · Bootstrapping signing key");
    match install::bootstrap_signing_key()? {
        Some(fp) => output::status(&format!("signing key ready (fingerprint: {fp})")),
        None => output::status("signing key ready"),
    }

    output::header("Step 2/3 · Writing default permission profiles");
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

    output::header("Step 3/3 · Signing permission profiles");
    for (provider, profile, path) in &paths {
        install::sign_permissions_file(*provider, path)?;
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
