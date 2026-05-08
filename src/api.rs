//! `spotifai api …` — forward-route to the pinned zad binary's
//! `spotify` subcommand.
//!
//! Spotify-flavoured commands are not re-implemented in spotifai; we
//! exec the managed zad binary at `~/.spotifai/bin/zad` so it can
//! enforce its own permission policy, OAuth flow, and keychain access.
//! Before forwarding, we run the same install/version check as
//! `spotifai install` so a missing or stale binary is replaced with
//! the version pinned in `.zadrc`.
//!
//! The forwarded process inherits the parent environment plus an
//! explicit `ZAD_PERMISSIONS_PATH` pointing at the spotifai-managed
//! file (`~/.spotifai/permissions.toml`). zad ≥ 0.3.0 reads that
//! variable to override the cwd-derived project slug, so the same
//! policy applies to every directory `spotifai api …` is invoked
//! from.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::{install, permissions};

/// Env var zad ≥ 0.3.0 honours for an explicit local-permissions
/// file. Setting it bypasses zad's project-slug lookup so a single
/// spotifai-managed policy applies regardless of cwd.
pub const ZAD_PERMISSIONS_PATH_ENV: &str = "ZAD_PERMISSIONS_PATH";

/// Run `<zad> spotify <user_args...>` after ensuring the pinned zad
/// binary is present at `~/.spotifai/bin/zad`. On a non-zero exit
/// from zad, this exits the current process with the same code so
/// callers see zad's status verbatim.
pub fn forward(user_args: &[String]) -> Result<()> {
    let zad = install::ensure_installed(false)?;
    let policy_path = permissions::default_path()?;
    let status = build_command(&zad, user_args)
        .env(ZAD_PERMISSIONS_PATH_ENV, &policy_path)
        .status()
        .with_context(|| format!("running {}", zad.display()))?;
    if status.success() {
        Ok(())
    } else {
        std::process::exit(status.code().unwrap_or(1));
    }
}

/// Build the argv that gets passed to zad: `spotify` followed by
/// whatever the user typed after `spotifai api`. Extracted so it can
/// be unit-tested without spawning zad.
pub fn forward_args(user_args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(user_args.len() + 1);
    out.push("spotify".to_string());
    out.extend(user_args.iter().cloned());
    out
}

/// Helper used by tests to assemble the same `Command` `forward`
/// would build, without spawning it. Does **not** set
/// `ZAD_PERMISSIONS_PATH`; callers in production wire that in via
/// [`forward`], and tests that care assert on it via
/// [`permissions_env_value`].
#[doc(hidden)]
pub fn build_command(zad: &Path, user_args: &[String]) -> Command {
    let mut cmd = Command::new(zad);
    cmd.args(forward_args(user_args));
    cmd
}

/// Resolve the value `forward` would set for `ZAD_PERMISSIONS_PATH`.
/// Exposed so tests can assert on the env-injection behaviour without
/// spawning zad.
pub fn permissions_env_value() -> Result<PathBuf> {
    permissions::default_path()
}

/// Convenience used in unit tests: read the `ZAD_PERMISSIONS_PATH`
/// value already set on a [`Command`].
#[doc(hidden)]
pub fn command_env<'a>(cmd: &'a Command, key: &str) -> Option<&'a OsStr> {
    cmd.get_envs()
        .find(|(k, _)| k.to_string_lossy() == key)
        .and_then(|(_, v)| v)
}
