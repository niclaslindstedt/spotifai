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
//!
//! Resume: progress is persisted to
//! `~/.spotifai/import-state/<provider>-<fingerprint>.json` (see
//! [`crate::import_state`]). On re-run, completed playlists are
//! skipped and in-progress ones resume from the last saved track
//! offset. Pass `--no-resume` to ignore any prior state.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::export_schema::{Envelope, Playlist, SCHEMA_VERSION, Track};
use crate::import_state::{self, ImportState, PlaylistState, PlaylistStatus};
use crate::output;
use crate::permissions::{self, Profile};
use crate::providers::Provider;
use crate::zad_client::{self, map_zad};

/// Run the import.
pub fn run(
    provider: Provider,
    input_path: Option<&Path>,
    dry_run: bool,
    wait: bool,
    no_resume: bool,
) -> Result<()> {
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
    output::info(&format!(
        "envelope: source `{}` exported {} ({} playlists)",
        envelope.source.service,
        envelope.exported_at,
        envelope.playlists.len(),
    ));
    if cross_provider {
        output::detail(&format!(
            "cross-provider migration: source `{}` → target `{}`; resolving tracks via search",
            envelope.source.service,
            provider.as_str()
        ));
    }

    let fp = import_state::fingerprint(&envelope, provider);
    let state_path = import_state::state_path(provider, &fp)?;
    let state = if dry_run {
        ImportState::new(fp.clone(), provider)
    } else if no_resume {
        import_state::clear(&state_path)?;
        output::detail("--no-resume: ignoring any prior state");
        ImportState::new(fp.clone(), provider)
    } else {
        match import_state::load(&state_path)? {
            Some(prior) => {
                let c = prior.counts();
                output::info(&format!(
                    "resuming from {} (started {}, last update {})",
                    state_path.display(),
                    prior.started_at,
                    prior.last_updated_at,
                ));
                output::detail(&format!(
                    "previously completed: {} done, {} skipped duplicate, \
                     {} in progress, {} failed create; {} tracks added so far",
                    c.completed,
                    c.skipped_duplicate,
                    c.in_progress,
                    c.failed_create,
                    c.tracks_added,
                ));
                prior
            }
            None => ImportState::new(fp.clone(), provider),
        }
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;

    let result = rt.block_on(async {
        match provider {
            Provider::Spotify => {
                run_spotify(envelope, dry_run, cross_provider, wait, state, &state_path).await
            }
            Provider::YouTubeMusic => {
                run_ymusic(envelope, dry_run, cross_provider, wait, state, &state_path).await
            }
        }
    });

    match result {
        Ok((report, state)) => {
            if !dry_run {
                let _ = import_state::save(&state, &state_path);
                if every_terminal(&state) {
                    let _ = import_state::clear(&state_path);
                    output::detail("all playlists processed; cleared state file");
                } else {
                    output::detail(&format!("state saved to {}", state_path.display()));
                }
            }
            output::status(&import_summary_line(&report));
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn every_terminal(state: &ImportState) -> bool {
    state.playlists.values().all(PlaylistState::is_terminal)
}

// ---------------------------------------------------------------------------
// Spotify importer
// ---------------------------------------------------------------------------

async fn run_spotify(
    envelope: Envelope,
    dry_run: bool,
    cross_provider: bool,
    wait: bool,
    mut state: ImportState,
    state_path: &Path,
) -> Result<(ImportReport, ImportState)> {
    use zad::service::spotify::{CreatePlaylistRequest, PlaylistsRequest};

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
    let total = envelope.playlists.len();
    output::detail(&format!("{total} playlists in envelope"));

    for (idx, playlist) in envelope.playlists.into_iter().enumerate() {
        let name = playlist.name.trim().to_string();
        let header_label = format!("`{name}` ({} tracks)", playlist.tracks.len());
        output::step(idx + 1, total, &format!("importing {header_label}"));

        if name.is_empty() {
            output::warn("skipping playlist with empty name");
            report.playlists_failed += 1;
            continue;
        }

        if let Some(prior) = state.get(&name)
            && prior.is_terminal()
        {
            tally_terminal(&mut report, prior);
            output::detail(&format!(
                "previously {} — skipping",
                terminal_label(prior.status)
            ));
            continue;
        }

        if existing_names.contains(&name.to_ascii_lowercase())
            && state.get(&name).is_none_or(|s| s.target_id.is_none())
        {
            output::warn(&format!(
                "playlist `{name}` already exists on target — skipping"
            ));
            report.playlists_skipped_duplicate += 1;
            if !dry_run {
                state.upsert(&name, PlaylistState::new_skipped_duplicate());
                let _ = import_state::save(&state, state_path);
            }
            continue;
        }

        // Resolve (or reuse cached resolution).
        let prior = state.get(&name).cloned();
        let (resolved_ids, unresolved) = match &prior {
            Some(s) if !s.resolved_track_ids.is_empty() => {
                output::detail(&format!(
                    "reusing {} resolved tracks from prior run ({} unresolved)",
                    s.resolved_track_ids.len(),
                    s.unresolved_count,
                ));
                (s.resolved_track_ids.clone(), s.unresolved_count)
            }
            _ => {
                output::action(&format!("resolving {} tracks", playlist.tracks.len()));
                resolve_spotify_tracks(&client, &playlist, cross_provider, wait).await?
            }
        };
        report.tracks_unresolved += unresolved;
        let uris: Vec<String> = resolved_ids.iter().map(|id| spotify_uri_for(id)).collect();

        if dry_run {
            output::detail(&format!("would create `{name}` with {} tracks", uris.len()));
            report.playlists_created += 1;
            report.tracks_added += uris.len();
            continue;
        }

        let target_id = match prior.as_ref().and_then(|s| s.target_id.clone()) {
            Some(id) => {
                output::detail(&format!("reusing prior playlist id `{id}`"));
                id
            }
            None => {
                output::action(&format!("creating playlist `{name}` on target"));
                let req = CreatePlaylistRequest::new(
                    name.clone(),
                    playlist.description.clone(),
                    playlist.public.unwrap_or(false),
                )
                .map_err(map_zad)?;
                zad_client::precall_check(Provider::Spotify, wait).await?;
                match client.create_playlist(req).await {
                    Ok(p) => {
                        state.upsert(
                            &name,
                            PlaylistState {
                                status: PlaylistStatus::InProgress,
                                target_id: Some(p.id.clone()),
                                resolved_track_ids: resolved_ids.clone(),
                                tracks_processed: 0,
                                unresolved_count: unresolved,
                                tracks_added: 0,
                                tracks_failed: 0,
                            },
                        );
                        let _ = import_state::save(&state, state_path);
                        p.id
                    }
                    Err(e) => {
                        if is_rate_limit_error(&e) {
                            return Err(rate_limit_bail(
                                &name,
                                "create_playlist",
                                e,
                                state,
                                state_path,
                                &report,
                            ));
                        }
                        output::warn(&format!("`create_playlist` failed for `{name}`: {e}"));
                        state.upsert(&name, PlaylistState::new_failed_create());
                        let _ = import_state::save(&state, state_path);
                        report.playlists_failed += 1;
                        continue;
                    }
                }
            }
        };

        // Resume mid-add: skip past tracks the prior run already added.
        let already_added = prior.as_ref().map(|s| s.tracks_added).unwrap_or(0);
        let prior_failed = prior.as_ref().map(|s| s.tracks_failed).unwrap_or(0);
        let remaining: Vec<String> = uris.iter().skip(already_added).cloned().collect();
        if already_added > 0 {
            output::detail(&format!(
                "{already_added} of {} tracks already added in a prior run; \
                 continuing with {} remaining",
                uris.len(),
                remaining.len(),
            ));
        }

        // Spotify caps `add_playlist_tracks` at 100 URIs per call, so
        // we keep the batched insert here even though ymusic interleaves
        // resolve+insert per track. Resolution already ran above, so
        // every source track is either in `resolved_ids` or counted as
        // unresolved — set `tracks_processed` to the full source length.
        let tracks_processed_total = playlist.tracks.len();
        let mut added_now = 0usize;
        let mut failed_now = 0usize;
        let mut hit_rate_limit: Option<zad::ZadError> = None;
        for chunk in remaining.chunks(100) {
            zad_client::precall_check(Provider::Spotify, wait).await?;
            let chunk_vec: Vec<String> = chunk.to_vec();
            match http.add_playlist_tracks(&target_id, &chunk_vec).await {
                Ok(()) => {
                    added_now += chunk.len();
                    persist_progress(
                        &mut state,
                        state_path,
                        &name,
                        &target_id,
                        &resolved_ids,
                        tracks_processed_total,
                        unresolved,
                        already_added + added_now,
                        prior_failed + failed_now,
                        PlaylistStatus::InProgress,
                    );
                }
                Err(e) => {
                    if is_rate_limit_error(&e) {
                        hit_rate_limit = Some(e);
                        break;
                    }
                    output::warn(&format!("`add_playlist_tracks` failed for `{name}`: {e}"));
                    failed_now += chunk.len();
                }
            }
        }

        let total_added = already_added + added_now;
        let total_failed = prior_failed + failed_now;

        if let Some(e) = hit_rate_limit {
            persist_progress(
                &mut state,
                state_path,
                &name,
                &target_id,
                &resolved_ids,
                tracks_processed_total,
                unresolved,
                total_added,
                total_failed,
                PlaylistStatus::InProgress,
            );
            report.tracks_added += added_now;
            report.tracks_failed += failed_now;
            return Err(rate_limit_bail(
                &name,
                "add_playlist_tracks",
                e,
                state,
                state_path,
                &report,
            ));
        }

        persist_progress(
            &mut state,
            state_path,
            &name,
            &target_id,
            &resolved_ids,
            tracks_processed_total,
            unresolved,
            total_added,
            total_failed,
            PlaylistStatus::Completed,
        );
        report.playlists_created += 1;
        report.tracks_added += added_now;
        report.tracks_failed += failed_now;
        output::status(&format!(
            "created `{name}` with {total_added}/{} tracks ({unresolved} unresolved, {total_failed} failed adds)",
            resolved_ids.len(),
        ));
    }

    Ok((report, state))
}

async fn resolve_spotify_tracks(
    client: &zad::service::spotify::Spotify,
    playlist: &Playlist,
    cross_provider: bool,
    wait: bool,
) -> Result<(Vec<String>, usize)> {
    use zad::service::spotify::SearchRequest;

    let mut ids: Vec<String> = Vec::with_capacity(playlist.tracks.len());
    let mut unresolved = 0usize;
    let total = playlist.tracks.len();
    for (idx, track) in playlist.tracks.iter().enumerate() {
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
            Some(id) => {
                output::detail(&format!(
                    "[{}/{}] resolved `{}` → {}",
                    idx + 1,
                    total,
                    track_label(track),
                    id
                ));
                ids.push(id);
            }
            None => {
                output::warn(&format!(
                    "[{}/{}] could not resolve `{}` on Spotify",
                    idx + 1,
                    total,
                    track_label(track)
                ));
                unresolved += 1;
            }
        }
    }
    Ok((ids, unresolved))
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
    if let Some(q) = track.search_query()
        && let Some(id) = search(q, vec!["track".into()]).await?
    {
        return Ok(Some(id));
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
    mut state: ImportState,
    state_path: &Path,
) -> Result<(ImportReport, ImportState)> {
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
    let total = envelope.playlists.len();
    output::detail(&format!("{total} playlists in envelope"));

    for (idx, playlist) in envelope.playlists.into_iter().enumerate() {
        let name = playlist.name.trim().to_string();
        let header_label = format!("`{name}` ({} tracks)", playlist.tracks.len());
        output::step(idx + 1, total, &format!("importing {header_label}"));

        if name.is_empty() {
            output::warn("skipping playlist with empty name");
            report.playlists_failed += 1;
            continue;
        }

        if let Some(prior) = state.get(&name)
            && prior.is_terminal()
        {
            tally_terminal(&mut report, prior);
            output::detail(&format!(
                "previously {} — skipping",
                terminal_label(prior.status)
            ));
            continue;
        }

        if existing_names.contains(&name.to_ascii_lowercase())
            && state.get(&name).is_none_or(|s| s.target_id.is_none())
        {
            output::warn(&format!(
                "playlist `{name}` already exists on target — skipping"
            ));
            report.playlists_skipped_duplicate += 1;
            if !dry_run {
                state.upsert(&name, PlaylistState::new_skipped_duplicate());
                let _ = import_state::save(&state, state_path);
            }
            continue;
        }

        if dry_run {
            output::detail(&format!(
                "would create `{name}` and add up to {} videos",
                playlist.tracks.len(),
            ));
            report.playlists_created += 1;
            report.tracks_added += playlist.tracks.len();
            continue;
        }

        let prior = state.get(&name).cloned();
        let privacy = if playlist.public.unwrap_or(false) {
            Privacy::Public
        } else {
            Privacy::Private
        };

        // Create or reuse the target playlist *before* resolving any
        // tracks, so a quota-bounded run resolves+inserts one track at
        // a time rather than burning the entire budget on searches.
        let target_id = match prior.as_ref().and_then(|s| s.target_id.clone()) {
            Some(id) => {
                output::detail(&format!("reusing prior playlist id `{id}`"));
                id
            }
            None => {
                output::action(&format!("creating playlist `{name}` on target"));
                let req =
                    CreatePlaylistRequest::new(name.clone(), playlist.description.clone(), privacy)
                        .map_err(map_zad)?;
                zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
                match client.create_playlist(req).await {
                    Ok(p) => {
                        state.upsert(
                            &name,
                            PlaylistState {
                                status: PlaylistStatus::InProgress,
                                target_id: Some(p.id.clone()),
                                resolved_track_ids: Vec::new(),
                                tracks_processed: 0,
                                unresolved_count: 0,
                                tracks_added: 0,
                                tracks_failed: 0,
                            },
                        );
                        let _ = import_state::save(&state, state_path);
                        p.id
                    }
                    Err(e) => {
                        if is_rate_limit_error(&e) {
                            return Err(rate_limit_bail(
                                &name,
                                "create_playlist",
                                e,
                                state,
                                state_path,
                                &report,
                            ));
                        }
                        output::warn(&format!("`create_playlist` failed for `{name}`: {e}"));
                        state.upsert(&name, PlaylistState::new_failed_create());
                        let _ = import_state::save(&state, state_path);
                        report.playlists_failed += 1;
                        continue;
                    }
                }
            }
        };

        let mut resolved_ids = prior
            .as_ref()
            .map(|s| s.resolved_track_ids.clone())
            .unwrap_or_default();
        let cursor_start = prior.as_ref().map(|s| s.tracks_processed).unwrap_or(0);
        let mut tracks_processed = cursor_start;
        let mut unresolved = prior.as_ref().map(|s| s.unresolved_count).unwrap_or(0);
        let mut tracks_added = prior.as_ref().map(|s| s.tracks_added).unwrap_or(0);
        let mut tracks_failed = prior.as_ref().map(|s| s.tracks_failed).unwrap_or(0);
        let total_tracks = playlist.tracks.len();

        if cursor_start > 0 {
            output::detail(&format!(
                "resuming at track {}/{} ({tracks_added} added, {unresolved} unresolved, \
                 {tracks_failed} failed)",
                cursor_start + 1,
                total_tracks,
            ));
        }

        let mut bail_reason: Option<(&'static str, zad::ZadError)> = None;
        for (offset, track) in playlist.tracks.iter().skip(cursor_start).enumerate() {
            let idx = cursor_start + offset;
            let label = track_label(track);

            // Stage 1 — resolve the source track to a video id.
            let resolved = if cross_provider {
                let search_outcome = resolve_ymusic_via_search(track, |q| async {
                    zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
                    let req = SearchRequest::new(q, vec!["video".into()], 1).map_err(map_zad)?;
                    let res = client.search(req).await.map_err(map_zad)?;
                    Ok(res
                        .into_iter()
                        .next()
                        .and_then(|item| item.id.and_then(|id| id.video_id)))
                })
                .await;
                match search_outcome {
                    Ok(id) => id,
                    Err(e) => {
                        if anyhow_is_rate_limited(&e) {
                            bail_reason = Some((
                                "search",
                                zad::ZadError::Service {
                                    name: "ymusic",
                                    message: format!("{e}"),
                                },
                            ));
                            break;
                        }
                        return Err(e);
                    }
                }
            } else {
                track.source_id_for("ymusic").map(str::to_string)
            };

            let Some(video_id) = resolved else {
                output::warn(&format!(
                    "[{}/{}] could not resolve `{label}` on YouTube Music",
                    idx + 1,
                    total_tracks,
                ));
                unresolved += 1;
                tracks_processed = idx + 1;
                persist_progress(
                    &mut state,
                    state_path,
                    &name,
                    &target_id,
                    &resolved_ids,
                    tracks_processed,
                    unresolved,
                    tracks_added,
                    tracks_failed,
                    PlaylistStatus::InProgress,
                );
                continue;
            };

            output::detail(&format!(
                "[{}/{}] resolved `{label}` → {video_id}",
                idx + 1,
                total_tracks,
            ));
            resolved_ids.push(video_id.clone());

            // Stage 2 — add it to the target playlist immediately.
            let req = AddPlaylistItemRequest::new(target_id.clone(), video_id.clone())
                .map_err(map_zad)?;
            zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
            match client.add_playlist_item(req).await {
                Ok(_) => {
                    tracks_added += 1;
                    output::detail(&format!(
                        "[{}/{}] added `{label}` → `{name}`",
                        idx + 1,
                        total_tracks,
                    ));
                }
                Err(e) => {
                    if is_rate_limit_error(&e) {
                        bail_reason = Some(("add_playlist_item", e));
                        break;
                    }
                    output::warn(&format!(
                        "[{}/{}] `add_playlist_item` failed for `{label}`: {e}",
                        idx + 1,
                        total_tracks,
                    ));
                    tracks_failed += 1;
                }
            }

            tracks_processed = idx + 1;
            persist_progress(
                &mut state,
                state_path,
                &name,
                &target_id,
                &resolved_ids,
                tracks_processed,
                unresolved,
                tracks_added,
                tracks_failed,
                PlaylistStatus::InProgress,
            );
        }

        let prior_added = prior.as_ref().map(|s| s.tracks_added).unwrap_or(0);
        let prior_unresolved = prior.as_ref().map(|s| s.unresolved_count).unwrap_or(0);
        let prior_failed = prior.as_ref().map(|s| s.tracks_failed).unwrap_or(0);
        report.tracks_added += tracks_added.saturating_sub(prior_added);
        report.tracks_unresolved += unresolved.saturating_sub(prior_unresolved);
        report.tracks_failed += tracks_failed.saturating_sub(prior_failed);

        if let Some((op, err)) = bail_reason {
            persist_progress(
                &mut state,
                state_path,
                &name,
                &target_id,
                &resolved_ids,
                tracks_processed,
                unresolved,
                tracks_added,
                tracks_failed,
                PlaylistStatus::InProgress,
            );
            return Err(rate_limit_bail(&name, op, err, state, state_path, &report));
        }

        persist_progress(
            &mut state,
            state_path,
            &name,
            &target_id,
            &resolved_ids,
            total_tracks,
            unresolved,
            tracks_added,
            tracks_failed,
            PlaylistStatus::Completed,
        );
        report.playlists_created += 1;
        output::status(&format!(
            "created `{name}` with {tracks_added}/{total_tracks} videos \
             ({unresolved} unresolved, {tracks_failed} failed adds)",
        ));
    }

    Ok((report, state))
}

async fn resolve_ymusic_via_search<F, Fut>(track: &Track, mut search: F) -> Result<Option<String>>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<Option<String>>>,
{
    // YouTube has no ISRC search support — fall straight to title +
    // artist text search.
    if let Some(q) = track.search_query()
        && let Some(id) = search(q).await?
    {
        return Ok(Some(id));
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn persist_progress(
    state: &mut ImportState,
    state_path: &Path,
    name: &str,
    target_id: &str,
    resolved_ids: &[String],
    tracks_processed: usize,
    unresolved: usize,
    tracks_added: usize,
    tracks_failed: usize,
    status: PlaylistStatus,
) {
    state.upsert(
        name,
        PlaylistState {
            status,
            target_id: Some(target_id.to_string()),
            resolved_track_ids: resolved_ids.to_vec(),
            tracks_processed,
            unresolved_count: unresolved,
            tracks_added,
            tracks_failed,
        },
    );
    let _ = import_state::save(state, state_path);
}

fn tally_terminal(report: &mut ImportReport, prior: &PlaylistState) {
    match prior.status {
        PlaylistStatus::Completed => {
            report.playlists_created += 1;
            report.tracks_added += prior.tracks_added;
            report.tracks_unresolved += prior.unresolved_count;
            report.tracks_failed += prior.tracks_failed;
        }
        PlaylistStatus::SkippedDuplicate => {
            report.playlists_skipped_duplicate += 1;
        }
        PlaylistStatus::FailedCreate => {
            report.playlists_failed += 1;
        }
        PlaylistStatus::InProgress => {}
    }
}

fn terminal_label(status: PlaylistStatus) -> &'static str {
    match status {
        PlaylistStatus::Completed => "completed",
        PlaylistStatus::SkippedDuplicate => "skipped (duplicate)",
        PlaylistStatus::FailedCreate => "failed to create",
        PlaylistStatus::InProgress => "in progress",
    }
}

/// True iff a `ZadError` represents a rate-limit hit (HTTP 429 or
/// Google-quota 403). We match by formatted text because
/// `ZadError::RateLimited` is emitted both by zad's clients and by
/// our own `precall_check` wrapper, but the wrapper goes through
/// [`anyhow::Error`] in some paths — keeping the check at the string
/// surface means both paths classify the same way.
pub fn is_rate_limit_error(e: &zad::ZadError) -> bool {
    matches!(e, zad::ZadError::RateLimited { .. })
}

/// True iff an `anyhow::Error` was produced from a rate-limit
/// `ZadError`. We classify by message because the conversion through
/// `map_zad` (and through `precall_check`'s `anyhow!("{e}")` wrap)
/// erases the original variant.
pub fn anyhow_is_rate_limited(err: &anyhow::Error) -> bool {
    let s = format!("{err}");
    s.contains("rate-limited this call")
}

/// Build a fatal `anyhow::Error` for an interrupted import: the
/// stderr message tells the user how to resume, and the underlying
/// `ZadError::RateLimited` is preserved as the cause so debug logs
/// keep the full body.
fn rate_limit_bail(
    playlist_name: &str,
    op: &str,
    err: zad::ZadError,
    state: ImportState,
    state_path: &Path,
    report: &ImportReport,
) -> anyhow::Error {
    let _ = import_state::save(&state, state_path);
    let counts = state.counts();
    output::warn(&format!(
        "rate-limit hit during `{op}` on `{playlist_name}`: {err}"
    ));
    output::info(&format!(
        "progress so far: {} playlists completed, {} skipped duplicate, {} in progress, \
         {} failed; {} tracks added, {} unresolved, {} failed adds",
        counts.completed,
        counts.skipped_duplicate,
        counts.in_progress,
        counts.failed_create,
        counts.tracks_added + report.tracks_added,
        counts.tracks_unresolved + report.tracks_unresolved,
        counts.tracks_failed + report.tracks_failed,
    ));
    output::hint(&format!(
        "state saved to {} — re-run the same `spotifai import` command \
         (optionally with `--wait`) to resume from this point. \
         Pass `--no-resume` to ignore the saved state and start over.",
        state_path.display()
    ));
    anyhow::Error::msg(format!("{err}")).context(format!(
        "import interrupted by rate limit during `{op}` on `{playlist_name}`"
    ))
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
