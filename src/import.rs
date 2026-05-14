//! `spotifai import` — recreate playlists on the active provider
//! from a unified spotifai schema envelope.
//!
//! Reads the JSON envelope from stdin or `--input PATH`, then walks
//! every [`crate::export_schema::Playlist`]:
//!
//! - Resolve each [`crate::export_schema::Track`] to a target-provider
//!   item id. On a same-provider re-import (`source.service` matches
//!   `--provider`) we use the track's `source_ids[<service>]` verbatim.
//!   On a cross-provider migration we run `Spotify::search` /
//!   `Ymusic::search` against an ISRC-first query, with a `<title>
//!   <primary artist>` fallback.
//! - Skip playlists whose name already exists on the target.
//! - Create the playlist on the target via the typed
//!   `create_playlist` request, then add each resolved track.
//!
//! Scope is intentionally **playlists only**. Liked items and saved
//! albums in the envelope are ignored — those would force widening
//! [`crate::permissions::Profile::Playlist`] to allow library writes,
//! which is out of scope for the migration use case.
//!
//! Existing-name collision policy: skip and warn. The pre-fetch of
//! existing playlists runs even under `--dry-run` so the preview is
//! realistic; it is read-only.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::export_schema::{Envelope, Playlist, SCHEMA_VERSION, Track};
use crate::output;
use crate::permissions::{self, Profile};
use crate::providers::Provider;
use crate::zad_client::{self, map_zad};

/// Run the import.
pub fn run(provider: Provider, input_path: Option<&Path>, dry_run: bool, wait: bool) -> Result<()> {
    let (policy_path, _wrote) = permissions::ensure_default_for(provider, Profile::Playlist)?;

    let _scope = output::section(
        &format!("spotifai import ({})", provider.display_name()),
        "import",
    );
    output::detail(&format!("permissions: {}", policy_path.display()));
    if dry_run {
        output::detail("dry-run: no playlists will be created or modified");
    }

    let raw = read_input(input_path)?;
    let envelope: Envelope = parse_envelope(&raw)?;
    validate_schema_version(&envelope)?;

    let cross_provider = envelope.source.service != provider.as_str();
    if cross_provider {
        output::detail(&format!(
            "cross-provider migration: source `{}` → target `{}`; resolving tracks via search",
            envelope.source.service,
            provider.as_str()
        ));
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;

    rt.block_on(async {
        match provider {
            Provider::Spotify => run_spotify(envelope, dry_run, cross_provider, wait).await,
            Provider::YouTubeMusic => run_ymusic(envelope, dry_run, cross_provider, wait).await,
        }
    })
}

// ---------------------------------------------------------------------------
// Spotify importer
// ---------------------------------------------------------------------------

async fn run_spotify(
    envelope: Envelope,
    dry_run: bool,
    cross_provider: bool,
    wait: bool,
) -> Result<()> {
    use zad::service::spotify::{CreatePlaylistRequest, PlaylistsRequest, SearchRequest};

    let client = zad_client::load_spotify_all()?;
    let http = zad_client::load_spotify_http(spotify_import_scopes())?;

    output::action("fetching existing playlists on target");
    zad_client::precall_check(Provider::Spotify, wait).await?;
    let existing = client
        .playlists(PlaylistsRequest::all())
        .await
        .map_err(map_zad)?;
    let existing_names: BTreeSet<String> = existing
        .iter()
        .map(|p| p.name.trim().to_ascii_lowercase())
        .collect();
    output::detail(&format!("{} existing playlists on target", existing.len()));

    let mut report = ImportReport::default();
    output::detail(&format!(
        "{} playlists in envelope",
        envelope.playlists.len()
    ));
    for playlist in envelope.playlists {
        let name = playlist.name.trim().to_string();
        if name.is_empty() {
            output::warn("skipping playlist with empty name");
            report.playlists_failed += 1;
            continue;
        }
        if existing_names.contains(&name.to_ascii_lowercase()) {
            output::warn(&format!(
                "playlist `{name}` already exists on target — skipping"
            ));
            report.playlists_skipped_duplicate += 1;
            continue;
        }

        // Resolve each track to a Spotify URI.
        let mut uris: Vec<String> = Vec::with_capacity(playlist.tracks.len());
        for track in &playlist.tracks {
            let resolved = if cross_provider {
                resolve_spotify_via_search(track, |q, types| async {
                    zad_client::precall_check(Provider::Spotify, wait).await?;
                    let req = SearchRequest::new(q, types, 1).map_err(map_zad)?;
                    let res = client.search(req).await.map_err(map_zad)?;
                    Ok(res
                        .tracks
                        .as_ref()
                        .and_then(|p| p.items.first())
                        .map(|t| t.id.clone()))
                })
                .await?
            } else {
                track.source_id_for("spotify").map(str::to_string)
            };
            match resolved {
                Some(id) => uris.push(spotify_uri_for(&id)),
                None => {
                    output::warn(&format!(
                        "could not resolve `{}` on Spotify",
                        track_label(track)
                    ));
                    report.tracks_unresolved += 1;
                }
            }
        }

        if dry_run {
            output::detail(&format!("would create `{name}` with {} tracks", uris.len()));
            report.playlists_created += 1;
            report.tracks_added += uris.len();
            continue;
        }

        let req = CreatePlaylistRequest::new(
            name.clone(),
            playlist.description.clone(),
            playlist.public.unwrap_or(false),
        )
        .map_err(map_zad)?;
        zad_client::precall_check(Provider::Spotify, wait).await?;
        let created = match client.create_playlist(req).await {
            Ok(p) => p,
            Err(e) => {
                output::warn(&format!("`create_playlist` failed for `{name}`: {e}"));
                report.playlists_failed += 1;
                continue;
            }
        };

        // Spotify caps `add_playlist_tracks` at 100 URIs per call.
        let mut added = 0usize;
        for chunk in uris.chunks(100) {
            let chunk_vec: Vec<String> = chunk.to_vec();
            zad_client::precall_check(Provider::Spotify, wait).await?;
            match http.add_playlist_tracks(&created.id, &chunk_vec).await {
                Ok(()) => added += chunk.len(),
                Err(e) => {
                    output::warn(&format!("`add_playlist_tracks` failed for `{name}`: {e}"));
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

fn spotify_import_scopes() -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    s.insert("search".into());
    s.insert("playlists.write".into());
    s
}

fn spotify_uri_for(id: &str) -> String {
    if id.starts_with("spotify:") {
        id.to_string()
    } else {
        format!("spotify:track:{id}")
    }
}

async fn resolve_spotify_via_search<F, Fut>(track: &Track, mut search: F) -> Result<Option<String>>
where
    F: FnMut(String, Vec<String>) -> Fut,
    Fut: std::future::Future<Output = Result<Option<String>>>,
{
    if let Some(isrc) = track.isrc.as_deref()
        && !isrc.trim().is_empty()
    {
        let q = format!("isrc:{}", isrc.trim());
        if let Some(id) = search(q, vec!["track".into()]).await? {
            return Ok(Some(id));
        }
    }
    if let Some(q) = track.search_query() {
        if let Some(id) = search(q, vec!["track".into()]).await? {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// YouTube Music importer
// ---------------------------------------------------------------------------

async fn run_ymusic(
    envelope: Envelope,
    dry_run: bool,
    cross_provider: bool,
    wait: bool,
) -> Result<()> {
    use zad::service::ymusic::client::Privacy;
    use zad::service::ymusic::{
        AddPlaylistItemRequest, CreatePlaylistRequest, PlaylistsRequest, SearchRequest,
    };

    let client = zad_client::load_ymusic_all()?;

    output::action("fetching existing playlists on target");
    zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
    let existing = client
        .playlists(PlaylistsRequest::all())
        .await
        .map_err(map_zad)?;
    let existing_names: BTreeSet<String> = existing
        .iter()
        .filter_map(|p| {
            p.snippet
                .as_ref()
                .map(|s| s.title.trim().to_ascii_lowercase())
        })
        .collect();
    output::detail(&format!("{} existing playlists on target", existing.len()));

    let mut report = ImportReport::default();
    output::detail(&format!(
        "{} playlists in envelope",
        envelope.playlists.len()
    ));
    for playlist in envelope.playlists {
        let name = playlist.name.trim().to_string();
        if name.is_empty() {
            output::warn("skipping playlist with empty name");
            report.playlists_failed += 1;
            continue;
        }
        if existing_names.contains(&name.to_ascii_lowercase()) {
            output::warn(&format!(
                "playlist `{name}` already exists on target — skipping"
            ));
            report.playlists_skipped_duplicate += 1;
            continue;
        }

        let mut video_ids: Vec<String> = Vec::with_capacity(playlist.tracks.len());
        for track in &playlist.tracks {
            let resolved = if cross_provider {
                resolve_ymusic_via_search(track, |q| async {
                    zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
                    let req = SearchRequest::new(q, vec!["video".into()], 1).map_err(map_zad)?;
                    let res = client.search(req).await.map_err(map_zad)?;
                    Ok(res
                        .into_iter()
                        .next()
                        .and_then(|item| item.id.and_then(|id| id.video_id)))
                })
                .await?
            } else {
                track.source_id_for("ymusic").map(str::to_string)
            };
            match resolved {
                Some(id) => video_ids.push(id),
                None => {
                    output::warn(&format!(
                        "could not resolve `{}` on YouTube Music",
                        track_label(track)
                    ));
                    report.tracks_unresolved += 1;
                }
            }
        }

        if dry_run {
            output::detail(&format!(
                "would create `{name}` with {} videos",
                video_ids.len()
            ));
            report.playlists_created += 1;
            report.tracks_added += video_ids.len();
            continue;
        }

        let privacy = if playlist.public.unwrap_or(false) {
            Privacy::Public
        } else {
            Privacy::Private
        };
        let req = CreatePlaylistRequest::new(name.clone(), playlist.description.clone(), privacy)
            .map_err(map_zad)?;
        zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
        let created = match client.create_playlist(req).await {
            Ok(p) => p,
            Err(e) => {
                output::warn(&format!("`create_playlist` failed for `{name}`: {e}"));
                report.playlists_failed += 1;
                continue;
            }
        };

        let mut added = 0usize;
        for video_id in &video_ids {
            let req = AddPlaylistItemRequest::new(created.id.clone(), video_id.clone())
                .map_err(map_zad)?;
            zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
            match client.add_playlist_item(req).await {
                Ok(_) => added += 1,
                Err(e) => {
                    output::warn(&format!("`add_playlist_item` failed for `{name}`: {e}"));
                    report.tracks_failed += 1;
                }
            }
        }
        report.playlists_created += 1;
        report.tracks_added += added;
        output::status(&format!("created `{name}` with {added} videos"));
    }

    output::status(&import_summary_line(&report));
    Ok(())
}

async fn resolve_ymusic_via_search<F, Fut>(track: &Track, mut search: F) -> Result<Option<String>>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<Option<String>>>,
{
    // YouTube has no ISRC search support — fall straight to title +
    // artist text search.
    if let Some(q) = track.search_query() {
        if let Some(id) = search(q).await? {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

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

/// Parse the envelope JSON text. Fatal on malformed JSON or
/// schema-violating shape.
pub fn parse_envelope(raw: &str) -> Result<Envelope> {
    serde_json::from_str(raw).context("parsing envelope JSON")
}

/// Verify the envelope's `schema_version` matches what this build
/// can consume.
pub fn validate_schema_version(envelope: &Envelope) -> Result<()> {
    if envelope.schema_version == SCHEMA_VERSION {
        Ok(())
    } else {
        bail!(
            "unsupported envelope `schema_version`: `{}`; this build expects `{SCHEMA_VERSION}`",
            envelope.schema_version
        )
    }
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

/// Case-insensitive, trimmed comparison. "Focus" and "focus " are
/// the same playlist for the purposes of duplicate-skip.
pub fn is_duplicate_name(name: &str, existing: &[String]) -> bool {
    let needle = name.trim().to_ascii_lowercase();
    existing
        .iter()
        .any(|e| e.trim().to_ascii_lowercase() == needle)
}

/// Render a track for status output.
pub fn track_label(track: &Track) -> String {
    let title = track.title.trim();
    let title = if title.is_empty() { "<unknown>" } else { title };
    match track.primary_artist() {
        Some(a) => format!("{title} — {a}"),
        None => title.to_string(),
    }
}

/// Walk a [`Playlist`] and return tracks that have at least one
/// piece of resolvable information (a source id, an ISRC, or a
/// title). Useful for unit tests of the resolver.
pub fn resolvable_tracks(playlist: &Playlist) -> impl Iterator<Item = &Track> {
    playlist.tracks.iter().filter(|t| {
        !t.source_ids.is_empty()
            || t.isrc
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
            || !t.title.trim().is_empty()
    })
}
