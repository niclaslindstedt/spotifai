//! Pure-function tests for `spotifai::playlist` — prompt parsing and
//! permissions injection. No tokio runtime, no zag spawn.

use spotifai::permissions::playlist_default;
use spotifai::playlist::{PLAYLIST_PROMPT_RAW, extract_system_section, render_system_prompt};

#[test]
fn extract_system_section_strips_yaml_front_matter() {
    let body = extract_system_section(PLAYLIST_PROMPT_RAW);
    assert!(
        !body.starts_with("---"),
        "front matter leaked into system body: {body}"
    );
    assert!(
        !body.contains("description:"),
        "front matter `description:` key leaked: {body}"
    );
}

#[test]
fn extract_system_section_drops_user_section() {
    let body = extract_system_section(PLAYLIST_PROMPT_RAW);
    assert!(
        !body.contains("{{ user_query }}"),
        "User section leaked into system prompt: {body}"
    );
    assert!(
        !body.contains("\n## User"),
        "User heading leaked into system prompt: {body}"
    );
}

#[test]
fn extract_system_section_keeps_subheadings() {
    let body = extract_system_section(PLAYLIST_PROMPT_RAW);
    for needle in [
        "### How to talk to Spotify",
        "### Permissions — read this every turn",
        "### Style",
    ] {
        assert!(
            body.contains(needle),
            "playlist prompt missing subsection `{needle}`:\n{body}"
        );
    }
}

#[test]
fn render_system_prompt_substitutes_permissions_block() {
    let policy = playlist_default();
    let rendered = render_system_prompt(PLAYLIST_PROMPT_RAW, &policy);

    assert!(
        !rendered.contains("{{ permissions_block }}"),
        "permissions placeholder not substituted: {rendered}"
    );

    let block = policy.as_prompt_block();
    for line in block.lines().filter(|l| !l.is_empty()) {
        assert!(
            rendered.contains(line),
            "rendered prompt missing line `{line}` from permissions block:\n{rendered}"
        );
    }
}

#[test]
fn rendered_prompt_lists_playlist_create_and_add_verbs() {
    let policy = playlist_default();
    let rendered = render_system_prompt(PLAYLIST_PROMPT_RAW, &policy);
    for needle in [
        "`spotifai api playlists create`",
        "`spotifai api playlists add",
        "`spotifai api search",
    ] {
        assert!(
            rendered.contains(needle),
            "rendered playlist prompt missing example `{needle}`:\n{rendered}"
        );
    }
}

#[test]
fn rendered_prompt_does_not_leak_profile_env_var_name() {
    // Same constraint as `ask`: never tell the agent how profile
    // selection is wired, so it has nothing to bind to if it tries
    // to escalate.
    let policy = playlist_default();
    let rendered = render_system_prompt(PLAYLIST_PROMPT_RAW, &policy);
    for needle in ["SPOTIFAI_PROFILE", "ZAD_PERMISSIONS_PATH"] {
        assert!(
            !rendered.contains(needle),
            "rendered playlist prompt leaks env var name `{needle}`:\n{rendered}"
        );
    }
}
