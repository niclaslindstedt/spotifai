//! Bootstrap helpers for the pinned zad binary at
//! `~/.spotifai/bin/zad` and the signed per-provider permissions
//! files at `~/.spotifai/permissions/<provider>/<profile>.toml`.
//!
//! spotifai forward-routes provider subcommands to this exact binary
//! path so a globally-installed zad with mismatched permissions or
//! schema can never be picked up by accident. The pinned version is
//! baked in at compile time from `.zadrc` at the repo root.
//!
//! zad ≥ 0.4.0 fails closed on permission files that are not in the
//! per-machine signed trust store, so the install flow also
//! bootstraps the local Ed25519 signing key (`zad signing init`) and
//! signs each spotifai-managed policy file (`zad <provider>
//! permissions sign`) before the first `spotifai api …` call needs
//! it. Each provider has its own zad subcommand for signing —
//! `zad spotify permissions sign` for the Spotify profile files,
//! `zad ymusic permissions sign` for the YouTube Music profile files
//! — but the rest of the flow (env-var pinning, idempotence) is
//! identical.

use std::ffi::OsStr;
use std::fs;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

use crate::output;
use crate::providers::Provider;

/// Env var zad ≥ 0.3.0 honours for an explicit local-permissions
/// file. Re-exported here so the install module can sign the
/// spotifai-managed file in place without hard-coding the global vs.
/// project-local resolution.
pub const ZAD_PERMISSIONS_PATH_ENV: &str = "ZAD_PERMISSIONS_PATH";

/// Tag string baked in from `.zadrc` (e.g. `v0.6.0`).
pub const PINNED_VERSION_RAW: &str = include_str!("../.zadrc");

const REPO: &str = "niclaslindstedt/zad";
const HTTP_USER_AGENT: &str = concat!("spotifai/", env!("CARGO_PKG_VERSION"));

/// `.zadrc` contents trimmed of whitespace.
pub fn pinned_version() -> &'static str {
    PINNED_VERSION_RAW.trim()
}

/// Absolute path of the managed zad binary (`~/.spotifai/bin/zad`).
pub fn zad_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    let mut p = home.join(".spotifai").join("bin").join("zad");
    if cfg!(windows) {
        p.set_extension("exe");
    }
    Ok(p)
}

/// Ensure the pinned zad binary is installed at [`zad_path`].
/// Returns the absolute path it ended up at.
///
/// `force` re-downloads even when the existing binary already
/// reports the correct version.
pub fn ensure_installed(force: bool) -> Result<PathBuf> {
    let target = zad_path()?;
    let pinned = pinned_version();

    if !force {
        if let Some(installed) = current_version(&target)? {
            if version_matches(&installed, pinned) {
                output::status(&format!(
                    "zad {installed} already installed at {}",
                    target.display()
                ));
                return Ok(target);
            }
            output::info(&format!(
                "zad {installed} present but pinned version is {pinned}; replacing"
            ));
        }
    }

    let url = asset_url(pinned).context("computing release asset URL")?;
    download_and_install(&url, &target)?;

    let installed = current_version(&target)?
        .ok_or_else(|| anyhow!("downloaded zad does not report a version"))?;
    if !version_matches(&installed, pinned) {
        bail!(
            "downloaded zad reports version `{installed}` but expected `{pinned}` — \
             check the release tag in .zadrc"
        );
    }
    output::status(&format!("installed zad {installed} → {}", target.display()));
    Ok(target)
}

/// Run `<binary> --version` and parse the trailing version token.
///
/// Returns `Ok(None)` if the binary is missing. Errors only on real
/// I/O / parse failures so callers can treat "not installed" the
/// same as "wrong version".
pub fn current_version(binary: &Path) -> Result<Option<String>> {
    if !binary.exists() {
        return Ok(None);
    }
    let out = Command::new(binary)
        .arg("--version")
        .output()
        .with_context(|| format!("running {} --version", binary.display()))?;
    if !out.status.success() {
        bail!("{} --version exited with {}", binary.display(), out.status);
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    parse_version(&stdout)
        .map(Some)
        .ok_or_else(|| anyhow!("could not parse version from `{}`", stdout.trim()))
}

/// Pull the trailing whitespace-separated token from a `<name>
/// <version>` line as printed by clap's `--version`. Tolerant of
/// multi-line output.
pub fn parse_version(s: &str) -> Option<String> {
    let line = s.lines().next()?.trim();
    let token = line.split_whitespace().next_back()?;
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

/// Compare `zad --version` output (e.g. `0.6.0`) against a `.zadrc`
/// tag (e.g. `v0.6.0`). The leading `v` is optional on either side.
pub fn version_matches(installed: &str, pinned: &str) -> bool {
    strip_v(installed.trim()) == strip_v(pinned.trim())
}

fn strip_v(s: &str) -> &str {
    s.strip_prefix('v').unwrap_or(s)
}

/// Build the
/// `https://github.com/<repo>/releases/download/<tag>/<asset>` URL
/// for the current host. Errors on unsupported OS/arch combinations.
pub fn asset_url(tag: &str) -> Result<String> {
    let asset = asset_name()?;
    Ok(format!(
        "https://github.com/{REPO}/releases/download/{tag}/{asset}"
    ))
}

/// Compute the release asset filename for the current host,
/// mirroring zad's own `scripts/install.sh` naming.
pub fn asset_name() -> Result<String> {
    asset_name_for(std::env::consts::OS, std::env::consts::ARCH)
}

pub fn asset_name_for(os: &str, arch: &str) -> Result<String> {
    let target_os = match os {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        other => bail!("unsupported OS `{other}`; zad ships builds for linux, macos, windows"),
    };
    let target_arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => bail!("unsupported architecture `{other}`; zad ships x86_64 and aarch64"),
    };
    let suffix = if os == "windows" { ".exe" } else { "" };
    Ok(format!("zad-{target_arch}-{target_os}{suffix}"))
}

fn download_and_install(url: &str, target: &Path) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("zad install path has no parent directory"))?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    output::info(&format!("downloading {url}"));
    let client = reqwest::blocking::Client::builder()
        .user_agent(HTTP_USER_AGENT)
        .build()
        .context("building HTTP client")?;
    let mut resp = client
        .get(url)
        .send()
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed: {url}"))?;

    let tmp = target.with_extension("download");
    {
        let mut f =
            fs::File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
        copy(&mut resp, &mut f).with_context(|| format!("writing {}", tmp.display()))?;
    }
    set_executable(&tmp)?;
    fs::rename(&tmp, target).with_context(|| format!("installing to {}", target.display()))?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut perms = fs::metadata(p)
        .with_context(|| format!("stat {}", p.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(p, perms).with_context(|| format!("chmod {}", p.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_p: &Path) -> Result<()> {
    Ok(())
}

/// Run `<zad> signing init` to bootstrap the per-machine Ed25519
/// signing key in the OS keychain. Idempotent — `signing init` is a
/// no-op when a key already exists, so this is safe to call on every
/// `spotifai install` invocation.
///
/// On success the keypair lives in the OS keychain (account
/// `signing:v1`) and a self-signed empty trust store is written to
/// `~/.zad/signing/trusted.toml`.
pub fn bootstrap_signing_key(zad: &Path) -> Result<Option<String>> {
    let out = Command::new(zad)
        .args(["signing", "init", "--json"])
        .output()
        .with_context(|| format!("running {} signing init", zad.display()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!(
            "zad signing init failed (exit {}): {}",
            out.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }
    Ok(parse_fingerprint(&String::from_utf8_lossy(&out.stdout)))
}

/// Run `<zad> <provider> permissions sign --local` against the file
/// at `policy_path`, pinned via `ZAD_PERMISSIONS_PATH`. Adds a
/// `[signature]` block to the file in place and upserts a
/// trust-store entry so subsequent `spotifai api …` invocations pass
/// zad's load-time verification.
///
/// Each provider has its own zad subcommand for signing
/// (`zad spotify permissions sign`, `zad ymusic permissions sign`,
/// …), but the env-var pin and the `--local` scope flag are shared.
///
/// Requires [`bootstrap_signing_key`] (or an earlier `zad signing
/// init`) to have populated the keychain — the call fails with
/// `SigningKeyMissing` otherwise.
pub fn sign_permissions_file(zad: &Path, provider: Provider, policy_path: &Path) -> Result<()> {
    let out = Command::new(zad)
        .args([provider.zad_subcommand(), "permissions", "sign", "--local"])
        .env(ZAD_PERMISSIONS_PATH_ENV, policy_path)
        .output()
        .with_context(|| {
            format!(
                "running {} {} permissions sign --local",
                zad.display(),
                provider.zad_subcommand(),
            )
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!(
            "zad {} permissions sign failed (exit {}): {}",
            provider.zad_subcommand(),
            out.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }
    Ok(())
}

/// Pull the `fingerprint` field out of `zad signing init --json`
/// output. Returns `None` for malformed or fingerprint-less JSON so
/// the install flow can continue with a placeholder rather than
/// erroring on a purely cosmetic field.
pub fn parse_fingerprint(json: &str) -> Option<String> {
    // The JSON shape is small and stable; sidestep adding a
    // serde_json dep just for this status line by scanning for the
    // `"fingerprint": "<value>"` token.
    let needle = "\"fingerprint\"";
    let start = json.find(needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = &rest[colon + 1..];
    let open = after.find('"')? + 1;
    let close_rel = after[open..].find('"')?;
    Some(after[open..open + close_rel].to_string())
}

/// Helper used by tests: assemble the same `Command`
/// [`bootstrap_signing_key`] would build, without spawning it.
#[doc(hidden)]
pub fn build_signing_init_command(zad: &Path) -> Command {
    let mut cmd = Command::new(zad);
    cmd.args(["signing", "init", "--json"]);
    cmd
}

/// Helper used by tests: assemble the same `Command`
/// [`sign_permissions_file`] would build, without spawning it.
#[doc(hidden)]
pub fn build_permissions_sign_command(
    zad: &Path,
    provider: Provider,
    policy_path: &Path,
) -> Command {
    let mut cmd = Command::new(zad);
    cmd.args([provider.zad_subcommand(), "permissions", "sign", "--local"]);
    cmd.env(ZAD_PERMISSIONS_PATH_ENV, policy_path);
    cmd
}

/// Convenience used by tests: read a single env var off a [`Command`].
#[doc(hidden)]
pub fn command_env<'a>(cmd: &'a Command, key: &str) -> Option<&'a OsStr> {
    cmd.get_envs()
        .find(|(k, _)| k.to_string_lossy() == key)
        .and_then(|(_, v)| v)
}
