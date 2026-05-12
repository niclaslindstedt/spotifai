//! spotifai permission profiles
//! (`~/.spotifai/permissions/<provider>/<profile>.toml`).
//!
//! Each agent surface has its own TOML policy file, scoped per
//! backing music provider. `spotifai ask` injects the `ask` profile
//! into its system prompt; `spotifai playlist` injects the `playlist`
//! profile; `spotifai clean` injects the `clean` profile. The same
//! files are pointed at by `ZAD_PERMISSIONS_PATH` when the agent
//! shells out through `spotifai api`, so zad's load-time verification
//! gate sees the profile that matches the active surface.
//!
//! All three files ship with safe defaults — `ask` is read-only,
//! `playlist` adds the verbs needed to build a new playlist
//! end-to-end, and `clean` adds the destructive verbs needed to remove
//! existing playlists and saved items. Users can hand-edit any file,
//! then re-run `spotifai install` so the signing step picks up the
//! change. The
//! two layers (prompt-side verb list and zad's signed runtime gate)
//! serve different roles: the prompt keeps the agent from proposing
//! forbidden verbs in the first place; zad's verification fails
//! closed if the agent tries anyway.
//!
//! The provider axis (Spotify, YouTube Music, …) lives in
//! [`crate::providers`]; this module owns the [`Profile`] axis and
//! the on-disk file format, and delegates the per-provider verb
//! defaults to that module.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::providers::Provider;

/// Subdirectory under `~/.spotifai/` that holds the per-provider
/// permission directories.
pub const PERMISSIONS_DIR: &str = "permissions";

/// Mode tag stored in a read-only profile.
pub const MODE_READ_ONLY: &str = "read_only";

/// Mode tag stored in a playlist-curator profile.
pub const MODE_PLAYLIST_CURATOR: &str = "playlist_curator";

/// Mode tag stored in a library-cleanup profile.
pub const MODE_LIBRARY_CLEANUP: &str = "library_cleanup";

/// Identifier for one of spotifai's per-command permission profiles.
///
/// Each variant maps 1:1 to a TOML file under
/// `~/.spotifai/permissions/<provider>/` and to the command that
/// loads it (`ask` → `Profile::Ask`, `playlist` → `Profile::Playlist`,
/// `clean` → `Profile::Clean`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Ask,
    Playlist,
    Clean,
}

impl Profile {
    /// Every profile spotifai knows about. The install flow iterates
    /// this (per provider) to scaffold and sign one file per entry.
    pub const ALL: &'static [Profile] = &[Profile::Ask, Profile::Playlist, Profile::Clean];

    /// Stable string used as both the file stem and the value of the
    /// `SPOTIFAI_PROFILE` env var.
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Ask => "ask",
            Profile::Playlist => "playlist",
            Profile::Clean => "clean",
        }
    }

    /// Inverse of [`Profile::as_str`]. Returns `None` for unknown
    /// values so callers can render a "no profile selected" error
    /// rather than silently falling back. Named `parse` rather than
    /// `from_str` to avoid being confused with the
    /// `std::str::FromStr` trait method (which would force a
    /// different error type).
    pub fn parse(s: &str) -> Option<Profile> {
        match s {
            "ask" => Some(Profile::Ask),
            "playlist" => Some(Profile::Playlist),
            "clean" => Some(Profile::Clean),
            _ => None,
        }
    }

    /// In-memory default policy for the (`provider`, `self`) pair.
    /// Used when the on-disk file is missing and as the seed when
    /// scaffolding a fresh file.
    pub fn default_policy(self, provider: Provider) -> Permissions {
        provider.default_policy(self)
    }
}

/// Parsed contents of a `~/.spotifai/permissions/<provider>/<profile>.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions {
    /// Short tag (`read_only`, `playlist_curator`, …). Informational —
    /// the effective policy is the `allowed` / `denied` lists below.
    pub mode: String,
    /// Free-text description embedded in the system prompt so the
    /// agent can quote the policy back to the user verbatim.
    pub description: String,
    /// `spotifai api` verbs the agent is allowed to invoke. Each
    /// entry is the literal subcommand string after `spotifai api `
    /// (e.g. `playlists list`, `search`).
    #[serde(default)]
    pub allowed: Vec<String>,
    /// `spotifai api` verbs the agent must refuse to invoke.
    #[serde(default)]
    pub denied: Vec<String>,
}

impl Permissions {
    /// Render the policy as a Markdown block ready to paste into the
    /// agent's system prompt.
    pub fn as_prompt_block(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Mode: {}\n", self.mode));
        out.push_str(&format!("Policy: {}\n\n", self.description));
        out.push_str("Allowed `spotifai api` verbs:\n");
        if self.allowed.is_empty() {
            out.push_str("- (none)\n");
        } else {
            for v in &self.allowed {
                out.push_str(&format!("- `spotifai api {v}`\n"));
            }
        }
        out.push_str("\nDenied `spotifai api` verbs (refuse the user if asked):\n");
        if self.denied.is_empty() {
            out.push_str("- (none)\n");
        } else {
            for v in &self.denied {
                out.push_str(&format!("- `spotifai api {v}`\n"));
            }
        }
        out
    }
}

/// Resolve the per-(provider, profile) policy path:
/// `<dirs::home_dir()>/.spotifai/permissions/<provider>/<profile>.toml`.
pub fn path_for(provider: Provider, profile: Profile) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    let filename = format!("{}.toml", profile.as_str());
    Ok(home
        .join(".spotifai")
        .join(PERMISSIONS_DIR)
        .join(provider.as_str())
        .join(filename))
}

/// Serialize a [`Permissions`] to a TOML string with a leading
/// comment header so a curious user opening the file knows what it
/// is.
pub fn to_toml_string(p: &Permissions) -> Result<String> {
    let body = toml::to_string_pretty(p).context("serializing permissions to TOML")?;
    Ok(format!("{}{body}", file_header()))
}

fn file_header() -> &'static str {
    "# spotifai permission profile — guides one of the LLM agent surfaces.\n\
     #\n\
     # This file is read by spotifai and injected into the active agent's\n\
     # system prompt so it self-restricts to the listed verbs. It does NOT\n\
     # replace zad's own runtime enforcement at\n\
     # `~/.zad/services/<provider>/permissions.toml`. Edit `allowed` /\n\
     # `denied` to widen or narrow the surface, then re-run\n\
     # `spotifai install` so zad's load-time trust check accepts the new\n\
     # file.\n\n"
}

/// Parse a permissions file from a TOML string.
pub fn from_toml_string(s: &str) -> Result<Permissions> {
    toml::from_str(s).context("parsing permissions TOML")
}

/// Read the permissions file at `path`. Returns `fallback` if the
/// file does not exist, so callers can blindly inject without first
/// checking for presence.
pub fn read_or(path: &Path, fallback: Permissions) -> Result<Permissions> {
    match fs::read_to_string(path) {
        Ok(s) => from_toml_string(&s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(fallback),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

/// Write `default` to `path` as the seed permissions file if `path`
/// does not already exist. Returns `true` if a file was written,
/// `false` if one was already present. Creates parent directories as
/// needed. Used by [`ensure_default_for`] and exposed so tests can
/// drive the scaffolding without overriding `HOME`.
pub fn ensure_default_at(path: &Path, default: &Permissions) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let s = to_toml_string(default)?;
    fs::write(path, s).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

/// Write the default policy file for `(provider, profile)` if it
/// does not already exist. Returns the resolved path together with a
/// flag: `true` if a file was written, `false` if one was already
/// present.
pub fn ensure_default_for(provider: Provider, profile: Profile) -> Result<(PathBuf, bool)> {
    let path = path_for(provider, profile)?;
    let wrote = ensure_default_at(&path, &profile.default_policy(provider))?;
    Ok((path, wrote))
}
