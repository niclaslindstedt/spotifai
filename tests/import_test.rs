//! Pure-function tests for `spotifai::import` — no zad spawn, no I/O.

use std::cell::RefCell;

use anyhow::{Result, anyhow};
use serde_json::json;

use spotifai::import::{
    ImportReport, SUPPORTED_SCHEMA_VERSIONS, build_add_args, build_create_args,
    extract_created_playlist_id, import_summary_line, is_duplicate_name, parse_envelope,
    playlist_display_name, playlists_from_envelope, resolve_track, search_query_isrc,
    search_query_text, source_service, target_track_id, tracks_in_playlist,
    validate_schema_version,
};
use spotifai::providers::Provider;

#[test]
fn parse_envelope_round_trips_minimal_document() {
    let raw = r#"{"schema_version":"1","source":{"service":"spotify"},"playlists":[]}"#;
    let v = parse_envelope(raw).expect("parses");
    assert_eq!(v["schema_version"], "1");
}

#[test]
fn parse_envelope_rejects_invalid_json() {
    assert!(parse_envelope("not json").is_err());
    assert!(parse_envelope("{").is_err());
}

#[test]
fn validate_schema_version_accepts_known() {
    let v = json!({"schema_version": "1"});
    validate_schema_version(&v).expect("ok");
}

#[test]
fn validate_schema_version_rejects_unknown() {
    let v = json!({"schema_version": "2"});
    let err = validate_schema_version(&v).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("`2`"), "should name the bad version: {msg}");
    assert!(
        msg.contains("\"1\""),
        "should name supported versions: {msg}"
    );
}

#[test]
fn validate_schema_version_rejects_missing() {
    let v = json!({});
    let err = validate_schema_version(&v).unwrap_err();
    assert!(format!("{err:#}").contains("schema_version"));
}

#[test]
fn source_service_extracts_string() {
    let v = json!({"source": {"service": "spotify"}});
    assert_eq!(source_service(&v).unwrap(), "spotify");
}

#[test]
fn source_service_errors_on_missing() {
    assert!(source_service(&json!({})).is_err());
    assert!(source_service(&json!({"source": {}})).is_err());
}

#[test]
fn source_service_errors_on_non_string() {
    let v = json!({"source": {"service": 42}});
    assert!(source_service(&v).is_err());
}

#[test]
fn playlists_from_envelope_returns_each_entry() {
    let v = json!({"playlists": [{"name": "a"}, {"name": "b"}]});
    let pl = playlists_from_envelope(&v).unwrap();
    assert_eq!(pl.len(), 2);
}

#[test]
fn playlists_from_envelope_errors_on_missing_or_wrong_type() {
    assert!(playlists_from_envelope(&json!({})).is_err());
    assert!(playlists_from_envelope(&json!({"playlists": "nope"})).is_err());
}

#[test]
fn tracks_in_playlist_uses_tracks_for_spotify() {
    let pl = json!({"tracks": [{"id": "t1"}, {"id": "t2"}]});
    let t = tracks_in_playlist(&pl, "spotify");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0]["id"], "t1");
}

#[test]
fn tracks_in_playlist_uses_videos_for_ymusic() {
    let pl = json!({"videos": [{"video_id": "v1"}]});
    let t = tracks_in_playlist(&pl, "ymusic");
    assert_eq!(t.len(), 1);
    assert_eq!(t[0]["video_id"], "v1");
}

#[test]
fn tracks_in_playlist_falls_back_to_other_key() {
    // A spotify-tagged source that happens to carry videos still
    // produces something rather than silently empty.
    let pl = json!({"videos": [{"id": "x"}]});
    let t = tracks_in_playlist(&pl, "spotify");
    assert_eq!(t.len(), 1);
}

#[test]
fn tracks_in_playlist_empty_for_neither_key() {
    let pl = json!({"unrelated": [1, 2]});
    assert!(tracks_in_playlist(&pl, "spotify").is_empty());
}

#[test]
fn is_duplicate_name_is_case_insensitive_and_trimmed() {
    let existing = vec!["Focus".to_string(), "  party mix ".to_string()];
    assert!(is_duplicate_name("focus", &existing));
    assert!(is_duplicate_name("FOCUS ", &existing));
    assert!(is_duplicate_name("Party Mix", &existing));
    assert!(!is_duplicate_name("Chill", &existing));
}

#[test]
fn target_track_id_priority_on_spotify() {
    let hit = json!({
        "id": "abc",
        "uri": "spotify:track:xyz",
        "spotify_id": "spotify:track:zzz",
    });
    assert_eq!(
        target_track_id(&hit, Provider::Spotify).unwrap(),
        "spotify:track:zzz"
    );

    let hit = json!({"id": "abc", "uri": "spotify:track:xyz"});
    assert_eq!(
        target_track_id(&hit, Provider::Spotify).unwrap(),
        "spotify:track:xyz"
    );

    let hit = json!({"id": "abc"});
    assert_eq!(target_track_id(&hit, Provider::Spotify).unwrap(), "abc");

    assert!(target_track_id(&json!({}), Provider::Spotify).is_none());
}

#[test]
fn target_track_id_priority_on_ymusic() {
    let hit = json!({"id": "x", "videoId": "vY", "video_id": "vS"});
    assert_eq!(target_track_id(&hit, Provider::YouTubeMusic).unwrap(), "vS");

    let hit = json!({"id": "x", "videoId": "vY"});
    assert_eq!(target_track_id(&hit, Provider::YouTubeMusic).unwrap(), "vY");

    let hit = json!({"id": "x"});
    assert_eq!(target_track_id(&hit, Provider::YouTubeMusic).unwrap(), "x");
}

#[test]
fn search_query_isrc_returns_none_for_missing_or_empty() {
    assert!(search_query_isrc(&json!({})).is_none());
    assert!(search_query_isrc(&json!({"isrc": ""})).is_none());
    assert!(search_query_isrc(&json!({"isrc": "   "})).is_none());
    assert_eq!(
        search_query_isrc(&json!({"isrc": "USRC17607839"})).unwrap(),
        "isrc:USRC17607839"
    );
}

#[test]
fn search_query_text_handles_artists_object_array() {
    let track = json!({
        "name": "Billie Jean",
        "artists": [{"name": "Michael Jackson"}, {"name": "Other"}],
    });
    assert_eq!(
        search_query_text(&track).unwrap(),
        "Billie Jean Michael Jackson"
    );
}

#[test]
fn search_query_text_handles_artists_string_array() {
    let track = json!({"name": "Imagine", "artists": ["John Lennon"]});
    assert_eq!(search_query_text(&track).unwrap(), "Imagine John Lennon");
}

#[test]
fn search_query_text_handles_singular_artist_field() {
    let track = json!({"title": "Clair de Lune", "artist": "Debussy"});
    assert_eq!(search_query_text(&track).unwrap(), "Clair de Lune Debussy");
}

#[test]
fn search_query_text_returns_none_when_artist_missing() {
    let track = json!({"name": "Untitled"});
    assert!(search_query_text(&track).is_none());
}

#[test]
fn resolve_track_uses_isrc_first_then_text_fallback() {
    let track = json!({
        "isrc": "USRC17607839",
        "name": "Billie Jean",
        "artists": ["Michael Jackson"],
    });
    let calls: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let id = resolve_track(&track, Provider::Spotify, |q, ty| {
        calls.borrow_mut().push(q.to_string());
        assert_eq!(ty, Some("track"));
        // ISRC query → empty; text query → one hit.
        if q.starts_with("isrc:") {
            Ok(vec![])
        } else {
            Ok(vec![json!({"spotify_id": "spotify:track:abc"})])
        }
    })
    .unwrap();
    assert_eq!(id.unwrap(), "spotify:track:abc");
    let calls = calls.borrow();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], "isrc:USRC17607839");
    assert_eq!(calls[1], "Billie Jean Michael Jackson");
}

#[test]
fn resolve_track_returns_first_isrc_hit_without_text_fallback() {
    let track = json!({
        "isrc": "USRC17607839",
        "name": "Billie Jean",
        "artists": ["Michael Jackson"],
    });
    let calls: RefCell<usize> = RefCell::new(0);
    let id = resolve_track(&track, Provider::Spotify, |_q, _ty| {
        *calls.borrow_mut() += 1;
        Ok(vec![json!({"id": "abc"})])
    })
    .unwrap();
    assert_eq!(id.unwrap(), "abc");
    assert_eq!(*calls.borrow(), 1, "must short-circuit on first hit");
}

#[test]
fn resolve_track_returns_none_when_both_queries_empty() {
    let track = json!({"isrc": "USRC17607839", "name": "Untitled", "artists": []});
    // No `isrc:` hit AND no usable text query (artist missing).
    let id = resolve_track(&track, Provider::Spotify, |_q, _ty| Ok(vec![])).unwrap();
    assert!(id.is_none());
}

#[test]
fn resolve_track_propagates_search_error() {
    let track = json!({"isrc": "USRC17607839"});
    let result: Result<_> = resolve_track(&track, Provider::Spotify, |_q, _ty| {
        Err(anyhow!("network down"))
    });
    let err = result.unwrap_err();
    assert!(format!("{err:#}").contains("network"));
}

#[test]
fn resolve_track_returns_none_when_track_has_no_identifying_fields() {
    let track = json!({});
    let id = resolve_track(&track, Provider::Spotify, |_q, _ty| {
        panic!("search should not be called when there is nothing to query")
    })
    .unwrap();
    assert!(id.is_none());
}

#[test]
fn build_create_args_uses_name_flag_on_spotify() {
    let args = build_create_args(Provider::Spotify, "Focus");
    assert_eq!(
        args,
        vec!["playlists", "create", "--name", "Focus"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn build_create_args_uses_title_flag_on_ymusic() {
    let args = build_create_args(Provider::YouTubeMusic, "Focus");
    assert_eq!(
        args,
        vec!["playlists", "create", "--title", "Focus"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn build_add_args_emits_playlist_id_then_track_ids() {
    let ids = vec!["t1".to_string(), "t2".to_string(), "t3".to_string()];
    let args = build_add_args(Provider::Spotify, "p1", &ids);
    assert_eq!(
        args,
        vec!["playlists", "add", "p1", "t1", "t2", "t3"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn extract_created_playlist_id_handles_each_priority_key() {
    assert_eq!(
        extract_created_playlist_id(&json!({"id": "a", "playlist_id": "b"})).unwrap(),
        "a"
    );
    assert_eq!(
        extract_created_playlist_id(&json!({"playlist_id": "b"})).unwrap(),
        "b"
    );
    assert_eq!(
        extract_created_playlist_id(&json!({"spotify_id": "spotify:playlist:x"})).unwrap(),
        "spotify:playlist:x"
    );
    assert_eq!(
        extract_created_playlist_id(&json!({"uri": "spotify:playlist:y"})).unwrap(),
        "spotify:playlist:y"
    );
    assert!(extract_created_playlist_id(&json!({})).is_none());
}

#[test]
fn playlist_display_name_falls_back_through_keys() {
    assert_eq!(
        playlist_display_name(&json!({"name": "Focus", "title": "ignored"})).unwrap(),
        "Focus"
    );
    assert_eq!(
        playlist_display_name(&json!({"title": "Drive"})).unwrap(),
        "Drive"
    );
    assert!(playlist_display_name(&json!({})).is_none());
    assert!(playlist_display_name(&json!({"name": "  "})).is_none());
}

#[test]
fn import_summary_line_format_locks_to_known_counts() {
    let r = ImportReport {
        playlists_created: 3,
        playlists_skipped_duplicate: 1,
        playlists_failed: 0,
        tracks_added: 42,
        tracks_unresolved: 2,
        tracks_failed: 0,
    };
    let line = import_summary_line(&r);
    assert!(line.contains("3 playlists"));
    assert!(line.contains("42 tracks added"));
    assert!(line.contains("1 skipped duplicate"));
    assert!(line.contains("2 unresolved tracks"));
}

#[test]
fn supported_schema_versions_includes_one() {
    assert!(SUPPORTED_SCHEMA_VERSIONS.contains(&"1"));
}

#[test]
fn import_report_default_is_all_zero() {
    let r = ImportReport::default();
    assert_eq!(r.playlists_created, 0);
    assert_eq!(r.tracks_added, 0);
}

// Helper: assert that the same-provider path on a Spotify export
// uses the embedded `spotify_id` directly without ever going through
// search.
#[test]
fn same_provider_uses_embedded_id_directly() {
    let track = json!({
        "spotify_id": "spotify:track:abc",
        "name": "Billie Jean",
        "artists": ["Michael Jackson"],
    });
    let id = target_track_id(&track, Provider::Spotify);
    assert_eq!(id.unwrap(), "spotify:track:abc");
}

// Helper: confirm the `Value` we'd construct from an existing-list
// fetch goes through `extract_items` cleanly (we don't re-export
// extract_items here, but the shape is known to be either bare-array
// or `{playlists: [...]}`).
#[test]
fn playlist_display_name_works_on_listed_summary() {
    let summary = json!({"id": "p1", "name": "Focus"});
    assert_eq!(playlist_display_name(&summary).unwrap(), "Focus");
}
