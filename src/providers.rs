//! Per-provider metadata and defaults.
//!
//! `spotifai` was originally Spotify-only, but the same shell + agent
//! pattern works against any music backend zad ships a service for.
//! Today we support **Spotify** and **YouTube Music** (zad ≥ 0.6.0);
//! the abstraction here is sized so a third provider — Tidal, Apple
//! Music, anything else zad picks up — drops in by adding one
//! [`Provider`] variant and one entry in each `match` below.
//!
//! What every provider contributes:
//!
//! - A canonical slug used as the CLI flag value, the env-var value
//!   for [`crate::api::SPOTIFAI_PROVIDER_ENV`], and the directory name
//!   under `~/.spotifai/permissions/<slug>/`.
//! - The matching zad subcommand (`zad spotify …` / `zad ymusic …`)
//!   and the zad service slug consumed by `zad service create …`.
//! - A human-readable display name for prompts and CLI banners.
//! - A default permissions policy per profile
//!   ([`crate::permissions::Profile`]).
//! - A prompt example block — provider-specific `spotifai api`
//!   invocations the LLM agent is expected to use.
//!
//! Aside from a handful of cross-references in the docs and manpages,
//! the rest of the codebase routes through this module rather than
//! hard-coding `spotify` / `ymusic` strings, so additional providers
//! land without touching `cli.rs`, `api.rs`, `auth.rs`, etc.

use crate::permissions::{MODE_PLAYLIST_CURATOR, MODE_READ_ONLY, Permissions, Profile};

/// Identifier for the music provider an agent surface targets.
///
/// One variant per supported backend. Adding another provider takes:
///
/// 1. A new variant.
/// 2. New rows in [`Provider::ALL`], [`Provider::as_str`],
///    [`Provider::parse`], [`Provider::display_name`], and the
///    `zad_*` accessors.
/// 3. A new arm in [`Provider::default_policy`] (typically
///    delegating to a per-provider `*_default` helper below).
/// 4. A new arm in [`Provider::api_examples`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Spotify,
    YouTubeMusic,
}

impl Provider {
    /// Every provider spotifai knows about. The install flow iterates
    /// this so each provider's directory is scaffolded and signed.
    pub const ALL: &'static [Provider] = &[Provider::Spotify, Provider::YouTubeMusic];

    /// Default provider when no `--provider` flag is passed. Spotify
    /// is the original surface and remains the default to avoid
    /// breaking existing scripts when new providers are added.
    pub const DEFAULT: Provider = Provider::Spotify;

    /// Stable string used as the CLI flag value, the directory name
    /// under `~/.spotifai/permissions/`, and the value of the
    /// `SPOTIFAI_PROVIDER` env var.
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Spotify => "spotify",
            Provider::YouTubeMusic => "ymusic",
        }
    }

    /// Inverse of [`Provider::as_str`]. Accepts the canonical slugs
    /// plus a small set of aliases that users are likely to type.
    /// Returns `None` for unknown values so callers can render a
    /// clear error rather than silently falling back to the default.
    pub fn parse(s: &str) -> Option<Provider> {
        match s {
            "spotify" => Some(Provider::Spotify),
            "ymusic" | "youtube-music" | "youtube_music" | "ytmusic" => {
                Some(Provider::YouTubeMusic)
            }
            _ => None,
        }
    }

    /// Subcommand passed to zad when forwarding `spotifai api …`
    /// calls (`zad spotify …` / `zad ymusic …`).
    pub fn zad_subcommand(self) -> &'static str {
        // Today this matches [`Provider::as_str`], but conceptually
        // it is a separate axis — keep it independent so a future
        // renamed service can be tracked at one call site only.
        match self {
            Provider::Spotify => "spotify",
            Provider::YouTubeMusic => "ymusic",
        }
    }

    /// Service slug passed to `zad service create <slug>`.
    pub fn zad_service_slug(self) -> &'static str {
        match self {
            Provider::Spotify => "spotify",
            Provider::YouTubeMusic => "ymusic",
        }
    }

    /// Flag name `zad <provider> playlists create` accepts for the
    /// new playlist's display name. Spotify takes `--name`; YouTube
    /// Music takes `--title`. Kept on the [`Provider`] enum so
    /// [`crate::import`] stays provider-agnostic.
    pub fn playlist_name_flag(self) -> &'static str {
        match self {
            Provider::Spotify => "--name",
            Provider::YouTubeMusic => "--title",
        }
    }

    /// Human-readable provider name used in agent prompts and CLI
    /// banners. Capitalised and spelled the way the upstream service
    /// markets itself.
    pub fn display_name(self) -> &'static str {
        match self {
            Provider::Spotify => "Spotify",
            Provider::YouTubeMusic => "YouTube Music",
        }
    }

    /// Default policy for this `(provider, profile)` pair.
    pub fn default_policy(self, profile: Profile) -> Permissions {
        match self {
            Provider::Spotify => spotify_default(profile),
            Provider::YouTubeMusic => ymusic_default(profile),
        }
    }

    /// Provider-specific prompt example block. Substituted into the
    /// `{{ provider_examples }}` placeholder in the system prompts.
    /// Each line shows a `spotifai api …` invocation the agent is
    /// expected to use; the surrounding sentence in the prompt frames
    /// it ("the calls you will need most often").
    pub fn api_examples(self) -> &'static str {
        match self {
            Provider::Spotify => SPOTIFY_API_EXAMPLES,
            Provider::YouTubeMusic => YMUSIC_API_EXAMPLES,
        }
    }
}

/// Spotify default policy seeds, dispatched from
/// [`Provider::default_policy`].
pub fn spotify_default(profile: Profile) -> Permissions {
    match profile {
        Profile::Ask => Permissions {
            mode: MODE_READ_ONLY.to_string(),
            description:
                "Read-only access to the Spotify library. The agent may search the catalogue \
                 and read playlists, saved tracks, and saved albums, but must not create, \
                 modify, or delete anything."
                    .to_string(),
            allowed: vec![
                "search".into(),
                "playlists list".into(),
                "playlists show".into(),
                "library tracks list".into(),
                "library albums list".into(),
            ],
            denied: vec![
                "playlists create".into(),
                "playlists rename".into(),
                "playlists delete".into(),
                "playlists add".into(),
                "playlists remove".into(),
                "library tracks save".into(),
                "library tracks unsave".into(),
                "library albums save".into(),
                "library albums unsave".into(),
            ],
        },
        Profile::Playlist => Permissions {
            mode: MODE_PLAYLIST_CURATOR.to_string(),
            description:
                "Curate new playlists for the user. The agent may search the catalogue, read \
                 the user's existing playlists and library, create a new playlist, add tracks \
                 to it, and rename it. The agent must not delete playlists, remove tracks from \
                 playlists, or modify the user's saved library."
                    .to_string(),
            allowed: vec![
                "search".into(),
                "playlists list".into(),
                "playlists show".into(),
                "playlists create".into(),
                "playlists add".into(),
                "playlists rename".into(),
                "library tracks list".into(),
                "library albums list".into(),
            ],
            denied: vec![
                "playlists delete".into(),
                "playlists remove".into(),
                "library tracks save".into(),
                "library tracks unsave".into(),
                "library albums save".into(),
                "library albums unsave".into(),
            ],
        },
    }
}

/// YouTube Music default policy seeds, dispatched from
/// [`Provider::default_policy`]. The verb surface mirrors zad's
/// `ymusic` subcommand: a single `library list` covers rated videos
/// (the YouTube Data API has no "saved albums" concept) and library
/// writes are the `like` / `unlike` pair.
pub fn ymusic_default(profile: Profile) -> Permissions {
    match profile {
        Profile::Ask => Permissions {
            mode: MODE_READ_ONLY.to_string(),
            description:
                "Read-only access to the user's YouTube Music data. The agent may search the \
                 catalogue and read playlists and the user's rated videos, but must not \
                 create, modify, or delete anything."
                    .to_string(),
            allowed: vec![
                "search".into(),
                "playlists list".into(),
                "playlists show".into(),
                "library list".into(),
            ],
            denied: vec![
                "playlists create".into(),
                "playlists rename".into(),
                "playlists delete".into(),
                "playlists add".into(),
                "playlists remove".into(),
                "library like".into(),
                "library unlike".into(),
            ],
        },
        Profile::Playlist => Permissions {
            mode: MODE_PLAYLIST_CURATOR.to_string(),
            description:
                "Curate new YouTube Music playlists for the user. The agent may search the \
                 catalogue, read the user's existing playlists and rated videos, create a new \
                 playlist, add videos to it, and rename it. The agent must not delete \
                 playlists, remove videos from playlists, or like/unlike videos in the user's \
                 library."
                    .to_string(),
            allowed: vec![
                "search".into(),
                "playlists list".into(),
                "playlists show".into(),
                "playlists create".into(),
                "playlists add".into(),
                "playlists rename".into(),
                "library list".into(),
            ],
            denied: vec![
                "playlists delete".into(),
                "playlists remove".into(),
                "library like".into(),
                "library unlike".into(),
            ],
        },
    }
}

const SPOTIFY_API_EXAMPLES: &str = "\
- `spotifai api search \"moon river\"` — search the catalogue
- `spotifai api search \"kind of blue\" --type album --type artist`
- `spotifai api playlists list --json` — list all of the user's playlists
- `spotifai api playlists show <playlist-id-or-name> --json`
- `spotifai api playlists create --name \"<name>\" --json` — create a new playlist (write profiles only)
- `spotifai api playlists add <playlist-id> <track-id> [<track-id>…] --json` — populate a playlist (write profiles only)
- `spotifai api library tracks list --limit 50 --json`
- `spotifai api library albums list --limit 50 --json`";

const YMUSIC_API_EXAMPLES: &str = "\
- `spotifai api search \"moon river\"` — search the catalogue
- `spotifai api search \"kind of blue\" --type playlist`
- `spotifai api playlists list --json` — list all of the user's playlists
- `spotifai api playlists show <playlist-id> --json`
- `spotifai api playlists create --title \"<title>\" --json` — create a new playlist (write profiles only)
- `spotifai api playlists add <playlist-id> <video-id> [<video-id>…] --json` — populate a playlist (write profiles only)
- `spotifai api library list --limit 50 --json` — list the user's rated videos";
