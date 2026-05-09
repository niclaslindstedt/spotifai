//! Shared agent-runner used by `spotifai ask` and `spotifai playlist`.
//!
//! Both commands follow the same shape: ensure the pinned zad binary
//! is on disk, scaffold the matching
//! `~/.spotifai/permissions/<provider>/<profile>.toml` if missing,
//! render a Markdown prompt template with the policy and the
//! provider's example block injected, set
//! [`crate::api::SPOTIFAI_PROVIDER_ENV`] and
//! [`crate::api::SPOTIFAI_PROFILE_ENV`] so the child `spotifai api`
//! shells route to the correct policy file, and then hand control
//! over to zag.
//!
//! The prompt template format is shared too — see
//! [`prompts/README.md`](../prompts/README.md). The `## System`
//! section is what gets sent to the model; the `## User` block is
//! for the authoring workflow only and is dropped here.

use anyhow::{Context, Result};
use zag::builder::AgentBuilder;

use crate::api::{SPOTIFAI_PROFILE_ENV, SPOTIFAI_PROVIDER_ENV};
use crate::output;
use crate::permissions::{self, Permissions, Profile};
use crate::providers::Provider;

/// Drive one of spotifai's interactive zag surfaces end-to-end.
///
/// `command_label` is the display name printed to stderr when the
/// session starts (e.g. `"ask"`, `"playlist"`).
/// `prompt_template` is the raw Markdown file as baked in via
/// `include_str!`. `initial_prompt` becomes the agent's first turn;
/// `None` opens the session empty.
pub fn run_agent(
    provider: Provider,
    profile: Profile,
    command_label: &str,
    prompt_template: &str,
    initial_prompt: Option<&str>,
) -> Result<()> {
    // Always make sure the policy file exists before we read it. The
    // first run creates a default; subsequent runs are a no-op so
    // user edits are preserved.
    let (policy_path, wrote) = permissions::ensure_default_for(provider, profile)?;
    if wrote {
        output::status(&format!(
            "wrote default {} × {} permissions to {}",
            provider.as_str(),
            profile.as_str(),
            policy_path.display()
        ));
    }
    let policy = permissions::read_or(&policy_path, profile.default_policy(provider))?;

    let system_prompt = render_system_prompt(prompt_template, provider, &policy);

    // Pin the active provider + profile so child `spotifai api`
    // shells resolve to the same file we just rendered into the
    // prompt. Set on this process's env so it propagates to anything
    // zag spawns. The spotifai process handles exactly one command
    // per invocation, so a global set is safe.
    //
    // SAFETY: `set_var` is unsafe in the 2024 edition because POSIX
    // forbids env mutation in multithreaded programs. spotifai is
    // single-threaded at this point — the tokio runtime below is
    // built *after* the env is set — so the call is sound.
    unsafe {
        std::env::set_var(SPOTIFAI_PROVIDER_ENV, provider.as_str());
        std::env::set_var(SPOTIFAI_PROFILE_ENV, profile.as_str());
    }

    output::header(&format!(
        "spotifai {command_label} ({})",
        provider.display_name()
    ));
    output::info(&format!("permissions: {}", policy_path.display()));
    output::info("starting interactive zag session — Ctrl+D to exit\n");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    rt.block_on(async move {
        AgentBuilder::new()
            .system_prompt(&system_prompt)
            .run(initial_prompt)
            .await
    })
}

/// Render the system prompt by extracting the `## System` section of
/// the template and substituting `{{ permissions_block }}`,
/// `{{ provider_name }}`, and `{{ provider_examples }}`. The
/// `## User` section is dropped — the user types their query
/// interactively into zag.
pub fn render_system_prompt(template: &str, provider: Provider, policy: &Permissions) -> String {
    let system_section = extract_system_section(template);
    system_section
        .replace("{{ permissions_block }}", &policy.as_prompt_block())
        .replace("{{ provider_name }}", provider.display_name())
        .replace("{{ provider_examples }}", provider.api_examples())
}

/// Pull the `## System` block out of a prompt-template Markdown
/// file. Returns the body between `## System` and the next `## `
/// heading, trimmed. Falls back to the raw template if no
/// `## System` heading is present so we never silently end up with
/// an empty prompt.
pub fn extract_system_section(template: &str) -> String {
    let body = strip_front_matter(template);
    let mut lines = body.lines();
    let mut in_system = false;
    let mut collected = String::new();
    for line in lines.by_ref() {
        let trimmed = line.trim_start();
        if !in_system {
            if trimmed.starts_with("## ") && trimmed[3..].trim().eq_ignore_ascii_case("system") {
                in_system = true;
            }
            continue;
        }
        if trimmed.starts_with("## ") {
            break;
        }
        collected.push_str(line);
        collected.push('\n');
    }
    if !in_system {
        return body.trim().to_string();
    }
    collected.trim().to_string()
}

fn strip_front_matter(template: &str) -> &str {
    let trimmed = template.trim_start_matches('\u{feff}');
    if let Some(rest) = trimmed.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---\n")
    {
        return &rest[end + "\n---\n".len()..];
    }
    trimmed
}
