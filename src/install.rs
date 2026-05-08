//! Bootstrap helper that ensures the pinned zad binary lives at
//! `~/.spotifai/bin/zad`.
//!
//! spotifai forward-routes Spotify subcommands to this exact path so a
//! globally-installed zad with mismatched permissions or schema can
//! never be picked up by accident. The pinned version is baked in at
//! compile time from `.zadrc` at the repo root.

use std::fs;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

use crate::output;

/// Tag string baked in from `.zadrc` (e.g. `v0.2.0`).
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

/// Ensure the pinned zad binary is installed at [`zad_path`]. Returns
/// the absolute path it ended up at.
///
/// `force` re-downloads even when the existing binary already reports
/// the correct version.
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
/// I/O / parse failures so callers can treat "not installed" the same
/// as "wrong version".
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

/// Pull the trailing whitespace-separated token from a `<name> <version>`
/// line as printed by clap's `--version`. Tolerant of multi-line output.
pub fn parse_version(s: &str) -> Option<String> {
    let line = s.lines().next()?.trim();
    let token = line.split_whitespace().next_back()?;
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

/// Compare `zad --version` output (e.g. `0.2.0`) against a `.zadrc` tag
/// (e.g. `v0.2.0`). The leading `v` is optional on either side.
pub fn version_matches(installed: &str, pinned: &str) -> bool {
    strip_v(installed.trim()) == strip_v(pinned.trim())
}

fn strip_v(s: &str) -> &str {
    s.strip_prefix('v').unwrap_or(s)
}

/// Build the `https://github.com/<repo>/releases/download/<tag>/<asset>`
/// URL for the current host. Errors on unsupported OS/arch combinations.
pub fn asset_url(tag: &str) -> Result<String> {
    let asset = asset_name()?;
    Ok(format!(
        "https://github.com/{REPO}/releases/download/{tag}/{asset}"
    ))
}

/// Compute the release asset filename for the current host, mirroring
/// zad's own `scripts/install.sh` naming.
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
