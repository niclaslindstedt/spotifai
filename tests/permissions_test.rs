//! Pure-function tests for `spotifai::permissions`.

use spotifai::permissions::{
    self, Permissions, Profile, ask_default, ensure_default_at, from_toml_string, path_for,
    playlist_default, read_or, to_toml_string,
};
use tempfile::tempdir;

#[test]
fn ask_default_locks_down_every_write_verb() {
    let p = ask_default();
    assert_eq!(p.mode, "read_only");

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
            "ask profile should allow `{verb}`, got {:?}",
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
            "ask profile should deny `{verb}`, got {:?}",
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
fn playlist_default_allows_create_add_rename_only() {
    let p = playlist_default();
    assert_eq!(p.mode, "playlist_curator");

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
            "playlist profile should allow read verb `{verb}`, got {:?}",
            p.allowed
        );
    }

    // Write verbs needed to build a playlist.
    for verb in ["playlists create", "playlists add", "playlists rename"] {
        assert!(
            p.allowed.iter().any(|a| a == verb),
            "playlist profile should allow `{verb}`, got {:?}",
            p.allowed
        );
    }

    // Destructive and library-mutating verbs stay denied even in the
    // playlist profile — the agent's only write path is creating a
    // new playlist plus adding tracks to it.
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
            "playlist profile should deny `{verb}`, got {:?}",
            p.denied
        );
    }

    for v in &p.denied {
        assert!(
            !p.allowed.contains(v),
            "verb `{v}` is on both allow and deny lists in playlist profile"
        );
    }
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
fn profile_default_policy_matches_factory_helpers() {
    assert_eq!(Profile::Ask.default_policy(), ask_default());
    assert_eq!(Profile::Playlist.default_policy(), playlist_default());
}

#[test]
fn prompt_block_lists_each_verb_with_spotifai_api_prefix() {
    let p = ask_default();
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
    for original in [ask_default(), playlist_default()] {
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
    let wrote = ensure_default_at(&path, &ask_default()).unwrap();
    assert!(wrote, "first ensure_default_at should report a write");
    assert!(path.exists());

    // Hand-edit the file: change the mode tag. ensure_default_at must
    // not stomp on the user's edit on the next call.
    let edited = std::fs::read_to_string(&path)
        .unwrap()
        .replace("mode = \"read_only\"", "mode = \"read_write\"");
    std::fs::write(&path, &edited).unwrap();

    let wrote_again = ensure_default_at(&path, &ask_default()).unwrap();
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
    let p = read_or(&path, ask_default()).unwrap();
    assert_eq!(p, ask_default());
}

#[test]
fn read_or_parses_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("permissions.toml");
    let mut custom = ask_default();
    custom.mode = "custom".into();
    custom.description = "test".into();
    custom.allowed = vec!["search".into()];
    custom.denied = vec![];
    std::fs::write(&path, to_toml_string(&custom).unwrap()).unwrap();

    let read = read_or(&path, ask_default()).unwrap();
    assert_eq!(read, custom);
}

#[test]
fn path_for_lives_under_dot_spotifai_permissions_dir() {
    use std::path::Path;

    for &profile in Profile::ALL {
        let path = path_for(profile).unwrap();
        let expected_tail = Path::new(".spotifai")
            .join(permissions::PERMISSIONS_DIR)
            .join(format!("{}.toml", profile.as_str()));
        assert!(
            path.ends_with(&expected_tail),
            "path for {:?} = {} did not end with {}",
            profile,
            path.display(),
            expected_tail.display(),
        );
    }
}

#[test]
fn path_for_returns_distinct_files_per_profile() {
    let ask = path_for(Profile::Ask).unwrap();
    let playlist = path_for(Profile::Playlist).unwrap();
    assert_ne!(ask, playlist);
}
