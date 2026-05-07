//! spotifai — A Rust CLI for managing your Spotify library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify integration).

pub mod output;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}