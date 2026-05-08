//! `spotifai api …` — forward-route to the pinned zad binary's
//! `spotify` subcommand.
//!
//! Spotify-flavoured commands are not re-implemented in spotifai; we
//! exec the managed zad binary at `~/.spotifai/bin/zad` so it can
//! enforce its own permission policy, OAuth flow, and keychain access.
//! Before forwarding, we run the same install/version check as
//! `spotifai install` so a missing or stale binary is replaced with
//! the version pinned in `.zadrc`.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::install;

/// Run `<zad> spotify <user_args...>` after ensuring the pinned zad
/// binary is present at `~/.spotifai/bin/zad`. On a non-zero exit
/// from zad, this exits the current process with the same code so
/// callers see zad's status verbatim.
pub fn forward(user_args: &[String]) -> Result<()> {
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
/// would build, without spawning it.
#[doc(hidden)]
pub fn build_command(zad: &Path, user_args: &[String]) -> Command {
    let mut cmd = Command::new(zad);
    cmd.args(forward_args(user_args));
    cmd
}
