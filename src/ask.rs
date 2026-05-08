//! `spotifai ask` — start an interactive zag session pre-loaded with
//! a system prompt that explains how to drive `spotifai api …` and
//! injects the local permissions file so the agent self-restricts.
//!
//! The system prompt template lives at `prompts/ask/1_0_1.md` and is
//! baked in at compile time via `include_str!`. The agent runs
//! `spotifai api …` through its shell tool to talk to Spotify, so it
//! is the user's responsibility to keep `spotifai` on `$PATH` for the
//! interactive session.

use anyhow::Result;

use crate::permissions::{Permissions, Profile};
use crate::session;

/// Raw `prompts/ask/<version>.md` file baked in at compile time.
pub const ASK_PROMPT_RAW: &str = include_str!("../prompts/ask/1_0_1.md");

/// Run the `ask` command. `initial_prompt` is the user's first
/// question (the trailing positional arg from the CLI). `None` drops
/// straight into the interactive session with no opener.
pub fn run(initial_prompt: Option<&str>) -> Result<()> {
    session::run_agent(Profile::Ask, "ask", ASK_PROMPT_RAW, initial_prompt)
}

/// Render the system prompt by extracting the `## System` section of
/// the template and substituting `{{ permissions_block }}`. The
/// `## User` section is dropped — the user types their query
/// interactively into zag.
pub fn render_system_prompt(template: &str, policy: &Permissions) -> String {
    session::render_system_prompt(template, policy)
}

/// Pull the `## System` block out of a prompt-template Markdown file.
/// Returns the body between `## System` and the next `## ` heading,
/// trimmed. Falls back to the raw template if no `## System` heading
/// is present so we never silently end up with an empty prompt.
pub fn extract_system_section(template: &str) -> String {
    session::extract_system_section(template)
}
