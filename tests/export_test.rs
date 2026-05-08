//! Pure-function tests for `spotifai::export` — no zad spawn, no I/O.

use serde_json::{Value, json};

use spotifai::export::{
    PAGE_SIZE, SCHEMA_VERSION, build_envelope, build_zad_args, extract_items, format_iso8601,
    iso8601_now,
};

#[test]
fn build_zad_args_prefixes_spotify_and_appends_json() {
    let args = build_zad_args(&["library", "tracks", "list", "--limit", "50"]);
    assert_eq!(
        args,
        vec![
            "spotify".to_string(),
            "library".to_string(),
            "tracks".to_string(),
            "list".to_string(),
            "--limit".to_string(),
            "50".to_string(),
            "--json".to_string(),
        ]
    );
}

#[test]
fn build_zad_args_handles_empty_verb() {
    let args = build_zad_args(&[]);
    assert_eq!(args, vec!["spotify".to_string(), "--json".to_string()]);
}

#[test]
fn extract_items_returns_bare_arrays_unchanged() {
    let v = json!([{"id": "a"}, {"id": "b"}]);
    let items = extract_items(&v);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "a");
    assert_eq!(items[1]["id"], "b");
}

#[test]
fn extract_items_unwraps_known_pagination_keys() {
    for key in ["items", "tracks", "playlists", "albums", "data", "results"] {
        let v = json!({ key: [{"id": "x"}], "total": 1 });
        let items = extract_items(&v);
        assert_eq!(
            items.len(),
            1,
            "key `{key}` should be recognised as a list wrapper"
        );
        assert_eq!(items[0]["id"], "x");
    }
}

#[test]
fn extract_items_returns_empty_for_unknown_shapes() {
    let v = json!({"unknown_field": [1, 2, 3]});
    assert!(extract_items(&v).is_empty());

    let v = json!(42);
    assert!(extract_items(&v).is_empty());

    let v = json!("string");
    assert!(extract_items(&v).is_empty());
}

#[test]
fn build_envelope_has_required_top_level_keys() {
    let envelope = build_envelope(
        vec![json!({"id": "t1"})],
        vec![json!({"id": "a1"})],
        vec![json!({"id": "p1", "tracks": [json!({"id": "t1"})]})],
    );

    assert_eq!(envelope["schema_version"], SCHEMA_VERSION);
    assert!(envelope["exported_at"].is_string());
    assert_eq!(envelope["source"]["service"], "spotify");
    assert_eq!(envelope["source"]["tool"], "spotifai");
    assert!(envelope["source"]["tool_version"].is_string());

    let liked = envelope["liked_tracks"].as_array().expect("liked_tracks");
    assert_eq!(liked.len(), 1);
    assert_eq!(liked[0]["id"], "t1");

    let albums = envelope["saved_albums"].as_array().expect("saved_albums");
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0]["id"], "a1");

    let playlists = envelope["playlists"].as_array().expect("playlists");
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0]["id"], "p1");
    assert_eq!(playlists[0]["tracks"][0]["id"], "t1");
}

#[test]
fn build_envelope_preserves_arbitrary_zad_fields() {
    // The export wraps zad's --json output verbatim; any field zad
    // adds (isrc, added_at, custom_score) must round-trip without
    // spotifai having to know about it.
    let track = json!({
        "spotify_id": "spotify:track:abc",
        "isrc": "USRC17607839",
        "name": "Billie Jean",
        "artists": ["Michael Jackson"],
        "added_at": "2024-01-15T08:30:00Z",
        "duration_ms": 293826,
        "future_field_zad_might_add": {"nested": true},
    });
    let envelope = build_envelope(vec![track.clone()], vec![], vec![]);
    let liked = envelope["liked_tracks"].as_array().unwrap();
    assert_eq!(liked[0], track);
}

#[test]
fn build_envelope_preserves_playlist_track_ordering() {
    let playlist = json!({
        "id": "p1",
        "name": "Focus",
        "tracks": [
            json!({"id": "t1", "position": 0}),
            json!({"id": "t2", "position": 1}),
            json!({"id": "t3", "position": 2}),
        ],
    });
    let envelope = build_envelope(vec![], vec![], vec![playlist]);
    let tracks = envelope["playlists"][0]["tracks"].as_array().unwrap();
    assert_eq!(tracks[0]["id"], "t1");
    assert_eq!(tracks[1]["id"], "t2");
    assert_eq!(tracks[2]["id"], "t3");
}

#[test]
fn iso8601_now_matches_expected_shape() {
    let s = iso8601_now();
    assert_eq!(s.len(), 20, "expected `YYYY-MM-DDTHH:MM:SSZ`, got `{s}`");
    assert!(s.ends_with('Z'), "must end in Z, got `{s}`");
    assert_eq!(s.chars().nth(4), Some('-'));
    assert_eq!(s.chars().nth(7), Some('-'));
    assert_eq!(s.chars().nth(10), Some('T'));
    assert_eq!(s.chars().nth(13), Some(':'));
    assert_eq!(s.chars().nth(16), Some(':'));
}

#[test]
fn format_iso8601_locks_to_known_timestamps() {
    // Unix epoch.
    assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
    // 2000-01-01T00:00:00Z is 946684800.
    assert_eq!(format_iso8601(946_684_800), "2000-01-01T00:00:00Z");
    // 2024-01-15T08:30:00Z is 1705307400.
    assert_eq!(format_iso8601(1_705_307_400), "2024-01-15T08:30:00Z");
    // 2026-05-08T12:34:56Z is 1778243696.
    assert_eq!(format_iso8601(1_778_243_696), "2026-05-08T12:34:56Z");
    // Leap year boundary: 2024-02-29T12:00:00Z is 1709208000.
    assert_eq!(format_iso8601(1_709_208_000), "2024-02-29T12:00:00Z");
}

#[test]
fn schema_version_is_string_one() {
    assert_eq!(SCHEMA_VERSION, "1");
}

#[test]
fn page_size_matches_spotify_default() {
    assert_eq!(PAGE_SIZE, 50);
}

#[test]
fn envelope_serialises_to_valid_json_string() {
    let envelope = build_envelope(vec![], vec![], vec![]);
    let s = serde_json::to_string(&envelope).expect("serialises");
    let round_trip: Value = serde_json::from_str(&s).expect("parses back");
    assert_eq!(round_trip["schema_version"], SCHEMA_VERSION);
}
