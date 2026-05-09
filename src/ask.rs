//! `spotifai ask` — start an interactive zag session pre-loaded with
//! a system prompt that explains how to drive `spotifai api …` and
//! injects the local permissions file so the agent self-restricts.
//!
//! The system prompt template lives at `prompts/ask/1_1_1.md` and is
//! baked in at compile time via `include_str!`. It is rendered with
//! the active provider's display name and example block, plus the
//! `(provider, profile)` permissions policy. The agent runs
//! `spotifai api …` through its shell tool to talk to the active
//! provider, so it is the user's responsibility to keep `spotifai`
//! on `$PATH` for the interactive session.

use anyhow::Result;

use crate::permissions::{Permissions, Profile};
use crate::providers::Provider;
use crate::session;

/// Raw `prompts/ask/<version>.md` file baked in at compile time.
pub const ASK_PROMPT_RAW: &str = include_str!("../prompts/ask/1_1_1.md");

/// Run the `ask` command. `initial_prompt` is the user's first
/// question (the trailing positional arg from the CLI). `None` drops
/// straight into the interactive session with no opener.
pub fn run(provider: Provider, initial_prompt: Option<&str>) -> Result<()> {
    session::run_agent(
        provider,
        Profile::Ask,
        "ask",
        ASK_PROMPT_RAW,
        initial_prompt,
    )
}

/// Render the system prompt by extracting the `## System` section
/// of the template and substituting the provider/permissions
/// placeholders. The `## User` section is dropped — the user types
/// their query interactively into zag.
pub fn render_system_prompt(template: &str, provider: Provider, policy: &Permissions) -> String {
    session::render_system_prompt(template, provider, policy)
}

/// Pull the `## System` block out of a prompt-template Markdown
/// file. Returns the body between `## System` and the next `## `
/// heading, trimmed. Falls back to the raw template if no
/// `## System` heading is present so we never silently end up with
/// an empty prompt.
pub fn extract_system_section(template: &str) -> String {
    session::extract_system_section(template)
}
