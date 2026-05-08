//! Pure-function tests for `spotifai::ask` — prompt parsing and
//! permissions injection. No tokio runtime, no zag spawn.

use spotifai::ask::{ASK_PROMPT_RAW, extract_system_section, render_system_prompt};
use spotifai::permissions::Permissions;

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
    // would silently truncate the system prompt.
    let body = extract_system_section(ASK_PROMPT_RAW);
    for needle in [
        "### How to talk to Spotify",
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
fn render_system_prompt_substitutes_permissions_block() {
    let policy = Permissions::read_only_default();
    let rendered = render_system_prompt(ASK_PROMPT_RAW, &policy);

    assert!(
        !rendered.contains("{{ permissions_block }}"),
        "permissions placeholder not substituted: {rendered}"
    );

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
    let policy = Permissions::read_only_default();
    let rendered = render_system_prompt(ASK_PROMPT_RAW, &policy);

    // The agent must be told it talks to Spotify through `spotifai
    // api …`, not by hitting Spotify Web API directly. This is the
    // load-bearing line of the whole prompt.
    assert!(
        rendered.contains("`spotifai api`"),
        "rendered prompt does not mention `spotifai api`:\n{rendered}"
    );
    assert!(
        rendered.contains("do **not** call the Spotify Web API directly")
            || rendered.contains("do not call the Spotify Web API"),
        "rendered prompt missing the no-direct-Spotify guard:\n{rendered}"
    );
}
