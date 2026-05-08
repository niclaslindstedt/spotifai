//! `spotifai auth` — register OAuth credentials with the pinned zad
//! binary by forwarding to `zad service create <provider>`.
//!
//! The active provider is selected by `--provider` on the spotifai
//! side (default: Spotify). Spotify only issues one developer app
//! per user; YouTube Music auth uses a Google Cloud "Desktop app"
//! OAuth client. Either way the resulting credentials live at zad's
//! global scope (`~/.zad/services/<provider>/config.toml`) and apply
//! to every directory `spotifai api …` is invoked from.
//!
//! Extra arguments after `spotifai auth` are forwarded verbatim to
//! `zad service create <provider>`, so flags such as `--client-id`,
//! `--client-secret`, `--refresh-token`, `--no-browser`,
//! `--non-interactive`, etc. all work unchanged. The provider's own
//! help text (`spotifai auth --provider <name> --help`) is the
//! authoritative reference for which flags are supported.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::install;
use crate::providers::Provider;

/// Run `<zad> service create <provider> <user_args...>` after
/// ensuring the pinned zad binary is present at
/// `~/.spotifai/bin/zad`. On a non-zero exit from zad, this exits
/// the current process with the same code so callers see zad's
/// status verbatim.
pub fn run(provider: Provider, user_args: &[String]) -> Result<()> {
    let zad = install::ensure_installed(false)?;
    let status = Command::new(&zad)
        .args(forward_args(provider, user_args))
        .status()
        .with_context(|| format!("running {}", zad.display()))?;
    if status.success() {
        Ok(())
    } else {
        std::process::exit(status.code().unwrap_or(1));
    }
}

/// Build the argv that gets passed to zad: `service create
/// <provider>` followed by whatever the user typed after `spotifai
/// auth`. Extracted so it can be unit-tested without spawning zad.
///
/// Note: `--local` is intentionally **not** injected. The user can
/// still pass it explicitly if they really want a project-scoped
/// credential, but the documented default is the global scope.
pub fn forward_args(provider: Provider, user_args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(user_args.len() + 3);
    out.push("service".to_string());
    out.push("create".to_string());
    out.push(provider.zad_service_slug().to_string());
    out.extend(user_args.iter().cloned());
    out
}

/// Helper used by tests to assemble the same `Command` `run` would
/// build, without spawning it.
#[doc(hidden)]
pub fn build_command(zad: &Path, provider: Provider, user_args: &[String]) -> Command {
    let mut cmd = Command::new(zad);
    cmd.args(forward_args(provider, user_args));
    cmd
}
