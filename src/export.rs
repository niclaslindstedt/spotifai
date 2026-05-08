//! `spotifai export` — paginated, deterministic dump of the user's
//! library on the active provider into one structured JSON document.
//!
//! Reuses the `ask` permission profile because the verbs the export
//! needs (`playlists list/show`, `library …`) are exactly the
//! read-only set `ask` already allows. The permission injection
//! into a system prompt is irrelevant here — there is no LLM — but
//! `ZAD_PERMISSIONS_PATH` still has to point at a signed file so
//! zad's load-time gate passes.
//!
//! The output is intentionally a thin wrapper around whatever zad
//! returns under `--json`: each entity (track/video, album,
//! playlist) is embedded verbatim so any identifier zad already
//! exposes (`isrc`, `spotify_id`, `video_id`, `added_at`, position,
//! duration, …) flows through without spotifai having to track
//! zad's schema. A future importer for another music service reads
//! the same envelope.
//!
//! The provider axis is reflected in the envelope's `source.service`
//! field; importers route on that. Spotify exports include
//! `liked_tracks` and `saved_albums`; YouTube Music exports include
//! `liked_videos` (its `library list` covers rated videos) and
//! leave the album bucket empty since the YouTube Data API has no
//! "saved albums" concept.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::api::{SPOTIFAI_PROFILE_ENV, SPOTIFAI_PROVIDER_ENV, ZAD_PERMISSIONS_PATH_ENV};
use crate::install;
use crate::output;
use crate::permissions::{self, Profile};
use crate::providers::Provider;

/// Bumped when the on-disk envelope shape changes in a way an
/// importer must care about. Keep additive changes (new optional
/// fields) on `1`; reserve a bump for removed/renamed fields.
pub const SCHEMA_VERSION: &str = "1";

/// Page size requested from zad's list endpoints. Spotify caps most
/// list endpoints at 50 per page, so we do too. YouTube Data API v3
/// caps `playlistItems` at 50 too, so the same value is safe across
/// providers.
pub const PAGE_SIZE: usize = 50;

/// Hard cap on the number of pages we request for a single list, to
/// short-circuit if zad ever returns a full page indefinitely.
const MAX_PAGES: usize = 1000;

/// Run the export.
///
/// `output_path` redirects the JSON document to a file; with
/// `None`, the document is written to stdout. `pretty` toggles
/// two-space indented output. Status messages (header, counts)
/// always go to stderr via [`crate::output`].
pub fn run(provider: Provider, output_path: Option<&Path>, pretty: bool) -> Result<()> {
    let zad = install::ensure_installed(false)?;
    let (policy_path, _wrote) = permissions::ensure_default_for(provider, Profile::Ask)?;

    // Match the env-var pre-flight that `spotifai ask` and
    // `spotifai playlist` perform, so any helper that consults
    // SPOTIFAI_PROVIDER / SPOTIFAI_PROFILE behaves the same
    // regardless of which surface selected the pair.
    //
    // SAFETY: spotifai is single-threaded at this point — see the
    // matching block in src/session.rs.
    unsafe {
        std::env::set_var(SPOTIFAI_PROVIDER_ENV, provider.as_str());
        std::env::set_var(SPOTIFAI_PROFILE_ENV, Profile::Ask.as_str());
    }

    output::header(&format!("spotifai export ({})", provider.display_name()));
    output::info(&format!("permissions: {}", policy_path.display()));

    let envelope = match provider {
        Provider::Spotify => collect_spotify(&zad, &policy_path, provider)?,
        Provider::YouTubeMusic => collect_ymusic(&zad, &policy_path, provider)?,
    };

    let serialized = if pretty {
        serde_json::to_string_pretty(&envelope)?
    } else {
        serde_json::to_string(&envelope)?
    };
    write_output(output_path, &serialized)?;

    let liked_tracks = envelope_array_len(&envelope, "liked_tracks");
    let liked_videos = envelope_array_len(&envelope, "liked_videos");
    let albums = envelope_array_len(&envelope, "saved_albums");
    let playlists = envelope_array_len(&envelope, "playlists");
    output::status(&format!(
        "exported {liked_tracks} liked tracks, {liked_videos} liked videos, \
         {albums} albums, {playlists} playlists"
    ));
    Ok(())
}

fn collect_spotify(zad: &Path, policy: &Path, provider: Provider) -> Result<Value> {
    output::info("fetching liked tracks…");
    let liked_tracks = collect_paginated(zad, policy, provider, &["library", "tracks", "list"])?;
    output::info(&format!("  {} liked tracks", liked_tracks.len()));

    output::info("fetching saved albums…");
    let saved_albums = collect_paginated(zad, policy, provider, &["library", "albums", "list"])?;
    output::info(&format!("  {} saved albums", saved_albums.len()));

    output::info("fetching playlists…");
    let playlists = collect_playlists(zad, policy, provider)?;
    output::info(&format!("  {} playlists", playlists.len()));

    Ok(build_spotify_envelope(
        provider,
        liked_tracks,
        saved_albums,
        playlists,
    ))
}

fn collect_ymusic(zad: &Path, policy: &Path, provider: Provider) -> Result<Value> {
    output::info("fetching liked videos…");
    let liked_videos = collect_paginated(zad, policy, provider, &["library", "list"])?;
    output::info(&format!("  {} liked videos", liked_videos.len()));

    output::info("fetching playlists…");
    let playlists = collect_playlists(zad, policy, provider)?;
    output::info(&format!("  {} playlists", playlists.len()));

    Ok(build_ymusic_envelope(provider, liked_videos, playlists))
}

fn envelope_array_len(envelope: &Value, key: &str) -> usize {
    envelope
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

/// Build the top-level JSON envelope around already-collected
/// Spotify slices. Pulled out of [`run`] so unit tests can exercise
/// it without spawning zad. Kept as the legacy
/// `build_envelope(...)` shape — re-exported below — so existing
/// importers do not break.
pub fn build_spotify_envelope(
    provider: Provider,
    liked_tracks: Vec<Value>,
    saved_albums: Vec<Value>,
    playlists: Vec<Value>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "exported_at": iso8601_now(),
        "source": {
            "service": provider.zad_service_slug(),
            "tool": "spotifai",
            "tool_version": crate::version(),
        },
        "liked_tracks": liked_tracks,
        "saved_albums": saved_albums,
        "playlists": playlists,
    })
}

/// Build the YouTube Music JSON envelope. Carries `liked_videos`
/// instead of `liked_tracks`/`saved_albums`; importers route on
/// `source.service`.
pub fn build_ymusic_envelope(
    provider: Provider,
    liked_videos: Vec<Value>,
    playlists: Vec<Value>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "exported_at": iso8601_now(),
        "source": {
            "service": provider.zad_service_slug(),
            "tool": "spotifai",
            "tool_version": crate::version(),
        },
        "liked_videos": liked_videos,
        "playlists": playlists,
    })
}

/// Backwards-compatible Spotify envelope builder used by existing
/// tests. Defaults `provider` to [`Provider::Spotify`].
pub fn build_envelope(
    liked_tracks: Vec<Value>,
    saved_albums: Vec<Value>,
    playlists: Vec<Value>,
) -> Value {
    build_spotify_envelope(Provider::Spotify, liked_tracks, saved_albums, playlists)
}

/// Page through a `zad <provider> <verb> --json` list endpoint with
/// `--limit` / `--offset`, accumulating items in order.
///
/// Stops when a page returns fewer items than `PAGE_SIZE`, when an
/// empty page comes back, or when [`MAX_PAGES`] is hit.
fn collect_paginated(
    zad: &Path,
    policy: &Path,
    provider: Provider,
    verb: &[&str],
) -> Result<Vec<Value>> {
    let mut all = Vec::new();
    let mut offset: usize = 0;
    for _ in 0..MAX_PAGES {
        let limit_str = PAGE_SIZE.to_string();
        let offset_str = offset.to_string();
        let mut args: Vec<&str> = verb.to_vec();
        args.push("--limit");
        args.push(&limit_str);
        args.push("--offset");
        args.push(&offset_str);
        let value = run_zad_json(zad, policy, provider, &args)?;
        let page = extract_items(&value);
        let page_len = page.len();
        all.extend(page);
        if page_len < PAGE_SIZE {
            break;
        }
        offset += page_len;
    }
    Ok(all)
}

/// Page through `playlists list`, then for each playlist fetch
/// `playlists show <id>` and substitute the show output for the
/// list entry. The show output is the richer record — it carries
/// the playlist's full track list — so any caller importing the
/// export has both the metadata and the ordered tracks/videos in
/// one place.
fn collect_playlists(zad: &Path, policy: &Path, provider: Provider) -> Result<Vec<Value>> {
    let summaries = collect_paginated(zad, policy, provider, &["playlists", "list"])?;
    let mut out = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let id = match playlist_identifier(&summary) {
            Some(id) => id,
            None => {
                output::warn("skipping playlist: list entry has no `id`/`uri`/`name`/`title`");
                out.push(summary);
                continue;
            }
        };
        match run_zad_json(zad, policy, provider, &["playlists", "show", &id]) {
            Ok(detail) => out.push(detail),
            Err(e) => {
                output::warn(&format!("playlist `{id}` skipped: {e:#}"));
                out.push(summary);
            }
        }
    }
    Ok(out)
}

/// Pick the most stable identifier zad's playlist-list entries are
/// likely to expose. Tries `id`, then `uri`, then `name` / `title`
/// so we can still call `playlists show` against a name-keyed zad
/// CLI.
fn playlist_identifier(summary: &Value) -> Option<String> {
    for key in ["id", "uri", "spotify_id", "playlist_id", "name", "title"] {
        if let Some(s) = summary.get(key).and_then(|v| v.as_str())
            && !s.is_empty()
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Run `<zad> <provider> <args...> --json`, capture stdout, parse
/// JSON.
fn run_zad_json(zad: &Path, policy: &Path, provider: Provider, args: &[&str]) -> Result<Value> {
    let cmd_args = build_zad_args(provider, args);
    let out = Command::new(zad)
        .args(&cmd_args)
        .env(ZAD_PERMISSIONS_PATH_ENV, policy)
        .output()
        .with_context(|| format!("running {} {}", zad.display(), cmd_args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "zad {} exited {}: {}",
            cmd_args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    serde_json::from_slice(&out.stdout)
        .with_context(|| format!("parsing JSON from `zad {}`", cmd_args.join(" ")))
}

/// Prefix `<provider>` and append `--json` around a verb's argv.
/// Pulled out so tests can assert on the exact shape without
/// spawning zad.
pub fn build_zad_args(provider: Provider, args: &[&str]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len() + 2);
    out.push(provider.zad_subcommand().to_string());
    out.extend(args.iter().map(|s| s.to_string()));
    out.push("--json".to_string());
    out
}

/// Extract a list of items from whatever shape zad's `--json`
/// returns — a bare array, or an object wrapping the array under
/// one of the usual pagination keys.
pub fn extract_items(value: &Value) -> Vec<Value> {
    if let Some(arr) = value.as_array() {
        return arr.clone();
    }
    for key in [
        "items",
        "tracks",
        "playlists",
        "albums",
        "videos",
        "data",
        "results",
    ] {
        if let Some(arr) = value.get(key).and_then(|v| v.as_array()) {
            return arr.clone();
        }
    }
    Vec::new()
}

fn write_output(path: Option<&Path>, body: &str) -> Result<()> {
    match path {
        Some(p) => write_to_file(p, body),
        None => {
            use std::io::Write as _;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle
                .write_all(body.as_bytes())
                .context("writing JSON to stdout")?;
            handle
                .write_all(b"\n")
                .context("writing trailing newline")?;
            Ok(())
        }
    }
}

fn write_to_file(path: &Path, body: &str) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let with_newline = if body.ends_with('\n') {
        body.to_string()
    } else {
        format!("{body}\n")
    };
    fs::write(path, with_newline).with_context(|| format!("writing {}", path.display()))?;
    output::info(&format!("wrote {}", absolute_or_self(path).display()));
    Ok(())
}

fn absolute_or_self(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Format the current UTC time as `YYYY-MM-DDTHH:MM:SSZ` without
/// pulling in a date crate. Falls back to the unix epoch on the
/// (impossible) clock-before-1970 case.
pub fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_iso8601(secs)
}

/// Format an absolute unix-epoch second count as ISO 8601 in UTC.
/// Pulled out so tests can lock the formatter to a known timestamp.
pub fn format_iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Howard Hinnant's `civil_from_days` algorithm: convert a day
/// count since 1970-01-01 into a (year, month, day) civil triple.
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}
