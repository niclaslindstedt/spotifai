//! Pure-function tests for `spotifai::export` and the unified
//! schema mappers in `spotifai::export_schema`.
//!
//! The end-to-end export path needs the zad library to talk to
//! Spotify / YouTube Music HTTP, which is not unit-testable in
//! process. What we cover here is:
//!
//! - The schema mappers: given a synthetic zad response, do we
//!   produce the right [`Envelope`] shape?
//! - The schema's published constants and date-formatter helpers,
//!   which downstream consumers depend on.

use std::collections::BTreeSet;

use spotifai::export::{Selection, format_iso8601, iso8601_now};
use spotifai::export_schema::{
    Envelope, SCHEMA_VERSION, SpotifyExportData, Track, YmusicExportData,
    build_envelope_from_spotify, build_envelope_from_ymusic,
};

#[test]
fn schema_version_is_locked() {
    assert_eq!(SCHEMA_VERSION, "1");
}

#[test]
fn selection_defaults_to_all_when_no_flags_set() {
    let s = Selection::from_flags(false, false, false);
    assert_eq!(s, Selection::ALL);
    assert!(s.likes && s.albums && s.playlists);
}

#[test]
fn selection_honors_individual_flags() {
    let s = Selection::from_flags(true, false, false);
    assert!(s.likes && !s.albums && !s.playlists);
    let s = Selection::from_flags(false, false, true);
    assert!(!s.likes && !s.albums && s.playlists);
    let s = Selection::from_flags(true, false, true);
    assert!(s.likes && !s.albums && s.playlists);
}

#[test]
fn selection_all_constant_matches_default() {
    // Passing every flag explicitly should produce the same value
    // as the `ALL` constant — a regression guard if either side
    // grows new fields.
    assert_eq!(Selection::from_flags(true, true, true), Selection::ALL);
}

#[test]
fn iso8601_now_matches_expected_shape() {
    let s = iso8601_now();
    assert_eq!(s.len(), 20, "expected `YYYY-MM-DDTHH:MM:SSZ`, got `{s}`");
    assert!(s.ends_with('Z'), "must end in Z, got `{s}`");
    assert_eq!(s.chars().nth(4), Some('-'));
    assert_eq!(s.chars().nth(7), Some('-'));
    assert_eq!(s.chars().nth(10), Some('T'));
}

#[test]
fn format_iso8601_locks_to_known_timestamps() {
    assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
    assert_eq!(format_iso8601(946_684_800), "2000-01-01T00:00:00Z");
    assert_eq!(format_iso8601(1_705_307_400), "2024-01-15T08:30:00Z");
    assert_eq!(format_iso8601(1_778_243_696), "2026-05-08T12:34:56Z");
    assert_eq!(format_iso8601(1_709_208_000), "2024-02-29T12:00:00Z");
}

#[test]
fn empty_spotify_envelope_is_well_formed() {
    let env = build_envelope_from_spotify(
        SpotifyExportData::default(),
        "2026-01-01T00:00:00Z".into(),
        "0.1.0",
    );
    assert_eq!(env.schema_version, "1");
    assert_eq!(env.exported_at, "2026-01-01T00:00:00Z");
    assert_eq!(env.source.service, "spotify");
    assert_eq!(env.source.tool, "spotifai");
    assert_eq!(env.source.tool_version, "0.1.0");
    assert!(env.tracks.is_empty());
    assert!(env.albums.is_empty());
    assert!(env.playlists.is_empty());
}

#[test]
fn empty_ymusic_envelope_is_well_formed() {
    let env = build_envelope_from_ymusic(
        YmusicExportData::default(),
        "2026-01-01T00:00:00Z".into(),
        "0.1.0",
    );
    assert_eq!(env.source.service, "ymusic");
    assert!(env.tracks.is_empty());
    // Albums is intentionally empty for ymusic; importers seeing a
    // ymusic-sourced envelope must not expect album data.
    assert!(env.albums.is_empty());
    assert!(env.playlists.is_empty());
}

#[test]
fn spotify_user_lands_in_source_user() {
    let data = SpotifyExportData {
        user_id: Some("alice".into()),
        user_display_name: Some("Alice".into()),
        ..SpotifyExportData::default()
    };
    let env = build_envelope_from_spotify(data, "now".into(), "0.0.0");
    let user = env.source.user.expect("user");
    assert_eq!(user.id, "alice");
    assert_eq!(user.display_name.as_deref(), Some("Alice"));
}

#[test]
fn spotify_saved_track_round_trips_into_unified_track() {
    use zad::service::spotify::client::{ArtistRef, SavedTrack, TrackSummary};

    let st = SavedTrack {
        added_at: Some("2024-05-01T12:00:00Z".into()),
        track: TrackSummary {
            id: "abc".into(),
            name: "Billie Jean".into(),
            uri: Some("spotify:track:abc".into()),
            artists: vec![ArtistRef {
                id: "mj".into(),
                name: "Michael Jackson".into(),
                uri: None,
            }],
            album: None,
            duration_ms: Some(293_826),
            explicit: Some(false),
        },
    };
    let data = SpotifyExportData {
        saved_tracks: vec![st],
        ..SpotifyExportData::default()
    };
    let env = build_envelope_from_spotify(data, "now".into(), "0.0.0");
    let track = &env.tracks[0];
    assert_eq!(track.title, "Billie Jean");
    assert_eq!(track.artists, vec!["Michael Jackson"]);
    assert_eq!(track.duration_ms, Some(293_826));
    assert_eq!(track.added_at.as_deref(), Some("2024-05-01T12:00:00Z"));
    assert_eq!(track.source_id_for("spotify"), Some("abc"));
    // Source ids include only the source provider, not the target.
    assert_eq!(track.source_id_for("ymusic"), None);
}

#[test]
fn ymusic_video_round_trips_into_unified_track() {
    use zad::service::ymusic::client::{VideoSnippet, VideoSummary};

    let v = VideoSummary {
        id: "vid123".into(),
        snippet: Some(VideoSnippet {
            title: "Imagine".into(),
            channel_title: Some("John Lennon".into()),
            channel_id: Some("UCxx".into()),
            description: None,
        }),
        content_details: None,
    };
    let data = YmusicExportData {
        liked_videos: vec![v],
        ..YmusicExportData::default()
    };
    let env = build_envelope_from_ymusic(data, "now".into(), "0.0.0");
    let track = &env.tracks[0];
    assert_eq!(track.title, "Imagine");
    assert_eq!(track.artists, vec!["John Lennon"]);
    assert_eq!(track.source_id_for("ymusic"), Some("vid123"));
}

#[test]
fn track_search_query_prefers_isrc_over_title_artist() {
    let t = Track {
        title: "Billie Jean".into(),
        artists: vec!["Michael Jackson".into()],
        isrc: Some("USRC17607839".into()),
        ..Track::default()
    };
    assert_eq!(t.search_query().as_deref(), Some("isrc:USRC17607839"));
}

#[test]
fn track_search_query_falls_back_to_title_plus_artist() {
    let t = Track {
        title: "Imagine".into(),
        artists: vec!["John Lennon".into()],
        ..Track::default()
    };
    assert_eq!(t.search_query().as_deref(), Some("Imagine John Lennon"));
}

#[test]
fn track_search_query_returns_none_for_empty_track() {
    let t = Track::default();
    assert!(t.search_query().is_none());
}

#[test]
fn envelope_round_trips_through_json() {
    let env = build_envelope_from_spotify(
        SpotifyExportData::default(),
        "2026-01-01T00:00:00Z".into(),
        "0.1.0",
    );
    let body = serde_json::to_string(&env).unwrap();
    let back: Envelope = serde_json::from_str(&body).unwrap();
    assert_eq!(back, env);
}

// `BTreeSet` import keeps the helper module accessible without
// requiring callers to qualify the path.
#[allow(dead_code)]
fn _btreeset_smoke() -> BTreeSet<String> {
    BTreeSet::new()
}
