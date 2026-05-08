//! spotifai — A Rust CLI for managing your Spotify library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify integration).

pub mod api;
pub mod ask;
pub mod cli;
pub mod install;
pub mod output;
pub mod permissions;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
