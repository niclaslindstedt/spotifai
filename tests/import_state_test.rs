//! Pure-function tests for `spotifai::import_state`. Covers
//! fingerprint stability, on-disk round-trip, atomic write semantics,
//! and the `counts()` helper.

use std::collections::BTreeMap;

use spotifai::export_schema::{Envelope, Playlist, Source, Track};
use spotifai::import_state::{
    self, ImportState, PlaylistState, PlaylistStatus, fingerprint, load, save,
};
use spotifai::providers::Provider;

fn envelope_with(playlists: Vec<(&str, usize)>) -> Envelope {
    let playlists = playlists
        .into_iter()
        .map(|(name, n)| Playlist {
            name: name.into(),
            tracks: (0..n)
                .map(|_| Track {
                    title: "x".into(),
                    ..Track::default()
                })
                .collect(),
            ..Playlist::default()
        })
        .collect();
    let mut env = Envelope::new(
        Source {
            service: "spotify".into(),
            user: None,
            tool: "spotifai".into(),
            tool_version: "0.0.0".into(),
            api_calibration: None,
        },
        "2026-05-14T00:00:00Z".into(),
    );
    env.playlists = playlists;
    env
}

#[test]
fn fingerprint_is_stable_across_calls() {
    let env = envelope_with(vec![("Focus", 3), ("Drive", 5)]);
    let a = fingerprint(&env, Provider::YouTubeMusic);
    let b = fingerprint(&env, Provider::YouTubeMusic);
    assert_eq!(a, b, "fingerprint must be deterministic");
    assert_eq!(a.len(), 16, "should be a 16-char hex");
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn fingerprint_differs_when_target_provider_changes() {
    let env = envelope_with(vec![("Focus", 3)]);
    let s = fingerprint(&env, Provider::Spotify);
    let y = fingerprint(&env, Provider::YouTubeMusic);
    assert_ne!(s, y, "different target provider must hash differently");
}

#[test]
fn fingerprint_differs_when_playlists_differ() {
    let a = fingerprint(&envelope_with(vec![("Focus", 3)]), Provider::Spotify);
    let b = fingerprint(&envelope_with(vec![("Drive", 3)]), Provider::Spotify);
    let c = fingerprint(&envelope_with(vec![("Focus", 4)]), Provider::Spotify);
    assert_ne!(a, b);
    assert_ne!(a, c);
}

#[test]
fn fingerprint_differs_when_exported_at_changes() {
    let mut a = envelope_with(vec![("Focus", 3)]);
    let mut b = envelope_with(vec![("Focus", 3)]);
    a.exported_at = "2026-01-01T00:00:00Z".into();
    b.exported_at = "2026-02-01T00:00:00Z".into();
    assert_ne!(
        fingerprint(&a, Provider::Spotify),
        fingerprint(&b, Provider::Spotify),
    );
}

#[test]
fn save_and_load_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ymusic-deadbeef.json");

    let mut state = ImportState::new("deadbeef".into(), Provider::YouTubeMusic);
    state.upsert(
        "Focus",
        PlaylistState {
            status: PlaylistStatus::Completed,
            target_id: Some("PLxxx".into()),
            resolved_track_ids: vec!["a".into(), "b".into(), "c".into()],
            tracks_processed: 4,
            unresolved_count: 1,
            tracks_added: 3,
            tracks_failed: 0,
        },
    );
    state.upsert("Old Stuff", PlaylistState::new_skipped_duplicate());
    state.upsert("Broken", PlaylistState::new_failed_create());

    save(&state, &path).expect("save");
    let loaded = load(&path).expect("load").expect("present");
    assert_eq!(loaded, state);
}

#[test]
fn load_returns_none_for_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.json");
    assert!(load(&path).expect("ok").is_none());
}

#[test]
fn load_errors_on_garbage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("garbage.json");
    std::fs::write(&path, "not json").unwrap();
    assert!(load(&path).is_err());
}

#[test]
fn save_creates_parent_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("dir").join("state.json");
    let state = ImportState::new("ff".into(), Provider::Spotify);
    save(&state, &path).expect("save");
    assert!(path.exists());
}

#[test]
fn save_does_not_leave_tmp_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.json");
    let state = ImportState::new("ff".into(), Provider::Spotify);
    save(&state, &path).expect("save");
    let entries: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["state.json".to_string()]);
}

#[test]
fn clear_is_idempotent_on_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("never-existed.json");
    import_state::clear(&path).expect("ok");
    import_state::clear(&path).expect("ok");
}

#[test]
fn clear_removes_existing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.json");
    std::fs::write(&path, "{}").unwrap();
    assert!(path.exists());
    import_state::clear(&path).expect("ok");
    assert!(!path.exists());
}

#[test]
fn counts_aggregates_per_status_and_track_totals() {
    let mut state = ImportState::new("ff".into(), Provider::Spotify);
    state.upsert(
        "a",
        PlaylistState {
            status: PlaylistStatus::Completed,
            target_id: Some("1".into()),
            resolved_track_ids: vec!["x".into(); 5],
            tracks_processed: 6,
            unresolved_count: 1,
            tracks_added: 5,
            tracks_failed: 0,
        },
    );
    state.upsert(
        "b",
        PlaylistState {
            status: PlaylistStatus::InProgress,
            target_id: Some("2".into()),
            resolved_track_ids: vec!["x".into(); 10],
            tracks_processed: 5,
            unresolved_count: 0,
            tracks_added: 4,
            tracks_failed: 1,
        },
    );
    state.upsert("c", PlaylistState::new_skipped_duplicate());
    state.upsert("d", PlaylistState::new_failed_create());

    let c = state.counts();
    assert_eq!(c.completed, 1);
    assert_eq!(c.in_progress, 1);
    assert_eq!(c.skipped_duplicate, 1);
    assert_eq!(c.failed_create, 1);
    assert_eq!(c.tracks_added, 9);
    assert_eq!(c.tracks_unresolved, 1);
    assert_eq!(c.tracks_failed, 1);
}

#[test]
fn upsert_is_keyed_by_trimmed_name() {
    let mut state = ImportState::new("ff".into(), Provider::Spotify);
    state.upsert("  Focus  ", PlaylistState::new_skipped_duplicate());
    assert!(state.get("Focus").is_some());
    assert!(state.get(" Focus ").is_some());
}

#[test]
fn playlist_state_is_terminal_matrix() {
    assert!(PlaylistState::new_skipped_duplicate().is_terminal());
    assert!(PlaylistState::new_failed_create().is_terminal());
    let in_progress = PlaylistState {
        status: PlaylistStatus::InProgress,
        ..PlaylistState::new_skipped_duplicate()
    };
    assert!(!in_progress.is_terminal());
    let completed = PlaylistState {
        status: PlaylistStatus::Completed,
        target_id: Some("x".into()),
        resolved_track_ids: vec!["y".into()],
        tracks_added: 1,
        ..PlaylistState::new_skipped_duplicate()
    };
    assert!(completed.is_terminal());
}

#[test]
fn state_path_includes_provider_and_fingerprint() {
    // We just exercise the path-building helper to make sure it
    // produces something sane — the actual home-resolved path is
    // environment-dependent so we only assert the trailing filename.
    let p = import_state::state_path(Provider::YouTubeMusic, "abc123").expect("ok");
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(name, "ymusic-abc123.json");
}

#[test]
fn empty_state_serializes_with_empty_playlists_field() {
    let state = ImportState {
        envelope_fingerprint: "ff".into(),
        provider: "spotify".into(),
        started_at: "2026-05-14T00:00:00Z".into(),
        last_updated_at: "2026-05-14T00:00:00Z".into(),
        playlists: BTreeMap::new(),
    };
    let json = serde_json::to_string_pretty(&state).unwrap();
    assert!(json.contains("\"envelope_fingerprint\""));
    assert!(json.contains("\"playlists\""));
}

#[test]
fn loads_pre_tracks_processed_state_file() {
    // State files written before the `tracks_processed` field existed
    // must still load — `serde(default)` lets the new cursor default
    // to 0 so a resumed run re-processes from the top. With the resolve
    // cache and idempotent inserts, the redo is a fast no-op.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ymusic-old.json");
    let legacy = r#"{
      "envelope_fingerprint": "old",
      "provider": "ymusic",
      "started_at": "2026-05-01T00:00:00Z",
      "last_updated_at": "2026-05-01T00:00:00Z",
      "playlists": {
        "Focus": {
          "status": "in_progress",
          "target_id": "PLxxx",
          "resolved_track_ids": ["a", "b", "c"],
          "unresolved_count": 1,
          "tracks_added": 2,
          "tracks_failed": 0
        }
      }
    }"#;
    std::fs::write(&path, legacy).unwrap();
    let loaded = load(&path).expect("load").expect("present");
    let focus = loaded.get("Focus").expect("focus playlist present");
    assert_eq!(focus.tracks_processed, 0);
    assert_eq!(focus.resolved_track_ids.len(), 3);
    assert_eq!(focus.tracks_added, 2);
}
