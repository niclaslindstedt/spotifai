//! Pure-function tests for `spotifai::api` — no network, no zad
//! library calls. The dispatcher itself talks to the keychain and
//! Spotify/YouTube Music HTTP, neither of which is unit-testable;
//! what we lock down here is the user-args parser that drives it.

use serde_json::json;
use spotifai::api::{
    DEFAULT_LIMIT, SEARCH_LIMIT, SPOTIFAI_PROFILE_ENV, SPOTIFAI_PROVIDER_ENV, Verb, parse_verb,
};
use spotifai::api_fields::{self, OutputFormat};
use spotifai::providers::Provider;

fn args(raw: &[&str]) -> Vec<String> {
    raw.iter().map(|s| s.to_string()).collect()
}

#[test]
fn search_picks_up_query_and_defaults_to_track() {
    let v = parse_verb(Provider::Spotify, &args(&["search", "moon river"])).unwrap();
    match v {
        Verb::Search {
            query,
            types,
            limit,
            fields,
            format,
        } => {
            assert_eq!(query, "moon river");
            assert_eq!(types, vec!["track".to_string()]);
            assert_eq!(limit, SEARCH_LIMIT);
            assert!(fields.is_empty());
            assert_eq!(format, OutputFormat::Json);
        }
        other => panic!("expected Search, got {other:?}"),
    }
}

#[test]
fn search_collects_multiple_type_flags() {
    let v = parse_verb(
        Provider::Spotify,
        &args(&[
            "search",
            "kind of blue",
            "--type",
            "album",
            "--type",
            "artist",
        ]),
    )
    .unwrap();
    match v {
        Verb::Search { types, .. } => assert_eq!(types, vec!["album", "artist"]),
        other => panic!("expected Search, got {other:?}"),
    }
}

#[test]
fn search_parses_fields_as_comma_list_and_repeated_flag() {
    let v = parse_verb(
        Provider::Spotify,
        &args(&[
            "search",
            "moon river",
            "--fields",
            "title,artist",
            "--fields=album,id",
        ]),
    )
    .unwrap();
    match v {
        Verb::Search { fields, .. } => {
            assert_eq!(fields, vec!["title", "artist", "album", "id"]);
        }
        other => panic!("expected Search, got {other:?}"),
    }
}

#[test]
fn search_parses_format_text_and_json() {
    let t = parse_verb(
        Provider::Spotify,
        &args(&["search", "x", "--fields", "title", "--format", "text"]),
    )
    .unwrap();
    match t {
        Verb::Search { format, .. } => assert_eq!(format, OutputFormat::Text),
        other => panic!("expected Search, got {other:?}"),
    }

    let j = parse_verb(Provider::Spotify, &args(&["search", "x", "--format=json"])).unwrap();
    match j {
        Verb::Search { format, .. } => assert_eq!(format, OutputFormat::Json),
        other => panic!("expected Search, got {other:?}"),
    }
}

#[test]
fn search_format_text_requires_fields() {
    assert!(
        parse_verb(
            Provider::Spotify,
            &args(&["search", "x", "--format", "text"]),
        )
        .is_err()
    );
}

#[test]
fn search_rejects_unknown_format() {
    assert!(
        parse_verb(
            Provider::Spotify,
            &args(&["search", "x", "--format", "yaml"]),
        )
        .is_err()
    );
}

#[test]
fn project_envelope_keeps_only_requested_fields_with_aliases() {
    let mut value = json!({
        "tracks": {
            "items": [
                {
                    "id": "t1",
                    "name": "Billie Jean",
                    "uri": "spotify:track:t1",
                    "artists": [
                        {"id": "a1", "name": "Michael Jackson"},
                        {"id": "a2", "name": "Quincy Jones"}
                    ],
                    "album": {"id": "al1", "name": "Thriller", "release_date": "1982"},
                    "duration_ms": 294000,
                }
            ]
        }
    });
    let fields = vec![
        "title".to_string(),
        "artist".to_string(),
        "album".to_string(),
        "id".to_string(),
    ];
    api_fields::project_envelope(&mut value, &fields);
    let item = &value["tracks"]["items"][0];
    assert_eq!(item["title"], "Billie Jean");
    assert_eq!(item["artist"], "Michael Jackson, Quincy Jones");
    assert_eq!(item["album"], "Thriller");
    assert_eq!(item["id"], "t1");
    // Untracked keys are dropped.
    assert!(item.get("uri").is_none());
    assert!(item.get("duration_ms").is_none());
}

#[test]
fn project_envelope_handles_ymusic_top_level_items_and_nested_id() {
    let mut value = json!({
        "items": [
            {
                "id": {"kind": "youtube#video", "videoId": "v1"},
                "snippet": {"title": "Hello", "channelTitle": "Adele"}
            }
        ]
    });
    let fields = vec!["title".to_string(), "artist".to_string(), "id".to_string()];
    api_fields::project_envelope(&mut value, &fields);
    let item = &value["items"][0];
    assert_eq!(item["title"], "Hello");
    assert_eq!(item["artist"], "Adele");
    assert_eq!(item["id"], "v1");
}

#[test]
fn project_envelope_no_fields_is_a_noop() {
    let original = json!({"tracks": {"items": [{"id": "t1", "name": "X"}]}});
    let mut value = original.clone();
    api_fields::project_envelope(&mut value, &[]);
    assert_eq!(value, original);
}

#[test]
fn playlists_list_takes_limit() {
    let v = parse_verb(
        Provider::Spotify,
        &args(&["playlists", "list", "--limit", "20"]),
    )
    .unwrap();
    assert_eq!(v, Verb::PlaylistsList { limit: 20 });
}

#[test]
fn playlists_show_takes_id() {
    let v = parse_verb(Provider::Spotify, &args(&["playlists", "show", "abc123"])).unwrap();
    match v {
        Verb::PlaylistsShow { id, limit } => {
            assert_eq!(id, "abc123");
            assert_eq!(limit, DEFAULT_LIMIT);
        }
        other => panic!("expected PlaylistsShow, got {other:?}"),
    }
}

#[test]
fn playlists_create_uses_name_for_spotify_and_title_for_ymusic() {
    let s = parse_verb(
        Provider::Spotify,
        &args(&["playlists", "create", "--name", "Focus"]),
    )
    .unwrap();
    match s {
        Verb::PlaylistsCreate { name, public, .. } => {
            assert_eq!(name, "Focus");
            assert!(!public);
        }
        other => panic!("expected PlaylistsCreate, got {other:?}"),
    }

    let y = parse_verb(
        Provider::YouTubeMusic,
        &args(&["playlists", "create", "--title", "Focus"]),
    )
    .unwrap();
    match y {
        Verb::PlaylistsCreate { name, .. } => assert_eq!(name, "Focus"),
        other => panic!("expected PlaylistsCreate, got {other:?}"),
    }

    // Mismatched flag: Spotify expects --name, ymusic expects
    // --title. Using the wrong one should error out cleanly.
    assert!(
        parse_verb(
            Provider::Spotify,
            &args(&["playlists", "create", "--title", "X"])
        )
        .is_err()
    );
    assert!(
        parse_verb(
            Provider::YouTubeMusic,
            &args(&["playlists", "create", "--name", "X"])
        )
        .is_err()
    );
}

#[test]
fn playlists_add_collects_ids_after_playlist_id() {
    let v = parse_verb(
        Provider::Spotify,
        &args(&["playlists", "add", "PLST", "T1", "T2", "T3"]),
    )
    .unwrap();
    match v {
        Verb::PlaylistsAdd { playlist_id, ids } => {
            assert_eq!(playlist_id, "PLST");
            assert_eq!(ids, vec!["T1", "T2", "T3"]);
        }
        other => panic!("expected PlaylistsAdd, got {other:?}"),
    }
}

#[test]
fn library_routes_per_provider() {
    let s = parse_verb(
        Provider::Spotify,
        &args(&["library", "tracks", "list", "--limit", "10"]),
    )
    .unwrap();
    assert_eq!(s, Verb::SpotifyLibraryTracksList { limit: 10 });

    let s2 = parse_verb(Provider::Spotify, &args(&["library", "albums", "list"])).unwrap();
    assert_eq!(
        s2,
        Verb::SpotifyLibraryAlbumsList {
            limit: DEFAULT_LIMIT
        }
    );

    let y = parse_verb(
        Provider::YouTubeMusic,
        &args(&["library", "list", "--limit", "5"]),
    )
    .unwrap();
    assert_eq!(y, Verb::YmusicLibraryList { limit: 5 });
}

#[test]
fn library_rejects_wrong_shape_for_provider() {
    // Spotify has no bare `library list`; ymusic has no
    // `library tracks list`.
    assert!(parse_verb(Provider::Spotify, &args(&["library", "list"])).is_err());
    assert!(
        parse_verb(
            Provider::YouTubeMusic,
            &args(&["library", "tracks", "list"])
        )
        .is_err()
    );
}

#[test]
fn limit_is_validated_and_capped() {
    assert!(
        parse_verb(
            Provider::Spotify,
            &args(&["playlists", "list", "--limit", "0"])
        )
        .is_err()
    );
    assert!(
        parse_verb(
            Provider::Spotify,
            &args(&["playlists", "list", "--limit", "51"])
        )
        .is_err()
    );
    assert!(
        parse_verb(
            Provider::Spotify,
            &args(&["playlists", "list", "--limit", "abc"])
        )
        .is_err()
    );
}

#[test]
fn json_and_pretty_flags_are_accepted_as_no_ops() {
    let v = parse_verb(Provider::Spotify, &args(&["playlists", "list", "--json"])).unwrap();
    assert_eq!(
        v,
        Verb::PlaylistsList {
            limit: DEFAULT_LIMIT
        }
    );
}

#[test]
fn missing_verb_errors_out() {
    assert!(parse_verb(Provider::Spotify, &args(&[])).is_err());
}

#[test]
fn unknown_verb_errors_out() {
    assert!(parse_verb(Provider::Spotify, &args(&["bogus"])).is_err());
}

#[test]
fn env_constants_keep_their_names() {
    // External tooling depends on these names — `spotifai ask` /
    // `spotifai playlist` set them before spawning the agent.
    assert_eq!(SPOTIFAI_PROFILE_ENV, "SPOTIFAI_PROFILE");
    assert_eq!(SPOTIFAI_PROVIDER_ENV, "SPOTIFAI_PROVIDER");
}
