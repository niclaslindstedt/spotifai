//! `spotifai` export schema — provider-agnostic JSON envelope.
//!
//! Every `spotifai export` writes the same shape regardless of the
//! source provider; every `spotifai import` reads it. Spotify-side
//! exporters fold Spotify response types into [`Track`] / [`Album`]
//! / [`Playlist`]; the YouTube Music exporter folds YouTube types in
//! the same way. Importers run the inverse direction, using
//! [`Track::source_ids`] for same-provider roundtrips and falling
//! back to ISRC / title-+-artist search for cross-provider
//! migrations.
//!
//! See [`docs/export_schema.md`](../docs/export_schema.md) for the
//! field-level reference, an example envelope, and notes about
//! cross-provider conversion semantics.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Schema version baked into every envelope. Bumped on breaking
/// changes so older spotifai builds fail with a clear error rather
/// than silently mis-importing.
pub const SCHEMA_VERSION: &str = "1";

/// Top-level envelope written by `spotifai export` and read by
/// `spotifai import`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Envelope {
    /// Always [`SCHEMA_VERSION`] for envelopes this build emits.
    /// `spotifai import` rejects any other value with a clear error.
    pub schema_version: String,
    /// ISO 8601 UTC timestamp at the moment the export ran.
    pub exported_at: String,
    /// Where the data came from + which spotifai produced it.
    pub source: Source,
    /// User's "favourites" — Spotify *saved tracks* and YouTube
    /// Music *liked videos* both land here. The two services treat
    /// the concept identically from the user's point of view, so
    /// the schema collapses them into one list.
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// User's saved albums. Spotify only; YouTube Music has no
    /// "saved albums" concept and the field is an empty list on
    /// ymusic exports.
    #[serde(default)]
    pub albums: Vec<Album>,
    /// User's playlists, each with the full ordered track list
    /// embedded under [`Playlist::tracks`].
    #[serde(default)]
    pub playlists: Vec<Playlist>,
}

impl Envelope {
    pub fn new(source: Source, exported_at: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.into(),
            exported_at,
            source,
            tracks: Vec::new(),
            albums: Vec::new(),
            playlists: Vec::new(),
        }
    }
}

/// `source` block on the envelope. Captures the provider data was
/// pulled from, the authenticated user, and the spotifai build that
/// produced the file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    /// Lowercase service slug. One of `"spotify"`, `"ymusic"`.
    pub service: String,
    /// The authenticated user's identity at the source provider.
    /// `id` is the upstream identifier (Spotify user id, YouTube
    /// channel id); `display_name` is the human-friendly label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Always `"spotifai"`.
    pub tool: String,
    /// `Cargo.toml` version of the spotifai build that produced the
    /// envelope. Useful for diagnostics if the schema ever evolves.
    pub tool_version: String,
}

/// Authenticated user identity at the source provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct User {
    /// Provider-specific identifier (Spotify user id, YouTube
    /// channel id).
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// One track / video. Used for "saved" / "liked" items at the top
/// level *and* for the per-playlist track list. The shape is the
/// same in both contexts.
///
/// Why one type for both Spotify tracks and YouTube videos? From the
/// user's point of view a "song they liked" is a song; the fact
/// that one provider stores a 30-second sample and the other a
/// full-length music video is below the layer we care about.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Track {
    /// Display title.
    pub title: String,
    /// Artist names in display order. Always a list of plain
    /// strings — no nested objects — so importers don't need to
    /// guess at the shape.
    #[serde(default)]
    pub artists: Vec<String>,
    /// Album / release the track belongs to. Optional because
    /// YouTube Music videos don't always carry an album.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    /// Track duration in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// International Standard Recording Code. Spotify exposes it
    /// directly via `external_ids.isrc`; YouTube Music does not.
    /// When present this is the canonical cross-provider identifier
    /// — importers try `isrc:` searches first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    /// ISO 8601 UTC timestamp the user added the track/video to
    /// their library or playlist (whichever context this `Track`
    /// appears in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
    /// Per-provider identifiers preserved from the source. On a
    /// same-provider re-import we use this verbatim and skip
    /// search; on a cross-provider import we ignore it and resolve
    /// via [`Track::isrc`] / title + artist instead.
    ///
    /// Keys are lowercase service slugs (`"spotify"`, `"ymusic"`);
    /// values are the provider-specific item IDs (Spotify track id
    /// or `spotify:track:<id>` URI, YouTube video id).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_ids: BTreeMap<String, String>,
    /// Verbatim raw record from the source provider, kept for
    /// diagnostics and to enable provider-specific tooling without
    /// re-fetching. Not consumed by `spotifai import`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

/// One saved album. Spotify only; ymusic exports leave the
/// envelope's `albums` list empty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Album {
    pub title: String,
    #[serde(default)]
    pub artists: Vec<String>,
    /// Total tracks on the album, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tracks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_ids: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

/// One playlist with its ordered track list embedded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Playlist {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `true` if the playlist is public on the source provider.
    /// Carries through to the import — the importer creates a
    /// public playlist when this is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    /// Owner of the playlist on the source provider. Most users
    /// only export playlists they own; this field also captures
    /// playlists they followed but did not author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<User>,
    /// Ordered list of tracks/videos in the playlist. Same shape
    /// as the top-level `tracks` list.
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// Per-provider playlist identifiers (so a same-provider
    /// re-import could update an existing playlist; today the
    /// importer treats each entry as a fresh playlist regardless).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_ids: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

// ---------------------------------------------------------------------------
// Mappers — zad response types → spotifai schema.
// ---------------------------------------------------------------------------

/// Spotify-specific bundle of fetched library data; the export
/// flow assembles this and then passes it to
/// [`build_envelope_from_spotify`] to convert into the unified
/// schema.
#[derive(Debug, Clone, Default)]
pub struct SpotifyExportData {
    pub user_id: Option<String>,
    pub user_display_name: Option<String>,
    pub saved_tracks: Vec<zad::service::spotify::client::SavedTrack>,
    pub saved_albums: Vec<zad::service::spotify::client::SavedAlbum>,
    /// Each entry: (summary, ordered playlist tracks).
    pub playlists: Vec<(
        zad::service::spotify::client::PlaylistSummary,
        Vec<zad::service::spotify::client::PlaylistTrackItem>,
    )>,
}

/// YouTube Music-specific bundle.
#[derive(Debug, Clone, Default)]
pub struct YmusicExportData {
    pub channel_id: Option<String>,
    pub channel_title: Option<String>,
    pub liked_videos: Vec<zad::service::ymusic::client::VideoSummary>,
    pub playlists: Vec<(
        zad::service::ymusic::client::PlaylistSummary,
        Vec<zad::service::ymusic::client::PlaylistItem>,
    )>,
}

/// Convert a Spotify-side fetch into the unified envelope.
pub fn build_envelope_from_spotify(
    data: SpotifyExportData,
    exported_at: String,
    tool_version: &str,
) -> Envelope {
    let user = data.user_id.clone().map(|id| User {
        id,
        display_name: data.user_display_name.clone(),
    });
    let mut env = Envelope::new(
        Source {
            service: "spotify".into(),
            user,
            tool: "spotifai".into(),
            tool_version: tool_version.to_string(),
        },
        exported_at,
    );
    env.tracks = data
        .saved_tracks
        .into_iter()
        .map(spotify_saved_track_to_track)
        .collect();
    env.albums = data
        .saved_albums
        .into_iter()
        .map(spotify_saved_album_to_album)
        .collect();
    env.playlists = data
        .playlists
        .into_iter()
        .map(|(summary, items)| spotify_playlist_to_playlist(summary, items))
        .collect();
    env
}

/// Convert a YouTube Music-side fetch into the unified envelope.
pub fn build_envelope_from_ymusic(
    data: YmusicExportData,
    exported_at: String,
    tool_version: &str,
) -> Envelope {
    let user = data.channel_id.clone().map(|id| User {
        id,
        display_name: data.channel_title.clone(),
    });
    let mut env = Envelope::new(
        Source {
            service: "ymusic".into(),
            user,
            tool: "spotifai".into(),
            tool_version: tool_version.to_string(),
        },
        exported_at,
    );
    env.tracks = data
        .liked_videos
        .into_iter()
        .map(ymusic_video_to_track)
        .collect();
    // `albums` is empty for ymusic by design — see Envelope docs.
    env.playlists = data
        .playlists
        .into_iter()
        .map(|(summary, items)| ymusic_playlist_to_playlist(summary, items))
        .collect();
    env
}

fn spotify_saved_track_to_track(t: zad::service::spotify::client::SavedTrack) -> Track {
    let added = t.added_at.clone();
    let mut track = spotify_track_summary_to_track(t.track);
    track.added_at = added;
    track
}

fn spotify_track_summary_to_track(t: zad::service::spotify::client::TrackSummary) -> Track {
    let mut source_ids = BTreeMap::new();
    if !t.id.is_empty() {
        source_ids.insert("spotify".into(), t.id.clone());
    }
    Track {
        title: t.name.clone(),
        artists: t.artists.iter().map(|a| a.name.clone()).collect(),
        album: t.album.as_ref().map(|a| a.name.clone()),
        duration_ms: t.duration_ms,
        // The TrackSummary projection in zad 0.8.0 doesn't expose
        // `external_ids.isrc`. We leave it None here; future zad
        // versions can fill it in and the field will start
        // populating automatically.
        isrc: None,
        added_at: None,
        source_ids,
        raw: serde_json::to_value(&t).ok(),
    }
}

fn spotify_saved_album_to_album(a: zad::service::spotify::client::SavedAlbum) -> Album {
    let added = a.added_at.clone();
    let s = a.album;
    let mut source_ids = BTreeMap::new();
    if !s.id.is_empty() {
        source_ids.insert("spotify".into(), s.id.clone());
    }
    Album {
        title: s.name.clone(),
        artists: s.artists.iter().map(|x| x.name.clone()).collect(),
        total_tracks: s.total_tracks,
        release_date: s.release_date.clone(),
        added_at: added,
        source_ids,
        raw: serde_json::to_value(&s).ok(),
    }
}

fn spotify_playlist_to_playlist(
    summary: zad::service::spotify::client::PlaylistSummary,
    items: Vec<zad::service::spotify::client::PlaylistTrackItem>,
) -> Playlist {
    let mut source_ids = BTreeMap::new();
    if !summary.id.is_empty() {
        source_ids.insert("spotify".into(), summary.id.clone());
    }
    let owner = summary.owner.as_ref().map(|o| User {
        id: o.id.clone(),
        display_name: o.display_name.clone(),
    });
    let tracks: Vec<Track> = items
        .into_iter()
        .filter_map(|it| {
            let added = it.added_at.clone();
            it.item.map(|t| {
                let mut track = spotify_track_summary_to_track(t);
                track.added_at = added;
                track
            })
        })
        .collect();
    Playlist {
        name: summary.name.clone(),
        description: summary.description.clone(),
        public: summary.public,
        owner,
        tracks,
        source_ids,
        raw: serde_json::to_value(&summary).ok(),
    }
}

fn ymusic_video_to_track(v: zad::service::ymusic::client::VideoSummary) -> Track {
    let mut source_ids = BTreeMap::new();
    if !v.id.is_empty() {
        source_ids.insert("ymusic".into(), v.id.clone());
    }
    let snippet = v.snippet.as_ref();
    let title = snippet.map(|s| s.title.clone()).unwrap_or_default();
    // YouTube Music doesn't break out an artist list separately —
    // `channelTitle` is the closest equivalent (the uploader). We
    // surface it as the sole artist so cross-provider search has
    // something to work with.
    let artists: Vec<String> = snippet
        .and_then(|s| s.channel_title.clone())
        .map(|t| vec![t])
        .unwrap_or_default();
    Track {
        title,
        artists,
        album: None,
        duration_ms: None,
        isrc: None,
        added_at: None,
        source_ids,
        raw: serde_json::to_value(&v).ok(),
    }
}

fn ymusic_playlist_to_playlist(
    summary: zad::service::ymusic::client::PlaylistSummary,
    items: Vec<zad::service::ymusic::client::PlaylistItem>,
) -> Playlist {
    let mut source_ids = BTreeMap::new();
    if !summary.id.is_empty() {
        source_ids.insert("ymusic".into(), summary.id.clone());
    }
    let snippet = summary.snippet.clone();
    let name = snippet
        .as_ref()
        .map(|s| s.title.clone())
        .unwrap_or_default();
    let description = snippet.as_ref().and_then(|s| s.description.clone());
    let public = summary
        .status
        .as_ref()
        .and_then(|s| s.privacy_status.as_deref())
        .map(|p| p == "public");
    let owner = snippet.as_ref().and_then(|s| {
        s.channel_id.clone().map(|id| User {
            id,
            display_name: s.channel_title.clone(),
        })
    });
    let tracks: Vec<Track> = items
        .into_iter()
        .map(ymusic_playlist_item_to_track)
        .collect();
    Playlist {
        name,
        description,
        public,
        owner,
        tracks,
        source_ids,
        raw: serde_json::to_value(&summary).ok(),
    }
}

fn ymusic_playlist_item_to_track(it: zad::service::ymusic::client::PlaylistItem) -> Track {
    let video_id = it
        .content_details
        .as_ref()
        .and_then(|d| d.video_id.clone())
        .or_else(|| {
            it.snippet
                .as_ref()
                .and_then(|s| s.resource_id.as_ref())
                .and_then(|r| r.video_id.clone())
        });
    let mut source_ids = BTreeMap::new();
    if let Some(id) = &video_id {
        if !id.is_empty() {
            source_ids.insert("ymusic".into(), id.clone());
        }
    }
    let snippet = it.snippet.as_ref();
    let title = snippet.and_then(|s| s.title.clone()).unwrap_or_default();
    let artists: Vec<String> = snippet
        .and_then(|s| s.video_owner_channel_title.clone())
        .map(|t| vec![t])
        .unwrap_or_default();
    Track {
        title,
        artists,
        album: None,
        duration_ms: None,
        isrc: None,
        added_at: None,
        source_ids,
        raw: serde_json::to_value(&it).ok(),
    }
}

// ---------------------------------------------------------------------------
// Helpers used by both export and import.
// ---------------------------------------------------------------------------

impl Track {
    /// Provider-specific source id, if recorded. Used by `spotifai
    /// import` on the same-provider path.
    pub fn source_id_for(&self, service: &str) -> Option<&str> {
        self.source_ids.get(service).map(String::as_str)
    }

    /// Primary artist name, lower-cased and trimmed, or `None` when
    /// the track has no artists. Used as a tie-breaker key in
    /// duplicate detection.
    pub fn primary_artist(&self) -> Option<&str> {
        self.artists
            .iter()
            .find(|a| !a.trim().is_empty())
            .map(String::as_str)
    }

    /// Build a Spotify-style search query from the track's
    /// metadata. Prefers ISRC; falls back to `<title> <primary
    /// artist>`.
    pub fn search_query(&self) -> Option<String> {
        if let Some(isrc) = self.isrc.as_deref() {
            let trimmed = isrc.trim();
            if !trimmed.is_empty() {
                return Some(format!("isrc:{trimmed}"));
            }
        }
        let title = self.title.trim();
        if title.is_empty() {
            return None;
        }
        match self.primary_artist() {
            Some(artist) => Some(format!("{title} {artist}")),
            None => Some(title.to_string()),
        }
    }
}

impl Playlist {
    /// Provider-specific source id, if recorded.
    pub fn source_id_for(&self, service: &str) -> Option<&str> {
        self.source_ids.get(service).map(String::as_str)
    }
}
