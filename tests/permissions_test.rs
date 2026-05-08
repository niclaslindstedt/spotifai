//! Pure-function tests for `spotifai::permissions` and `spotifai::providers`.

use spotifai::permissions::{
    self, MODE_PLAYLIST_CURATOR, MODE_READ_ONLY, Permissions, Profile, ensure_default_at,
    from_toml_string, path_for, read_or, to_toml_string,
};
use spotifai::providers::{Provider, spotify_default, ymusic_default};
use tempfile::tempdir;

#[test]
fn spotify_ask_default_locks_down_every_write_verb() {
    let p = spotify_default(Profile::Ask);
    assert_eq!(p.mode, MODE_READ_ONLY);

    // Read-side coverage (these power the kinds of questions
    // `spotifai ask` is meant to answer).
    for verb in [
        "search",
        "playlists list",
        "playlists show",
        "library tracks list",
        "library albums list",
    ] {
        assert!(
            p.allowed.iter().any(|a| a == verb),
            "spotify×ask should allow `{verb}`, got {:?}",
            p.allowed
        );
    }

    // Every mutating zad spotify verb must be on the deny list.
    for verb in [
        "playlists create",
        "playlists rename",
        "playlists delete",
        "playlists add",
        "playlists remove",
        "library tracks save",
        "library tracks unsave",
        "library albums save",
        "library albums unsave",
    ] {
        assert!(
            p.denied.iter().any(|d| d == verb),
            "spotify×ask should deny `{verb}`, got {:?}",
            p.denied
        );
    }

    // Defense-in-depth: nothing on `denied` should also be on `allowed`.
    for v in &p.denied {
        assert!(
            !p.allowed.contains(v),
            "verb `{v}` is on both allow and deny lists"
        );
    }
}

#[test]
fn spotify_playlist_default_allows_create_add_rename_only() {
    let p = spotify_default(Profile::Playlist);
    assert_eq!(p.mode, MODE_PLAYLIST_CURATOR);

    // Read verbs are inherited so the agent can search and look at
    // the user's existing library before adding tracks.
    for verb in [
        "search",
        "playlists list",
        "playlists show",
        "library tracks list",
        "library albums list",
    ] {
        assert!(
            p.allowed.iter().any(|a| a == verb),
            "spotify×playlist should allow read verb `{verb}`, got {:?}",
            p.allowed
        );
    }

    // Write verbs needed to build a playlist.
    for verb in ["playlists create", "playlists add", "playlists rename"] {
        assert!(
            p.allowed.iter().any(|a| a == verb),
            "spotify×playlist should allow `{verb}`, got {:?}",
            p.allowed
        );
    }

    // Destructive and library-mutating verbs stay denied.
    for verb in [
        "playlists delete",
        "playlists remove",
        "library tracks save",
        "library tracks unsave",
        "library albums save",
        "library albums unsave",
    ] {
        assert!(
            p.denied.iter().any(|d| d == verb),
            "spotify×playlist should deny `{verb}`, got {:?}",
            p.denied
        );
    }

    for v in &p.denied {
        assert!(
            !p.allowed.contains(v),
            "verb `{v}` is on both allow and deny lists in spotify×playlist"
        );
    }
}

#[test]
fn ymusic_ask_default_is_read_only_over_ymusic_verbs() {
    let p = ymusic_default(Profile::Ask);
    assert_eq!(p.mode, MODE_READ_ONLY);

    for verb in ["search", "playlists list", "playlists show", "library list"] {
        assert!(
            p.allowed.iter().any(|a| a == verb),
            "ymusic×ask should allow `{verb}`, got {:?}",
            p.allowed
        );
    }

    for verb in [
        "playlists create",
        "playlists rename",
        "playlists delete",
        "playlists add",
        "playlists remove",
        "library like",
        "library unlike",
    ] {
        assert!(
            p.denied.iter().any(|d| d == verb),
            "ymusic×ask should deny `{verb}`, got {:?}",
            p.denied
        );
    }

    // YouTube Music has no `library albums` concept, so make sure we
    // do not accidentally surface a Spotify-shaped verb on the ymusic
    // policy.
    for verb in ["library tracks list", "library albums list"] {
        assert!(
            !p.allowed.iter().any(|a| a == verb),
            "ymusic policy must not list Spotify-shaped verb `{verb}`: {:?}",
            p.allowed
        );
    }
}

#[test]
fn ymusic_playlist_default_allows_create_add_rename_only() {
    let p = ymusic_default(Profile::Playlist);
    assert_eq!(p.mode, MODE_PLAYLIST_CURATOR);

    for verb in [
        "search",
        "playlists list",
        "playlists show",
        "playlists create",
        "playlists add",
        "playlists rename",
        "library list",
    ] {
        assert!(
            p.allowed.iter().any(|a| a == verb),
            "ymusic×playlist should allow `{verb}`, got {:?}",
            p.allowed
        );
    }

    for verb in [
        "playlists delete",
        "playlists remove",
        "library like",
        "library unlike",
    ] {
        assert!(
            p.denied.iter().any(|d| d == verb),
            "ymusic×playlist should deny `{verb}`, got {:?}",
            p.denied
        );
    }
}

#[test]
fn provider_round_trips_through_string() {
    for &provider in Provider::ALL {
        let s = provider.as_str();
        assert_eq!(Provider::parse(s), Some(provider));
    }
    // Aliases that humans are likely to type.
    assert_eq!(
        Provider::parse("youtube-music"),
        Some(Provider::YouTubeMusic)
    );
    assert_eq!(
        Provider::parse("youtube_music"),
        Some(Provider::YouTubeMusic)
    );
    assert_eq!(Provider::parse("ytmusic"), Some(Provider::YouTubeMusic));

    // Unknown / case-mismatch fall through.
    assert_eq!(Provider::parse(""), None);
    assert_eq!(Provider::parse("YMUSIC"), None);
    assert_eq!(Provider::parse("nonsense"), None);
}

#[test]
fn provider_default_is_spotify() {
    assert_eq!(Provider::DEFAULT, Provider::Spotify);
}

#[test]
fn profile_round_trips_through_string() {
    for &profile in Profile::ALL {
        let s = profile.as_str();
        assert_eq!(Profile::parse(s), Some(profile));
    }
    assert_eq!(Profile::parse(""), None);
    assert_eq!(Profile::parse("ASK"), None);
    assert_eq!(Profile::parse("nonsense"), None);
}

#[test]
fn profile_default_policy_dispatches_per_provider() {
    assert_eq!(
        Profile::Ask.default_policy(Provider::Spotify),
        spotify_default(Profile::Ask)
    );
    assert_eq!(
        Profile::Playlist.default_policy(Provider::Spotify),
        spotify_default(Profile::Playlist)
    );
    assert_eq!(
        Profile::Ask.default_policy(Provider::YouTubeMusic),
        ymusic_default(Profile::Ask)
    );
    assert_eq!(
        Profile::Playlist.default_policy(Provider::YouTubeMusic),
        ymusic_default(Profile::Playlist)
    );
}

#[test]
fn prompt_block_lists_each_verb_with_spotifai_api_prefix() {
    let p = spotify_default(Profile::Ask);
    let block = p.as_prompt_block();
    assert!(
        block.contains("Mode: read_only"),
        "prompt block missing mode: {block}"
    );
    assert!(
        block.contains("- `spotifai api search`"),
        "prompt block missing `search`: {block}"
    );
    assert!(
        block.contains("- `spotifai api playlists create`"),
        "prompt block missing denied `playlists create`: {block}"
    );
    // Sanity: every verb appears prefixed, exactly once.
    for v in p.allowed.iter().chain(p.denied.iter()) {
        let needle = format!("- `spotifai api {v}`");
        assert_eq!(
            block.matches(&needle).count(),
            1,
            "expected exactly one occurrence of `{needle}` in prompt block:\n{block}"
        );
    }
}

#[test]
fn empty_lists_render_as_none_placeholder() {
    let empty = Permissions {
        mode: "custom".into(),
        description: "Nothing allowed.".into(),
        allowed: vec![],
        denied: vec![],
    };
    let block = empty.as_prompt_block();
    // Two `- (none)` lines: one under Allowed, one under Denied.
    assert_eq!(block.matches("- (none)").count(), 2, "block:\n{block}");
}

#[test]
fn toml_roundtrips_through_to_and_from_string() {
    for original in [
        spotify_default(Profile::Ask),
        spotify_default(Profile::Playlist),
        ymusic_default(Profile::Ask),
        ymusic_default(Profile::Playlist),
    ] {
        let serialized = to_toml_string(&original).unwrap();
        assert!(
            serialized.starts_with("# spotifai permission profile"),
            "serialized output missing header comment:\n{serialized}"
        );
        let parsed = from_toml_string(&serialized).unwrap();
        assert_eq!(parsed, original);
    }
}

#[test]
fn ensure_default_at_writes_file_only_once() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested").join("permissions.toml");

    // First call writes the default policy and creates parents.
    let wrote = ensure_default_at(&path, &spotify_default(Profile::Ask)).unwrap();
    assert!(wrote, "first ensure_default_at should report a write");
    assert!(path.exists());

    // Hand-edit the file: change the mode tag. ensure_default_at must
    // not stomp on the user's edit on the next call.
    let edited = std::fs::read_to_string(&path)
        .unwrap()
        .replace("mode = \"read_only\"", "mode = \"read_write\"");
    std::fs::write(&path, &edited).unwrap();

    let wrote_again = ensure_default_at(&path, &spotify_default(Profile::Ask)).unwrap();
    assert!(!wrote_again, "second ensure_default_at should be a no-op");

    let after = std::fs::read_to_string(&path).unwrap();
    assert!(
        after.contains("mode = \"read_write\""),
        "ensure_default_at overwrote a user edit:\n{after}"
    );
}

#[test]
fn read_or_returns_fallback_for_missing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does-not-exist.toml");
    assert!(!path.exists());
    let p = read_or(&path, spotify_default(Profile::Ask)).unwrap();
    assert_eq!(p, spotify_default(Profile::Ask));
}

#[test]
fn read_or_parses_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("permissions.toml");
    let mut custom = spotify_default(Profile::Ask);
    custom.mode = "custom".into();
    custom.description = "test".into();
    custom.allowed = vec!["search".into()];
    custom.denied = vec![];
    std::fs::write(&path, to_toml_string(&custom).unwrap()).unwrap();

    let read = read_or(&path, spotify_default(Profile::Ask)).unwrap();
    assert_eq!(read, custom);
}

#[test]
fn path_for_lives_under_dot_spotifai_provider_permissions_dir() {
    use std::path::Path;

    for &provider in Provider::ALL {
        for &profile in Profile::ALL {
            let path = path_for(provider, profile).unwrap();
            let expected_tail = Path::new(".spotifai")
                .join(permissions::PERMISSIONS_DIR)
                .join(provider.as_str())
                .join(format!("{}.toml", profile.as_str()));
            assert!(
                path.ends_with(&expected_tail),
                "path for {provider:?}×{profile:?} = {} did not end with {}",
                path.display(),
                expected_tail.display(),
            );
        }
    }
}

#[test]
fn path_for_returns_distinct_files_per_provider_profile_pair() {
    let mut paths = std::collections::HashSet::new();
    for &provider in Provider::ALL {
        for &profile in Profile::ALL {
            let path = path_for(provider, profile).unwrap();
            assert!(
                paths.insert(path.clone()),
                "duplicate path between {provider:?}×{profile:?} entries: {}",
                path.display()
            );
        }
    }
}
