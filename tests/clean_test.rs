//! Pure-function tests for `spotifai::clean` — prompt parsing,
//! permissions injection, and verb-coverage smoke tests. No tokio
//! runtime, no zag spawn.

use spotifai::clean::{CLEAN_PROMPT_RAW, extract_system_section, render_system_prompt};
use spotifai::permissions::Profile;
use spotifai::providers::Provider;

#[test]
fn extract_system_section_strips_yaml_front_matter() {
    let body = extract_system_section(CLEAN_PROMPT_RAW);
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
    let body = extract_system_section(CLEAN_PROMPT_RAW);
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
    let body = extract_system_section(CLEAN_PROMPT_RAW);
    for needle in [
        "### How to talk to {{ provider_name }}",
        "### Cleanup workflow — read this every turn",
        "### Permissions — read this every turn",
        "### Style",
    ] {
        assert!(
            body.contains(needle),
            "clean prompt missing subsection `{needle}`:\n{body}"
        );
    }
}

#[test]
fn render_system_prompt_substitutes_permissions_and_provider_blocks() {
    let policy = Profile::Clean.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(CLEAN_PROMPT_RAW, Provider::Spotify, &policy);

    for placeholder in [
        "{{ permissions_block }}",
        "{{ provider_name }}",
        "{{ provider_examples }}",
    ] {
        assert!(
            !rendered.contains(placeholder),
            "placeholder `{placeholder}` not substituted: {rendered}"
        );
    }

    let block = policy.as_prompt_block();
    for line in block.lines().filter(|l| !l.is_empty()) {
        assert!(
            rendered.contains(line),
            "rendered prompt missing line `{line}` from permissions block:\n{rendered}"
        );
    }
}

#[test]
fn rendered_spotify_prompt_lists_destructive_verbs() {
    let policy = Profile::Clean.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(CLEAN_PROMPT_RAW, Provider::Spotify, &policy);
    for needle in [
        "`spotifai api playlists delete`",
        "`spotifai api playlists remove`",
        "`spotifai api library tracks unsave`",
        "`spotifai api library albums unsave`",
    ] {
        assert!(
            rendered.contains(needle),
            "rendered Spotify clean prompt missing destructive verb `{needle}`:\n{rendered}"
        );
    }
}

#[test]
fn rendered_spotify_prompt_denies_creator_and_search_verbs() {
    let policy = Profile::Clean.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(CLEAN_PROMPT_RAW, Provider::Spotify, &policy);
    // `search`, `playlists create`, `playlists add`, `playlists rename`
    // must appear on the denied list (rendered verbatim from
    // `Permissions::as_prompt_block`). They will also appear in the
    // `{{ provider_examples }}` block as part of the provider's vocab,
    // but the denied-list rendering is what locks the agent down.
    let denied_block = policy.as_prompt_block();
    for needle in [
        "`spotifai api search`",
        "`spotifai api playlists create`",
        "`spotifai api playlists add`",
        "`spotifai api playlists rename`",
    ] {
        assert!(
            denied_block.contains(needle),
            "Spotify clean policy missing denied entry `{needle}` in its prompt block:\n{denied_block}"
        );
        assert!(
            rendered.contains(needle),
            "rendered Spotify clean prompt missing denied entry `{needle}`:\n{rendered}"
        );
    }
}

#[test]
fn rendered_ymusic_prompt_uses_ymusic_destructive_verbs() {
    let policy = Profile::Clean.default_policy(Provider::YouTubeMusic);
    let rendered = render_system_prompt(CLEAN_PROMPT_RAW, Provider::YouTubeMusic, &policy);
    assert!(
        rendered.contains("How to talk to YouTube Music"),
        "ymusic clean prompt missing display-name substitution:\n{rendered}"
    );
    assert!(
        rendered.contains("`spotifai api library unlike`"),
        "ymusic clean prompt missing the unlike verb:\n{rendered}"
    );
    assert!(
        !rendered.contains("`spotifai api library albums unsave`"),
        "ymusic clean prompt should not mention Spotify-only `library albums unsave`:\n{rendered}"
    );
}

#[test]
fn rendered_prompt_requires_confirmation_step() {
    // The cleanup workflow is the whole point of `spotifai clean` — if a
    // future prompt revision drops the confirmation gate, this test
    // turns red.
    let policy = Profile::Clean.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(CLEAN_PROMPT_RAW, Provider::Spotify, &policy);
    assert!(
        rendered.contains("Proceed with deleting"),
        "clean prompt missing the explicit yes/no confirmation gate:\n{rendered}"
    );
    assert!(
        rendered.contains("Read → list → confirm → delete"),
        "clean prompt missing the read-then-confirm workflow header:\n{rendered}"
    );
}

#[test]
fn rendered_prompt_does_not_leak_internal_env_var_names() {
    let policy = Profile::Clean.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(CLEAN_PROMPT_RAW, Provider::Spotify, &policy);
    for needle in [
        "SPOTIFAI_PROFILE",
        "SPOTIFAI_PROVIDER",
        "ZAD_PERMISSIONS_PATH",
    ] {
        assert!(
            !rendered.contains(needle),
            "rendered clean prompt leaks env var name `{needle}`:\n{rendered}"
        );
    }
}
