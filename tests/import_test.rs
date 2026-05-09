//! Pure-function tests for `spotifai::import`. The end-to-end
//! import path needs the zad library; what we cover here is the
//! schema parser, validator, summary line, and pure helpers.

use std::collections::BTreeMap;

use spotifai::export_schema::{Envelope, SCHEMA_VERSION, Source, Track};
use spotifai::import::{
    ImportReport, import_summary_line, is_duplicate_name, parse_envelope, track_label,
    validate_schema_version,
};

fn empty_envelope(service: &str) -> Envelope {
    Envelope::new(
        Source {
            service: service.into(),
            user: None,
            tool: "spotifai".into(),
            tool_version: "0.0.0".into(),
        },
        "2026-01-01T00:00:00Z".into(),
    )
}

#[test]
fn schema_version_is_locked() {
    assert_eq!(SCHEMA_VERSION, "1");
}

#[test]
fn parse_envelope_round_trips_minimal_document() {
    let raw = r#"{
      "schema_version": "1",
      "exported_at": "2026-01-01T00:00:00Z",
      "source": { "service": "spotify", "tool": "spotifai", "tool_version": "0.1.0" },
      "tracks": [],
      "albums": [],
      "playlists": []
    }"#;
    let env = parse_envelope(raw).expect("parses");
    assert_eq!(env.schema_version, "1");
    assert_eq!(env.source.service, "spotify");
}

#[test]
fn parse_envelope_rejects_invalid_json() {
    assert!(parse_envelope("not json").is_err());
    assert!(parse_envelope("{").is_err());
}

#[test]
fn parse_envelope_rejects_missing_required_fields() {
    // Missing schema_version, source, etc.
    assert!(parse_envelope("{}").is_err());
}

#[test]
fn validate_schema_version_accepts_known() {
    let env = empty_envelope("spotify");
    validate_schema_version(&env).expect("ok");
}

#[test]
fn validate_schema_version_rejects_unknown() {
    let mut env = empty_envelope("spotify");
    env.schema_version = "999".into();
    let err = validate_schema_version(&env).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("`999`"), "should name the bad version: {msg}");
    assert!(msg.contains("`1`"), "should name supported: {msg}");
}

#[test]
fn is_duplicate_name_is_case_insensitive_and_trimmed() {
    let existing = vec!["Focus".into(), "  party mix ".into()];
    assert!(is_duplicate_name("focus", &existing));
    assert!(is_duplicate_name("FOCUS ", &existing));
    assert!(is_duplicate_name("Party Mix", &existing));
    assert!(!is_duplicate_name("Chill", &existing));
}

#[test]
fn import_summary_line_locks_to_known_counts() {
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
fn track_label_uses_title_em_dash_artist() {
    let t = Track {
        title: "Billie Jean".into(),
        artists: vec!["Michael Jackson".into()],
        ..Track::default()
    };
    assert_eq!(track_label(&t), "Billie Jean — Michael Jackson");
}

#[test]
fn track_label_falls_back_to_unknown_when_title_missing() {
    let t = Track {
        artists: vec!["Anon".into()],
        ..Track::default()
    };
    assert_eq!(track_label(&t), "<unknown> — Anon");
}

#[test]
fn track_source_id_lookup_is_per_service() {
    let t = Track {
        source_ids: BTreeMap::from([
            ("spotify".to_string(), "abc".to_string()),
            ("ymusic".to_string(), "def".to_string()),
        ]),
        ..Track::default()
    };
    assert_eq!(t.source_id_for("spotify"), Some("abc"));
    assert_eq!(t.source_id_for("ymusic"), Some("def"));
    assert_eq!(t.source_id_for("apple-music"), None);
}

#[test]
fn track_search_query_uses_isrc_first() {
    let t = Track {
        title: "Billie Jean".into(),
        artists: vec!["Michael Jackson".into()],
        isrc: Some("USRC17607839".into()),
        ..Track::default()
    };
    assert_eq!(t.search_query().as_deref(), Some("isrc:USRC17607839"));
}

#[test]
fn track_search_query_returns_text_when_no_isrc() {
    let t = Track {
        title: "Imagine".into(),
        artists: vec!["John Lennon".into()],
        ..Track::default()
    };
    assert_eq!(t.search_query().as_deref(), Some("Imagine John Lennon"));
}
