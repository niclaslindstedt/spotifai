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
//! - `ask`, `playlist`, and `clean` open interactive zag sessions
//!   backed by per-profile permissions files and a system prompt
//!   that injects the active policy. `clean` is the destructive
//!   surface — it strips `search` and the creator verbs and adds
//!   `playlists delete|remove` and the library-side unsave/unlike
//!   verbs.
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
use crate::{
    api, ask, auth, clean, commands_index, export, help_agent, import, install, logging, manpages,
    output, permissions, playlist, topic_docs,
};

#[derive(Debug, Parser)]
#[command(name = "spotifai", version, about, long_about = None)]
pub struct Cli {
    /// Echo `debug`-level diagnostic messages to stderr in addition
    /// to the always-on `debug.log` (§19.3). The file log captures
    /// `debug` regardless of this flag — see
    /// [`crate::logging::path`] for its location.
    #[arg(long, global = true)]
    pub debug: bool,

    /// Print a compact, prompt-injectable description of spotifai
    /// suitable for splicing into an LLM prompt via command
    /// substitution (§12.1). Prints to stdout and exits 0.
    #[arg(long, global = true)]
    pub help_agent: bool,

    /// Print a compact troubleshooting context block — log paths,
    /// config locations, env vars, common failure modes — for
    /// prompt injection into a debugging session (§12.2). Prints to
    /// stdout and exits 0.
    #[arg(long, global = true)]
    pub debug_agent: bool,

    /// When the active provider is in a 429 cooldown window (deadline
    /// persisted at `~/.zad/state/<service>/rate_limit.json` by zad
    /// 0.8.0), sleep until the deadline and continue instead of
    /// failing fast. Safe to leave on permanently — it is a no-op
    /// when no cooldown is recorded. The interactive surfaces
    /// (`spotifai ask`, `spotifai playlist`, `spotifai clean`) set
    /// `SPOTIFAI_WAIT=1` for the child `spotifai api` shells so
    /// sub-agents inherit it automatically. `--no-wait` overrides
    /// the env var.
    #[arg(long, global = true, overrides_with = "no_wait")]
    pub wait: bool,

    /// Force fail-fast behaviour on an active rate-limit window even
    /// when `SPOTIFAI_WAIT=1` is set in the environment. Mutually
    /// exclusive with `--wait`.
    #[arg(long = "no-wait", global = true, overrides_with = "wait")]
    pub no_wait: bool,

    /// Run the underlying zag agent with maximum permissions — i.e.
    /// skip every tool-call permission prompt. Only affects the
    /// interactive surfaces (`ask`, `playlist`, `clean`). The
    /// spotifai `(provider, profile)` permissions file is still
    /// enforced by `spotifai api` at the zad layer; `--yolo` only
    /// suppresses zag's per-tool approval gating on top of that.
    #[arg(long, global = true)]
    pub yolo: bool,

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
    /// Music). Requires a parent command (`ask`, `playlist`, or
    /// `clean`) to have selected a profile via `SPOTIFAI_PROFILE` —
    /// direct shell invocations exit with a usage error. The active
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

    /// Start an interactive zag session for cleaning up the user's
    /// library on the active provider.
    ///
    /// Loads `~/.spotifai/permissions/<provider>/clean.toml`, which
    /// allows the destructive verbs (`playlists delete`,
    /// `playlists remove`, `library tracks/albums unsave` on
    /// Spotify, `library unlike` on YouTube Music) plus the read
    /// verbs needed to enumerate candidates. `search` and the
    /// creator verbs (`playlists create|add|rename`,
    /// `library save|like`) are denied. The system prompt requires
    /// the agent to enumerate the candidate set, show it to the
    /// user, and wait for explicit confirmation before every
    /// destructive call. The optional positional argument becomes
    /// the agent's first turn.
    Clean(CleanArgs),

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

    /// Machine-readable command index (§12.4). With no argument,
    /// prints every command and its usage signature, one per line.
    /// With a `<name>` argument, prints the full usage spec for
    /// that command. With `--examples`, prints realistic example
    /// invocations instead of (or scoped to) the usage spec.
    Commands(CommandsArgs),

    /// Print an embedded reference manpage (§12.3). With no
    /// argument, lists every command that has a manpage. With a
    /// `<command>` argument, prints `man/<command>.md`.
    Man(ManArgs),

    /// Print an embedded conceptual doc (§12.3). With no argument,
    /// lists every available topic. With a `<topic>` argument,
    /// prints `docs/<topic>.md`.
    Docs(DocsArgs),
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
pub struct CleanArgs {
    /// Backing music provider whose library to clean up
    /// (default: `spotify`).
    #[arg(long, value_enum, default_value_t = ProviderArg::default())]
    pub provider: ProviderArg,

    /// Optional cleanup brief. Joined with spaces and used as the
    /// agent's first turn (e.g. `"remove all baby songs — my
    /// child is 15 now"`). Omit to drop straight into the
    /// interactive session with no opener.
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

    /// Include liked songs / liked videos in the exported envelope.
    /// When no `--likes` / `--albums` / `--playlists` flag is
    /// passed, every bucket is exported (backwards-compatible
    /// default). When any of the three is set, only the selected
    /// buckets are fetched — unselected ones are emitted as empty
    /// arrays. Handy for debugging one surface at a time.
    #[arg(long)]
    pub likes: bool,

    /// Include saved albums in the exported envelope. No-op on
    /// providers without a saved-albums concept (YouTube Music
    /// emits an empty `albums` array regardless). See `--likes`
    /// for the selection-flag interaction.
    #[arg(long)]
    pub albums: bool,

    /// Include playlists (and each playlist's full ordered track
    /// list) in the exported envelope. See `--likes` for the
    /// selection-flag interaction.
    #[arg(long)]
    pub playlists: bool,
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

    /// Ignore any saved progress under `~/.spotifai/import-state/`
    /// for this envelope and start the import from scratch. Useful
    /// when an earlier run left stale state behind (e.g. after
    /// manually deleting playlists on the target). Without this
    /// flag, re-running the same `spotifai import` command resumes
    /// from where the previous run was interrupted.
    #[arg(long)]
    pub no_resume: bool,
}

#[derive(Debug, clap::Args)]
pub struct CommandsArgs {
    /// Command name to look up. Without this argument, every
    /// command is listed.
    pub name: Option<String>,

    /// Print realistic example invocations instead of the usage
    /// spec. Combine with `<name>` to scope to one command.
    #[arg(long)]
    pub examples: bool,
}

#[derive(Debug, clap::Args)]
pub struct ManArgs {
    /// Command whose embedded manpage to print. With no argument,
    /// lists the available manpages.
    pub command: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct DocsArgs {
    /// Topic whose embedded conceptual doc to print. With no
    /// argument, lists the available topics.
    pub topic: Option<String>,
}

/// Entry point invoked by `main.rs`.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Err(e) = logging::init(cli.debug) {
        // Don't fail the command just because the log file is
        // unreachable — surface it once on stderr and continue. The
        // output module itself is still usable because every helper
        // double-writes to stderr regardless of the subscriber state.
        output::warn(&format!("logging disabled: {e:#}"));
    }
    // §12.1 / §12.2 — agent-prompt-injectable surfaces. Honored
    // anywhere on the command line (they're `global = true`),
    // including without a subcommand.
    if cli.help_agent {
        help_agent::print_help_agent();
        return Ok(());
    }
    if cli.debug_agent {
        help_agent::print_debug_agent();
        return Ok(());
    }
    // For one-shot commands (`api`, `export`, `import`) the default
    // is fail-fast — they are user-driven and a hard error is more
    // useful than a long silent sleep. For the interactive agent
    // surfaces (`ask`, `playlist`) the default is wait=true because
    // those spawn sub-agents that hammer `spotifai api` in parallel
    // and any one of them tripping a 429 would otherwise abort the
    // session. Both behaviours are overridden by explicit
    // `--wait` / `--no-wait` on the command line, or by setting
    // `SPOTIFAI_WAIT=1` / `=0` in the environment.
    let wait_oneshot = resolve_wait_flag(cli.wait, cli.no_wait, false);
    let wait_session = resolve_wait_flag(cli.wait, cli.no_wait, true);
    match cli.command {
        None => {
            output::plain(&format!("spotifai {}", crate::version()));
            output::plain(&format!("zad library {}", zad::version()));
            output::plain("");
            output::plain("Run `spotifai --help` for available commands.");
            Ok(())
        }
        Some(Command::Install(_)) => guided_install(),
        Some(Command::Api(args)) => api::forward(&args.args, wait_oneshot),
        Some(Command::Ask(args)) => {
            let query = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            ask::run(
                args.provider.into_provider(),
                query.as_deref(),
                wait_session,
                cli.yolo,
            )
        }
        Some(Command::Playlist(args)) => {
            let query = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            playlist::run(
                args.provider.into_provider(),
                query.as_deref(),
                wait_session,
                cli.yolo,
            )
        }
        Some(Command::Clean(args)) => {
            let query = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            clean::run(
                args.provider.into_provider(),
                query.as_deref(),
                wait_session,
                cli.yolo,
            )
        }
        Some(Command::Auth(args)) => auth::run(args.provider.into_provider(), &args.args),
        Some(Command::Export(args)) => export::run(
            args.provider.into_provider(),
            args.output.as_deref(),
            args.pretty,
            export::Selection::from_flags(args.likes, args.albums, args.playlists),
            wait_oneshot,
        ),
        Some(Command::Import(args)) => import::run(
            args.provider.into_provider(),
            args.input.as_deref(),
            args.dry_run,
            wait_oneshot,
            args.no_resume,
        ),
        Some(Command::Commands(args)) => commands_index::run(args.name.as_deref(), args.examples),
        Some(Command::Man(args)) => manpages::run(args.command.as_deref()),
        Some(Command::Docs(args)) => topic_docs::run(args.topic.as_deref()),
    }
}

/// Combine the CLI `--wait` / `--no-wait` flags with the
/// [`crate::zad_client::SPOTIFAI_WAIT_ENV`] environment variable into
/// the single boolean the rest of the codebase consults.
///
/// Resolution order:
///
/// 1. `--no-wait` on the command line → always `false`.
/// 2. `--wait` on the command line → always `true`.
/// 3. Otherwise, defer to `SPOTIFAI_WAIT` in the environment.
/// 4. Otherwise, fall back to `default_wait` (caller-supplied — the
///    interactive surfaces pick `true` to keep sub-agents coordinated,
///    one-shot commands pick `false` for fail-fast behaviour).
pub fn resolve_wait_flag(wait_cli: bool, no_wait_cli: bool, default_wait: bool) -> bool {
    let cli = if no_wait_cli {
        Some(false)
    } else if wait_cli {
        Some(true)
    } else {
        None
    };
    crate::zad_client::wait_mode_with_default(cli, default_wait)
}

/// Walk the user through the three steps that make spotifai's
/// agent surface usable: mint the signing key, scaffold every
/// permission profile (per provider), sign each one. Each step
/// prints a header so a first-time user can see what is happening.
fn guided_install() -> Result<()> {
    let _setup = output::section("spotifai setup", "install");

    output::step(1, 3, "bootstrapping signing key");
    {
        let _scope = output::scope("signing");
        match install::bootstrap_signing_key()? {
            Some(fp) => output::status(&format!("signing key ready (fingerprint: {fp})")),
            None => output::status("signing key ready"),
        }
    }

    output::step(2, 3, "writing default permission profiles");
    let paths = {
        let _scope = output::scope("permissions");
        let mut paths: Vec<(Provider, Profile, std::path::PathBuf)> =
            Vec::with_capacity(Provider::ALL.len() * Profile::ALL.len());
        for &provider in Provider::ALL {
            for &profile in Profile::ALL {
                let (path, wrote) = permissions::ensure_default_for(provider, profile)?;
                let label = format!("{} × {}", provider.as_str(), profile.as_str());
                if wrote {
                    output::status(&format!("wrote default {label} at {}", path.display()));
                } else {
                    output::detail(&format!("{label} already present at {}", path.display()));
                }
                paths.push((provider, profile, path));
            }
        }
        paths
    };

    output::step(3, 3, "signing permission profiles");
    {
        let _scope = output::scope("sign");
        for (provider, profile, path) in &paths {
            install::sign_permissions_file(*provider, path)?;
            output::status(&format!(
                "signed {} × {} at {}",
                provider.as_str(),
                profile.as_str(),
                path.display()
            ));
        }
    }

    output::newline();
    output::info("You're set up. Next:");
    output::detail("Register Spotify credentials:        spotifai auth");
    output::detail("Or YouTube Music credentials:        spotifai auth --provider ymusic");
    output::detail("Try a read-only API call:            spotifai ask \"list my playlists\"");
    output::detail(
        "Build a new Spotify playlist:        spotifai playlist \"a 30-min chill playlist\"",
    );
    output::detail(
        "Build a YouTube Music playlist:      spotifai playlist --provider ymusic \"a 30-min chill playlist\"",
    );
    output::detail(
        "Clean up your library:               spotifai clean \"remove every saved album from before 2010\"",
    );
    Ok(())
}
