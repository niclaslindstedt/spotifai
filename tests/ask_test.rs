//! Pure-function tests for `spotifai::ask` — prompt parsing and
//! permissions injection. No tokio runtime, no zag spawn.

use spotifai::ask::{ASK_PROMPT_RAW, extract_system_section, render_system_prompt};
use spotifai::permissions::Profile;
use spotifai::providers::Provider;

#[test]
fn extract_system_section_strips_yaml_front_matter() {
    let body = extract_system_section(ASK_PROMPT_RAW);
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
    // The template's `## User` block contains `{{ user_query }}`,
    // which is meaningless to the agent and would just confuse the
    // model if it leaked into the system prompt.
    let body = extract_system_section(ASK_PROMPT_RAW);
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
    // Subsections under System use `### ` so they MUST survive
    // extraction. If someone bumps them back to `## `, the extractor
    // would silently truncate the system prompt. The provider name
    // is templated, so the heading is checked with the substituted
    // form below.
    let body = extract_system_section(ASK_PROMPT_RAW);
    for needle in [
        "### How to talk to {{ provider_name }}",
        "### Permissions — read this every turn",
        "### Style",
    ] {
        assert!(
            body.contains(needle),
            "system prompt missing subsection `{needle}`:\n{body}"
        );
    }
}

#[test]
fn extract_system_section_falls_back_to_raw_body_when_marker_missing() {
    let raw = "# title\n\nplain content with no section markers.\n";
    let extracted = extract_system_section(raw);
    assert_eq!(
        extracted,
        "# title\n\nplain content with no section markers."
    );
}

#[test]
fn render_system_prompt_substitutes_permissions_and_provider_blocks() {
    let policy = Profile::Ask.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(ASK_PROMPT_RAW, Provider::Spotify, &policy);

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

    // The whole policy block — both allow- and deny-list lines —
    // must end up inside the rendered prompt verbatim.
    let block = policy.as_prompt_block();
    for line in block.lines().filter(|l| !l.is_empty()) {
        assert!(
            rendered.contains(line),
            "rendered prompt missing line `{line}` from permissions block:\n{rendered}"
        );
    }
}

#[test]
fn render_system_prompt_mentions_spotifai_api_usage() {
    let policy = Profile::Ask.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(ASK_PROMPT_RAW, Provider::Spotify, &policy);

    // The agent must be told it talks to the provider through
    // `spotifai api …`, not by hitting the API directly. This is
    // the load-bearing line of the whole prompt.
    assert!(
        rendered.contains("`spotifai api`"),
        "rendered prompt does not mention `spotifai api`:\n{rendered}"
    );
    assert!(
        rendered.contains("do **not** call the Spotify API directly"),
        "rendered Spotify prompt missing the no-direct-API guard:\n{rendered}"
    );
}

#[test]
fn render_system_prompt_branches_on_provider() {
    // Spotify and YouTube Music branches must each render the
    // matching display name and example block.
    let spotify_policy = Profile::Ask.default_policy(Provider::Spotify);
    let spotify_rendered = render_system_prompt(ASK_PROMPT_RAW, Provider::Spotify, &spotify_policy);
    assert!(
        spotify_rendered.contains("How to talk to Spotify"),
        "Spotify rendering missing display-name substitution:\n{spotify_rendered}"
    );
    assert!(
        spotify_rendered.contains("library albums list"),
        "Spotify rendering missing Spotify-specific verb:\n{spotify_rendered}"
    );

    let ymusic_policy = Profile::Ask.default_policy(Provider::YouTubeMusic);
    let ymusic_rendered =
        render_system_prompt(ASK_PROMPT_RAW, Provider::YouTubeMusic, &ymusic_policy);
    assert!(
        ymusic_rendered.contains("How to talk to YouTube Music"),
        "YouTube Music rendering missing display-name substitution:\n{ymusic_rendered}"
    );
    assert!(
        !ymusic_rendered.contains("library albums list"),
        "YouTube Music rendering should not mention Spotify-only verbs:\n{ymusic_rendered}"
    );
}

#[test]
fn rendered_prompt_does_not_leak_internal_env_var_names() {
    // The agent must not learn that profile/provider selection is
    // mediated by env vars, otherwise a sufficiently clever model
    // could try to escalate by spawning `spotifai api` with a
    // different (provider, profile) pair.
    let policy = Profile::Ask.default_policy(Provider::Spotify);
    let rendered = render_system_prompt(ASK_PROMPT_RAW, Provider::Spotify, &policy);
    for needle in [
        "SPOTIFAI_PROFILE",
        "SPOTIFAI_PROVIDER",
        "ZAD_PERMISSIONS_PATH",
    ] {
        assert!(
            !rendered.contains(needle),
            "rendered prompt leaks env var name `{needle}`:\n{rendered}"
        );
    }
}
