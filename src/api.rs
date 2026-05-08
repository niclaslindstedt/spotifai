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
//! `spotifai api` requires a parent spotifai command (`ask` or
//! `playlist`) to have selected a profile via the [`SPOTIFAI_PROFILE_ENV`]
//! variable. The selected profile resolves to one of the
//! `~/.spotifai/permissions/<profile>.toml` files; that path is pinned
//! on the forwarded child via `ZAD_PERMISSIONS_PATH`, overriding any
//! inherited value so an agent cannot escalate by setting the zad
//! variable itself before invoking this shim.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

use crate::install;
use crate::permissions::{self, Profile};

/// Env var zad ≥ 0.3.0 honours for an explicit local-permissions
/// file. Setting it bypasses zad's project-slug lookup so a single
/// spotifai-managed policy applies regardless of cwd.
pub const ZAD_PERMISSIONS_PATH_ENV: &str = "ZAD_PERMISSIONS_PATH";

/// Env var read by `spotifai api` to pick which profile's policy file
/// to forward to zad. Set by `spotifai ask` and `spotifai playlist`
/// before they spawn zag; an unset value is treated as a usage error
/// because there is no safe default.
pub const SPOTIFAI_PROFILE_ENV: &str = "SPOTIFAI_PROFILE";

/// Run `<zad> spotify <user_args...>` after ensuring the pinned zad
/// binary is present at `~/.spotifai/bin/zad`. On a non-zero exit
/// from zad, this exits the current process with the same code so
/// callers see zad's status verbatim.
///
/// Errors out before spawning zad if `SPOTIFAI_PROFILE` is unset or
/// holds an unknown value — there is intentionally no implicit
/// default.
pub fn forward(user_args: &[String]) -> Result<()> {
    let zad = install::ensure_installed(false)?;
    let profile = active_profile()?;
    let policy_path = resolve_permissions_path(profile)?;
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

/// Read [`SPOTIFAI_PROFILE_ENV`] and parse it into a [`Profile`].
///
/// Returns a usage-style error when the variable is missing, empty, or
/// holds an unknown value. The error message points the user at the
/// commands that set the variable on their behalf rather than coaching
/// them into setting it themselves — direct zad usage should go
/// through `~/.spotifai/bin/zad spotify …` instead.
pub fn active_profile() -> Result<Profile> {
    let raw = std::env::var(SPOTIFAI_PROFILE_ENV).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!(missing_profile_message());
    }
    Profile::parse(trimmed).ok_or_else(|| {
        anyhow!(
            "unknown {SPOTIFAI_PROFILE_ENV}=`{trimmed}`. {}",
            missing_profile_message(),
        )
    })
}

fn missing_profile_message() -> String {
    "`spotifai api` must be invoked through `spotifai ask` or `spotifai playlist`; \
     no permission profile is selected. To call zad directly, run \
     `~/.spotifai/bin/zad spotify …` with `ZAD_PERMISSIONS_PATH` set yourself."
        .to_string()
}

/// Resolve the policy file backing `profile` and verify it exists on
/// disk. Surfaces a friendly "run `spotifai install`" error rather
/// than letting zad's load-time trust check fail with a less specific
/// message.
pub fn resolve_permissions_path(profile: Profile) -> Result<PathBuf> {
    let path = permissions::path_for(profile)?;
    if !path.exists() {
        bail!(
            "permissions file for profile `{}` is missing at {}; run `spotifai install` to \
             scaffold and sign it",
            profile.as_str(),
            path.display()
        );
    }
    Ok(path)
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
/// `ZAD_PERMISSIONS_PATH`; production callers in [`forward`] wire that
/// in once the active profile has been resolved.
#[doc(hidden)]
pub fn build_command(zad: &Path, user_args: &[String]) -> Command {
    let mut cmd = Command::new(zad);
    cmd.args(forward_args(user_args));
    cmd
}

/// Convenience used in unit tests: read the `ZAD_PERMISSIONS_PATH`
/// value already set on a [`Command`].
#[doc(hidden)]
pub fn command_env<'a>(cmd: &'a Command, key: &str) -> Option<&'a OsStr> {
    cmd.get_envs()
        .find(|(k, _)| k.to_string_lossy() == key)
        .and_then(|(_, v)| v)
}
