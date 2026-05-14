//! spotifai — A Rust CLI for managing your Spotify library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify integration).

pub mod api;
pub mod api_fields;
pub mod ask;
pub mod auth;
pub mod clean;
pub mod cli;
pub mod commands_index;
pub mod export;
pub mod export_schema;
pub mod help_agent;
pub mod import;
pub mod import_state;
pub mod install;
pub mod logging;
pub mod manpages;
pub mod output;
pub mod permissions;
pub mod playlist;
pub mod providers;
pub mod session;
pub mod topic_docs;
pub mod zad_client;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
