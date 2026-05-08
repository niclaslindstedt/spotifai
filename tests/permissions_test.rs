//! Pure-function tests for `spotifai::permissions`.

use spotifai::permissions::{
    self, Permissions, ensure_default, from_toml_string, read_or_default, to_toml_string,
};
use tempfile::tempdir;

#[test]
fn read_only_default_locks_down_every_write_verb() {
    let p = Permissions::read_only_default();
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
            "read-only default should allow `{verb}`, got {:?}",
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
            "read-only default should deny `{verb}`, got {:?}",
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
fn prompt_block_lists_each_verb_with_spotifai_api_prefix() {
    let p = Permissions::read_only_default();
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
    let original = Permissions::read_only_default();
    let serialized = to_toml_string(&original).unwrap();
    assert!(
        serialized.starts_with("# spotifai permissions"),
        "serialized output missing header comment:\n{serialized}"
    );
    let parsed = from_toml_string(&serialized).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn ensure_default_writes_file_only_once() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("permissions.toml");

    // First call writes the default policy.
    let wrote = ensure_default(&path).unwrap();
    assert!(wrote, "first ensure_default should report a write");
    assert!(path.exists());

    // Hand-edit the file: change the mode tag. ensure_default must
    // not stomp on the user's edit on the next call.
    let edited = std::fs::read_to_string(&path)
        .unwrap()
        .replace("mode = \"read_only\"", "mode = \"read_write\"");
    std::fs::write(&path, &edited).unwrap();

    let wrote_again = ensure_default(&path).unwrap();
    assert!(!wrote_again, "second ensure_default should be a no-op");

    let after = std::fs::read_to_string(&path).unwrap();
    assert!(
        after.contains("mode = \"read_write\""),
        "ensure_default overwrote a user edit:\n{after}"
    );
}

#[test]
fn read_or_default_returns_in_memory_default_for_missing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does-not-exist.toml");
    assert!(!path.exists());
    let p = read_or_default(&path).unwrap();
    assert_eq!(p, Permissions::read_only_default());
}

#[test]
fn read_or_default_parses_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("permissions.toml");
    let mut custom = Permissions::read_only_default();
    custom.mode = "custom".into();
    custom.description = "test".into();
    custom.allowed = vec!["search".into()];
    custom.denied = vec![];
    std::fs::write(&path, to_toml_string(&custom).unwrap()).unwrap();

    let read = read_or_default(&path).unwrap();
    assert_eq!(read, custom);
}

#[test]
fn default_path_lives_under_dot_spotifai_home_dir() {
    let path = permissions::default_path().unwrap();
    let s = path.to_string_lossy();
    assert!(s.ends_with(".spotifai/permissions.toml"), "path = {s}");
}
