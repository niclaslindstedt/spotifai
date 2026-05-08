//! `spotifai ask` — start an interactive zag session pre-loaded with
//! a system prompt that explains how to drive `spotifai api …` and
//! injects the local permissions file so the agent self-restricts.
//!
//! The system prompt template lives at `prompts/ask/1_0_0.md` and is
//! baked in at compile time via `include_str!`. The agent runs
//! `spotifai api …` through its shell tool to talk to Spotify, so it
//! is the user's responsibility to keep `spotifai` on `$PATH` for the
//! interactive session.

use anyhow::{Context, Result};
use zag::builder::AgentBuilder;

use crate::install;
use crate::output;
use crate::permissions::{self, Permissions};

/// Raw `prompts/ask/<version>.md` file baked in at compile time.
pub const ASK_PROMPT_RAW: &str = include_str!("../prompts/ask/1_0_0.md");

/// Run the `ask` command. `initial_prompt` is the user's first
/// question (the trailing positional arg from the CLI). `None` drops
/// straight into the interactive session with no opener.
pub fn run(initial_prompt: Option<&str>) -> Result<()> {
    // Make sure the pinned zad binary is on disk before the agent
    // tries to shell out through it.
    install::ensure_installed(false)?;

    // Always make sure the policy file exists before we read it. The
    // first run creates a read-only default; subsequent runs are a
    // no-op so user edits are preserved.
    let policy_path = permissions::default_path()?;
    if permissions::ensure_default(&policy_path)? {
        output::status(&format!(
            "wrote default read-only permissions to {}",
            policy_path.display()
        ));
    }
    let policy = permissions::read_or_default(&policy_path)?;

    let system_prompt = render_system_prompt(ASK_PROMPT_RAW, &policy);

    output::header("spotifai ask");
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
/// the template and substituting `{{ permissions_block }}`. The
/// `## User` section is dropped — the user types their query
/// interactively into zag.
pub fn render_system_prompt(template: &str, policy: &Permissions) -> String {
    let system_section = extract_system_section(template);
    system_section.replace("{{ permissions_block }}", &policy.as_prompt_block())
}

/// Pull the `## System` block out of a prompt-template Markdown file.
/// Returns the body between `## System` and the next `## ` heading,
/// trimmed. Falls back to the raw template if no `## System` heading
/// is present so we never silently end up with an empty prompt.
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
