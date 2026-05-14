//! `spotifai clean` — start an interactive zag session for destructive
//! cleanup of the user's library on the active provider.
//!
//! Mirrors `spotifai ask` and `spotifai playlist` but loads the
//! `clean` permission profile, which strips out the public-catalogue
//! `search` verb and the creator verbs (`playlists create|add|rename`,
//! library `save`/`like`) and adds the destructive verbs:
//! `playlists delete`, `playlists remove`, `library tracks unsave`,
//! `library albums unsave` (Spotify), and `library unlike` (YouTube
//! Music). The system prompt under `prompts/clean/` instructs the
//! agent to enumerate the candidate set, show it to the user, and
//! wait for an explicit affirmative reply before issuing any
//! destructive call.

use anyhow::Result;

use crate::permissions::{Permissions, Profile};
use crate::providers::Provider;
use crate::session;

/// Raw `prompts/clean/<version>.md` file baked in at compile time.
pub const CLEAN_PROMPT_RAW: &str = include_str!("../prompts/clean/1_0_1.md");

/// Run the `clean` command. `initial_prompt` is the user's first
/// instruction (the trailing positional arg from the CLI). `None`
/// drops straight into the interactive session with no opener.
/// `wait` and `yolo` are forwarded to [`session::run_agent`] the
/// same way `ask` and `playlist` do.
pub fn run(provider: Provider, initial_prompt: Option<&str>, wait: bool, yolo: bool) -> Result<()> {
    session::run_agent(
        provider,
        Profile::Clean,
        "clean",
        CLEAN_PROMPT_RAW,
        initial_prompt,
        wait,
        yolo,
    )
}

/// Render the system prompt by extracting the `## System` section
/// of the template and substituting the provider/permissions
/// placeholders. The `## User` section is dropped — the user types
/// their instruction interactively into zag.
pub fn render_system_prompt(template: &str, provider: Provider, policy: &Permissions) -> String {
    session::render_system_prompt(template, provider, policy)
}

/// Pull the `## System` block out of a prompt-template Markdown
/// file. See [`crate::session::extract_system_section`] for the
/// contract.
pub fn extract_system_section(template: &str) -> String {
    session::extract_system_section(template)
}
