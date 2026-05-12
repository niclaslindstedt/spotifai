//! `spotifai export` — pull the user's library on the active
//! provider into the unified spotifai schema (see
//! [`crate::export_schema`] and `docs/export_schema.md`).
//!
//! Reads happen through the in-process zad library facades —
//! `zad::service::spotify::Spotify` and `zad::service::ymusic::Ymusic`,
//! plus a few low-level `SpotifyHttp` / `YmusicHttp` calls for verbs
//! the typed facades don't yet expose. The provider-specific
//! response shapes are folded into [`export_schema::Envelope`] so
//! the final document is identical regardless of source: `tracks` is
//! liked songs / liked videos, `albums` is saved albums (Spotify
//! only — ymusic emits an empty list), `playlists` is the full
//! ordered set with each track embedded under the same `Track`
//! schema.
//!
//! Pagination: zad's typed list endpoints walk the upstream cursor
//! under the hood when handed `None`, so the export captures the
//! whole library regardless of size.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::export_schema::{
    Envelope, SpotifyExportData, YmusicExportData, build_envelope_from_spotify,
    build_envelope_from_ymusic,
};
use crate::output;
use crate::permissions::{self, Profile};
use crate::providers::Provider;
use crate::zad_client;

/// Run the export.
///
/// `output_path` redirects the JSON to a file; `None` writes to
/// stdout. `pretty` toggles two-space indentation. Status messages
/// always go to stderr via [`crate::output`] so the JSON on stdout
/// stays pipe-clean.
pub fn run(provider: Provider, output_path: Option<&Path>, pretty: bool, wait: bool) -> Result<()> {
    // Materialize the read-only profile file before talking to zad
    // so a fresh user gets a sensible default scaffolded.
    let (policy_path, _wrote) = permissions::ensure_default_for(provider, Profile::Ask)?;

    output::header(&format!("spotifai export ({})", provider.display_name()));
    output::info(&format!("permissions: {}", policy_path.display()));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    let envelope = rt.block_on(async {
        match provider {
            Provider::Spotify => collect_spotify(wait).await,
            Provider::YouTubeMusic => collect_ymusic(wait).await,
        }
    })?;

    let serialized = if pretty {
        serde_json::to_string_pretty(&envelope)?
    } else {
        serde_json::to_string(&envelope)?
    };
    write_output(output_path, &serialized)?;

    let n_tracks = envelope.tracks.len();
    let n_albums = envelope.albums.len();
    let n_playlists = envelope.playlists.len();
    let n_playlist_tracks: usize = envelope.playlists.iter().map(|p| p.tracks.len()).sum();
    output::status(&format!(
        "exported {n_tracks} liked items, {n_albums} albums, \
         {n_playlists} playlists ({n_playlist_tracks} playlist tracks)"
    ));
    Ok(())
}

async fn collect_spotify(wait: bool) -> Result<Envelope> {
    use zad::service::spotify::{PlaylistsRequest, SavedTracksRequest};

    let identity = zad_client::read_self_identity(Provider::Spotify)?;
    let client = zad_client::load_spotify_all()?;
    let http = zad_client::load_spotify_http(spotify_export_scopes())?;

    output::info("fetching liked tracks…");
    zad_client::precall_check(Provider::Spotify, wait).await?;
    let saved_tracks = client
        .saved_tracks(SavedTracksRequest::all())
        .await
        .map_err(map_zad)?;
    output::info(&format!("  {} liked tracks", saved_tracks.len()));

    output::info("fetching saved albums…");
    zad_client::precall_check(Provider::Spotify, wait).await?;
    let saved_albums = http.list_saved_albums(None).await.map_err(map_zad)?;
    output::info(&format!("  {} saved albums", saved_albums.len()));

    output::info("fetching playlists…");
    zad_client::precall_check(Provider::Spotify, wait).await?;
    let summaries = client
        .playlists(PlaylistsRequest::all())
        .await
        .map_err(map_zad)?;
    output::info(&format!("  {} playlists", summaries.len()));

    let mut playlists_with_tracks = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let id = summary.id.clone();
        zad_client::precall_check(Provider::Spotify, wait).await?;
        match http.get_playlist_tracks(&id, None).await {
            Ok(items) => {
                playlists_with_tracks.push((summary, items));
            }
            Err(e) => {
                output::warn(&format!("playlist `{}` ({id}) skipped: {e}", summary.name));
                playlists_with_tracks.push((summary, Vec::new()));
            }
        }
    }

    let data = SpotifyExportData {
        user_id: identity.user_id,
        user_display_name: identity.display_name,
        saved_tracks,
        saved_albums,
        playlists: playlists_with_tracks,
    };
    Ok(build_envelope_from_spotify(
        data,
        iso8601_now(),
        crate::version(),
    ))
}

async fn collect_ymusic(wait: bool) -> Result<Envelope> {
    use zad::service::ymusic::{LikedRequest, PlaylistsRequest};

    let identity = zad_client::read_self_identity(Provider::YouTubeMusic)?;
    let client = zad_client::load_ymusic_all()?;
    let http = zad_client::load_ymusic_http(ymusic_export_scopes())?;

    output::info("fetching liked videos…");
    zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
    let liked_videos = client.liked(LikedRequest::all()).await.map_err(map_zad)?;
    output::info(&format!("  {} liked videos", liked_videos.len()));

    output::info("fetching playlists…");
    zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
    let summaries = client
        .playlists(PlaylistsRequest::all())
        .await
        .map_err(map_zad)?;
    output::info(&format!("  {} playlists", summaries.len()));

    let mut playlists_with_items = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let id = summary.id.clone();
        let title_label = summary
            .snippet
            .as_ref()
            .map(|s| s.title.clone())
            .unwrap_or_else(|| id.clone());
        zad_client::precall_check(Provider::YouTubeMusic, wait).await?;
        match http.get_playlist_items(&id, None).await {
            Ok(items) => {
                playlists_with_items.push((summary, items));
            }
            Err(e) => {
                output::warn(&format!("playlist `{title_label}` ({id}) skipped: {e}"));
                playlists_with_items.push((summary, Vec::new()));
            }
        }
    }

    let data = YmusicExportData {
        channel_id: identity.channel_id,
        channel_title: identity.display_name,
        liked_videos,
        playlists: playlists_with_items,
    };
    Ok(build_envelope_from_ymusic(
        data,
        iso8601_now(),
        crate::version(),
    ))
}

fn spotify_export_scopes() -> std::collections::BTreeSet<String> {
    let mut s = std::collections::BTreeSet::new();
    s.insert("library.read".into());
    s.insert("playlists.read".into());
    s
}

fn ymusic_export_scopes() -> std::collections::BTreeSet<String> {
    let mut s = std::collections::BTreeSet::new();
    s.insert("library.read".into());
    s.insert("playlists.read".into());
    s
}

fn map_zad(e: zad::ZadError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
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
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}
