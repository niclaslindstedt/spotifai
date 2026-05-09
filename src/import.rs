//! `spotifai import` — recreate playlists on the active provider
//! from a `spotifai export` envelope.
//!
//! Inverse of [`crate::export`]. Reads the JSON envelope from stdin
//! or `--input PATH`, then for each playlist calls `zad <provider>
//! playlists create` followed by chunked `zad <provider> playlists
//! add` to recreate the ordered track/video list. Same-provider
//! re-imports use the embedded source IDs verbatim; cross-provider
//! migrations resolve every track/video on the target via `zad
//! <provider> search` (ISRC first, then title + primary-artist
//! fallback). Unresolvable items are skipped, accumulated in the
//! final report, and never abort the import.
//!
//! Scope is intentionally **playlists only**. Liked tracks, liked
//! videos, and saved albums in the envelope are ignored — those
//! would force widening [`crate::permissions::Profile::Playlist`]
//! to allow `library tracks save` / `library albums save` /
//! `library like`, which is out of scope for the migration use case
//! (and would force a new install/sign step on every machine).
//!
//! Existing-name collision policy: skip and warn. The pre-fetch of
//! existing playlists runs even under `--dry-run` so the preview is
//! realistic; it is read-only.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use crate::api::{SPOTIFAI_PROFILE_ENV, SPOTIFAI_PROVIDER_ENV, ZAD_PERMISSIONS_PATH_ENV};
use crate::export::{PAGE_SIZE, build_zad_args, extract_items};
use crate::install;
use crate::output;
use crate::permissions::{self, Profile};
use crate::providers::Provider;

/// Schema versions this importer accepts. Add new entries when the
/// envelope shape grows additively; bump and gate on a fresh entry
/// when the shape changes incompatibly.
pub const SUPPORTED_SCHEMA_VERSIONS: &[&str] = &["1"];

/// Run the import.
pub fn run(provider: Provider, input_path: Option<&Path>, dry_run: bool) -> Result<()> {
    let zad = install::ensure_installed(false)?;
    let (policy_path, _wrote) = permissions::ensure_default_for(provider, Profile::Playlist)?;

    // SAFETY: spotifai is single-threaded at this point — see the
    // matching block in src/export.rs.
    unsafe {
        std::env::set_var(SPOTIFAI_PROVIDER_ENV, provider.as_str());
        std::env::set_var(SPOTIFAI_PROFILE_ENV, Profile::Playlist.as_str());
    }

    output::header(&format!("spotifai import ({})", provider.display_name()));
    output::info(&format!("permissions: {}", policy_path.display()));
    if dry_run {
        output::info("dry-run: no playlists will be created or modified");
    }

    let raw = read_input(input_path)?;
    let envelope = parse_envelope(&raw)?;
    validate_schema_version(&envelope)?;
    let source = source_service(&envelope)?.to_string();
    let cross_provider = source != provider.zad_service_slug();
    if cross_provider {
        output::info(&format!(
            "cross-provider migration: source `{}` → target `{}`; resolving tracks via search",
            source,
            provider.zad_service_slug()
        ));
    }

    let existing = fetch_existing_playlist_names(&zad, &policy_path, provider)?;
    output::info(&format!("{} existing playlists on target", existing.len()));

    let mut report = ImportReport::default();
    let playlists = playlists_from_envelope(&envelope)?;
    output::info(&format!("{} playlists in envelope", playlists.len()));

    for playlist in playlists {
        let name = match playlist_display_name(playlist) {
            Some(n) => n,
            None => {
                output::warn("skipping playlist: no `name`/`title` field");
                report.playlists_failed += 1;
                continue;
            }
        };

        if is_duplicate_name(&name, &existing) {
            output::warn(&format!(
                "playlist `{name}` already exists on target — skipping"
            ));
            report.playlists_skipped_duplicate += 1;
            continue;
        }

        let tracks = tracks_in_playlist(playlist, &source);
        let mut resolved_ids: Vec<String> = Vec::with_capacity(tracks.len());
        for track in tracks {
            let id_opt = if cross_provider {
                resolve_track(track, provider, |q, ty| {
                    run_zad_search(&zad, &policy_path, provider, q, ty)
                })?
            } else {
                target_track_id(track, provider)
            };
            match id_opt {
                Some(id) => resolved_ids.push(id),
                None => {
                    output::warn(&format!(
                        "could not resolve `{}` on {}",
                        track_label(track),
                        provider.display_name()
                    ));
                    report.tracks_unresolved += 1;
                }
            }
        }

        if dry_run {
            output::info(&format!(
                "would create `{name}` with {} tracks",
                resolved_ids.len()
            ));
            report.playlists_created += 1;
            report.tracks_added += resolved_ids.len();
            continue;
        }

        let create_response = match run_zad_json(
            &zad,
            &policy_path,
            provider,
            &str_refs(&build_create_args(provider, &name)),
        ) {
            Ok(v) => v,
            Err(e) => {
                output::warn(&format!("`playlists create` failed for `{name}`: {e:#}"));
                report.playlists_failed += 1;
                continue;
            }
        };

        let pid = match extract_created_playlist_id(&create_response) {
            Some(p) => p,
            None => {
                output::warn(&format!(
                    "zad did not return a playlist id for `{name}` — skipping add"
                ));
                report.playlists_failed += 1;
                continue;
            }
        };

        let mut added = 0usize;
        for chunk in resolved_ids.chunks(PAGE_SIZE) {
            let args = build_add_args(provider, &pid, chunk);
            match run_zad_json(&zad, &policy_path, provider, &str_refs(&args)) {
                Ok(_) => added += chunk.len(),
                Err(e) => {
                    output::warn(&format!("`playlists add` failed for `{name}`: {e:#}"));
                    report.tracks_failed += chunk.len();
                }
            }
        }

        report.playlists_created += 1;
        report.tracks_added += added;
        output::status(&format!("created `{name}` with {added} tracks"));
    }

    output::status(&import_summary_line(&report));
    Ok(())
}

/// Read the envelope text from `--input PATH` or stdin.
fn read_input(path: Option<&Path>) -> Result<String> {
    match path {
        Some(p) => fs::read_to_string(p).with_context(|| format!("reading {}", p.display())),
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("reading envelope from stdin")?;
            Ok(buf)
        }
    }
}

/// Parse the envelope JSON text. Fatal on malformed JSON.
pub fn parse_envelope(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).context("parsing envelope JSON")
}

/// Verify `schema_version` is one we know how to consume.
pub fn validate_schema_version(envelope: &Value) -> Result<()> {
    let v = envelope
        .get("schema_version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("envelope is missing `schema_version`"))?;
    if SUPPORTED_SCHEMA_VERSIONS.contains(&v) {
        Ok(())
    } else {
        bail!(
            "unsupported envelope `schema_version`: `{v}`; this build supports {SUPPORTED_SCHEMA_VERSIONS:?}"
        )
    }
}

/// Read `source.service` out of the envelope. Fatal on missing or
/// non-string.
pub fn source_service(envelope: &Value) -> Result<&str> {
    envelope
        .get("source")
        .and_then(|s| s.get("service"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("envelope is missing `source.service`"))
}

/// Pull the `playlists` array out of the envelope.
pub fn playlists_from_envelope(envelope: &Value) -> Result<Vec<&Value>> {
    let arr = envelope
        .get("playlists")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("envelope `playlists` is missing or not an array"))?;
    Ok(arr.iter().collect())
}

/// Extract the per-playlist track/video list. Spotify exports nest
/// the list under `tracks`; YouTube Music exports use `videos`. The
/// `source` discriminator picks the right key, with the inactive key
/// used as a fallback in case a future export tightens the
/// convention.
pub fn tracks_in_playlist<'a>(playlist: &'a Value, source: &str) -> Vec<&'a Value> {
    let primary = if source == "spotify" {
        "tracks"
    } else {
        "videos"
    };
    let secondary = if source == "spotify" {
        "videos"
    } else {
        "tracks"
    };
    if let Some(arr) = playlist.get(primary).and_then(|v| v.as_array()) {
        return arr.iter().collect();
    }
    if let Some(arr) = playlist.get(secondary).and_then(|v| v.as_array()) {
        return arr.iter().collect();
    }
    Vec::new()
}

/// Pick the playlist's display name from the most likely keys.
pub fn playlist_display_name(playlist: &Value) -> Option<String> {
    for key in ["name", "title"] {
        if let Some(s) = playlist.get(key).and_then(|v| v.as_str())
            && !s.trim().is_empty()
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Build a `q=isrc:XXXX` search query when the source track carries
/// an `isrc` field. Spotify Web Search natively understands the
/// qualifier; YouTube Music will return zero hits and the caller
/// falls through to [`search_query_text`].
pub fn search_query_isrc(track: &Value) -> Option<String> {
    let isrc = track
        .get("isrc")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some(format!("isrc:{isrc}"))
}

/// Build a `q=<title> <primary artist>` text query. Permissive about
/// shape: `track.artists[0].name`, `track.artists[0]` as a string, or
/// the singular `track.artist` field — different zad providers ship
/// different shapes and the export embeds them verbatim.
pub fn search_query_text(track: &Value) -> Option<String> {
    let name = first_str(track, &["name", "title"])?;
    let artist = primary_artist(track)?;
    Some(format!("{name} {artist}"))
}

/// Resolve a target-provider track ID for an already-fetched search
/// hit (or, on the same-provider path, the embedded source track).
/// Each provider has a different canonical ID shape.
pub fn target_track_id(hit: &Value, target: Provider) -> Option<String> {
    let keys: &[&str] = match target {
        Provider::Spotify => &["spotify_id", "uri", "id"],
        Provider::YouTubeMusic => &["video_id", "videoId", "id"],
    };
    first_str(hit, keys).map(str::to_string)
}

/// Resolve a source track to a target-provider ID by calling the
/// supplied `search` callback. ISRC first, then title + primary
/// artist; returns `Ok(None)` when both queries either yield no hits
/// or yield hits without a usable ID.
pub fn resolve_track<F>(track: &Value, target: Provider, mut search: F) -> Result<Option<String>>
where
    F: FnMut(&str, Option<&str>) -> Result<Vec<Value>>,
{
    if let Some(q) = search_query_isrc(track) {
        let hits = search(&q, Some("track"))?;
        if let Some(id) = hits.first().and_then(|h| target_track_id(h, target)) {
            return Ok(Some(id));
        }
    }
    if let Some(q) = search_query_text(track) {
        let hits = search(&q, Some("track"))?;
        if let Some(id) = hits.first().and_then(|h| target_track_id(h, target)) {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Case-insensitive, trimmed comparison. "Focus" and "focus " are
/// the same playlist for the purposes of duplicate-skip.
pub fn is_duplicate_name(name: &str, existing: &[String]) -> bool {
    let needle = name.trim().to_ascii_lowercase();
    existing
        .iter()
        .any(|e| e.trim().to_ascii_lowercase() == needle)
}

/// `playlists create <provider-flag> <name>`. Provider-agnostic via
/// [`Provider::playlist_name_flag`].
pub fn build_create_args(provider: Provider, name: &str) -> Vec<String> {
    vec![
        "playlists".into(),
        "create".into(),
        provider.playlist_name_flag().into(),
        name.into(),
    ]
}

/// `playlists add <playlist-id> <id1> <id2> ...`.
pub fn build_add_args(_provider: Provider, playlist_id: &str, ids: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(3 + ids.len());
    out.push("playlists".into());
    out.push("add".into());
    out.push(playlist_id.into());
    out.extend(ids.iter().cloned());
    out
}

/// Pluck the new playlist's ID out of `zad playlists create --json`
/// output. zad's shape varies a touch across providers (Spotify
/// exposes `spotify_id`; YouTube Music uses `id`/`playlist_id`), so
/// try a small priority list.
pub fn extract_created_playlist_id(create_response: &Value) -> Option<String> {
    for key in ["id", "playlist_id", "spotify_id", "uri"] {
        if let Some(s) = create_response.get(key).and_then(|v| v.as_str())
            && !s.is_empty()
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Final stderr summary line.
pub fn import_summary_line(report: &ImportReport) -> String {
    format!(
        "imported {} playlists ({} tracks added, {} skipped duplicate, {} failed playlists, \
         {} unresolved tracks, {} failed adds)",
        report.playlists_created,
        report.tracks_added,
        report.playlists_skipped_duplicate,
        report.playlists_failed,
        report.tracks_unresolved,
        report.tracks_failed,
    )
}

/// Counters surfaced in the final summary line.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub playlists_created: usize,
    pub playlists_skipped_duplicate: usize,
    pub playlists_failed: usize,
    pub tracks_added: usize,
    pub tracks_unresolved: usize,
    pub tracks_failed: usize,
}

fn fetch_existing_playlist_names(
    zad: &Path,
    policy: &Path,
    provider: Provider,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut offset: usize = 0;
    let max_pages = 1000usize;
    for _ in 0..max_pages {
        let limit_str = PAGE_SIZE.to_string();
        let offset_str = offset.to_string();
        let args = [
            "playlists",
            "list",
            "--limit",
            &limit_str,
            "--offset",
            &offset_str,
        ];
        let value = run_zad_json(zad, policy, provider, &args)?;
        let page = extract_items(&value);
        let n = page.len();
        for item in &page {
            if let Some(name) = playlist_display_name(item) {
                names.push(name);
            }
        }
        if n < PAGE_SIZE {
            break;
        }
        offset += n;
    }
    Ok(names)
}

fn run_zad_search(
    zad: &Path,
    policy: &Path,
    provider: Provider,
    query: &str,
    ty: Option<&str>,
) -> Result<Vec<Value>> {
    let mut args: Vec<&str> = vec!["search", query];
    if let Some(t) = ty {
        args.push("--type");
        args.push(t);
    }
    let value = run_zad_json(zad, policy, provider, &args)?;
    Ok(extract_items(&value))
}

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

fn first_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(s) = value.get(*k).and_then(|v| v.as_str())
            && !s.is_empty()
        {
            return Some(s);
        }
    }
    None
}

fn primary_artist(track: &Value) -> Option<String> {
    if let Some(arr) = track.get("artists").and_then(|v| v.as_array())
        && let Some(first) = arr.first()
    {
        if let Some(s) = first.as_str()
            && !s.is_empty()
        {
            return Some(s.to_string());
        }
        if let Some(s) = first.get("name").and_then(|v| v.as_str())
            && !s.is_empty()
        {
            return Some(s.to_string());
        }
    }
    if let Some(s) = track.get("artist").and_then(|v| v.as_str())
        && !s.is_empty()
    {
        return Some(s.to_string());
    }
    None
}

fn track_label(track: &Value) -> String {
    let name = first_str(track, &["name", "title"]).unwrap_or("<unknown>");
    match primary_artist(track) {
        Some(a) => format!("{name} — {a}"),
        None => name.to_string(),
    }
}

fn str_refs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}
