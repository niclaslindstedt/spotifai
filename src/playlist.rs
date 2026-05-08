//! `spotifai playlist` — start an interactive zag session pre-loaded
//! with a system prompt that walks the agent through building a new
//! playlist for the user.
//!
//! Mirrors `spotifai ask` but loads the `playlist` permission profile
//! (which adds `playlists create|add|rename` on top of the read-only
//! verbs `ask` ships with). The agent is restricted to creating one
//! new playlist per session — destructive verbs stay denied.

use anyhow::Result;

use crate::permissions::{Permissions, Profile};
use crate::session;

/// Raw `prompts/playlist/<version>.md` file baked in at compile time.
pub const PLAYLIST_PROMPT_RAW: &str = include_str!("../prompts/playlist/1_0_0.md");

/// Run the `playlist` command. `initial_prompt` is the user's first
/// brief (the trailing positional arg from the CLI). `None` drops
/// straight into the interactive session with no opener.
pub fn run(initial_prompt: Option<&str>) -> Result<()> {
    session::run_agent(
        Profile::Playlist,
        "playlist",
        PLAYLIST_PROMPT_RAW,
        initial_prompt,
    )
}

/// Render the system prompt by extracting the `## System` section of
/// the template and substituting `{{ permissions_block }}`. The
/// `## User` section is dropped — the user types their query
/// interactively into zag.
pub fn render_system_prompt(template: &str, policy: &Permissions) -> String {
    session::render_system_prompt(template, policy)
}

/// Pull the `## System` block out of a prompt-template Markdown file.
/// See [`crate::session::extract_system_section`] for the contract.
pub fn extract_system_section(template: &str) -> String {
    session::extract_system_section(template)
}
