//! `spotifai auth` — register Spotify OAuth credentials with the
//! pinned zad binary by forwarding to `zad service create spotify`.
//!
//! Spotify only issues one developer app per user, and zad's PKCE
//! public-client flow stores `client_id` + `refresh_token` in the OS
//! keychain. There is therefore no reason to scope the credentials
//! to a project — `auth` always invokes `create` without `--local`,
//! so the resulting credentials live at
//! `~/.zad/services/spotify/config.toml` and are visible to every
//! cwd `spotifai api …` runs from.
//!
//! Extra arguments after `spotifai auth` are forwarded verbatim to
//! `zad service create spotify`, so `--client-id`, `--refresh-token`,
//! `--no-browser`, `--non-interactive`, etc. all work unchanged.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::install;

/// Run `<zad> service create spotify <user_args...>` after ensuring
/// the pinned zad binary is present at `~/.spotifai/bin/zad`. On a
/// non-zero exit from zad, this exits the current process with the
/// same code so callers see zad's status verbatim.
pub fn run(user_args: &[String]) -> Result<()> {
    let zad = install::ensure_installed(false)?;
    let status = Command::new(&zad)
        .args(forward_args(user_args))
        .status()
        .with_context(|| format!("running {}", zad.display()))?;
    if status.success() {
        Ok(())
    } else {
        std::process::exit(status.code().unwrap_or(1));
    }
}

/// Build the argv that gets passed to zad: `service create spotify`
/// followed by whatever the user typed after `spotifai auth`.
/// Extracted so it can be unit-tested without spawning zad.
///
/// Note: `--local` is intentionally **not** injected. The user can
/// still pass it explicitly if they really want a project-scoped
/// credential, but the documented default is the global scope.
pub fn forward_args(user_args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(user_args.len() + 3);
    out.push("service".to_string());
    out.push("create".to_string());
    out.push("spotify".to_string());
    out.extend(user_args.iter().cloned());
    out
}

/// Helper used by tests to assemble the same `Command` `run` would
/// build, without spawning it.
#[doc(hidden)]
pub fn build_command(zad: &Path, user_args: &[String]) -> Command {
    let mut cmd = Command::new(zad);
    cmd.args(forward_args(user_args));
    cmd
}
