//! Provider-specific helpers around the `zad` Rust library.
//!
//! `zad::service::spotify::Spotify` and `zad::service::ymusic::Ymusic`
//! are the two typed facades spotifai talks to in-process. Each one
//! needs OAuth credentials that `spotifai auth` writes into the OS
//! keychain (account names `spotify-client-id:global`,
//! `spotify-refresh:global`, `ymusic-client-id:global`,
//! `ymusic-client-secret:global`, `ymusic-refresh:global`), plus the
//! authenticated user's identifier (Spotify user id, YouTube channel
//! id) which `create_playlist` requires. The identifier is captured
//! at OAuth time and persisted at `~/.spotifai/<provider>.toml`.
//!
//! `with_credentials` takes a `config_path` argument that the
//! underlying http client only uses for diagnostic strings. We pass
//! the per-provider self-id file as a placeholder — nothing in the
//! codepaths spotifai exercises reads or writes it.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use zad::oauth::KeychainRefreshStore;
use zad::secrets::{self, Scope};
use zad::service::spotify::{Spotify, SpotifyCredentials, SpotifyHttp};
use zad::service::ymusic::{Ymusic, YmusicCredentials, YmusicHttp};

use crate::providers::Provider;

/// Per-provider self-identifier captured at OAuth time.
///
/// Spotify needs `user_id` for `create_playlist`; YouTube Music does
/// not need a channel id at request time but the field is kept for
/// symmetry with Spotify and for diagnostic output.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelfIdentity {
    /// Spotify only: the authenticated user's Spotify user id, used
    /// as the path component in `POST /users/{user_id}/playlists`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// YouTube Music only: the authenticated user's YouTube channel
    /// id (kept for future use; YouTube Music's create_playlist
    /// endpoint infers the channel from the access token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    /// Optional human-readable label (display name / channel title).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional email captured by the OAuth userinfo probe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Resolve the per-provider self-identity file path
/// (`~/.spotifai/<provider>.toml`).
pub fn self_id_path(provider: Provider) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(home
        .join(".spotifai")
        .join(format!("{}.toml", provider.as_str())))
}

/// Read the per-provider self-identity file, or return an empty value
/// if the file does not exist.
pub fn read_self_identity(provider: Provider) -> Result<SelfIdentity> {
    let path = self_id_path(provider)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s)
            .with_context(|| format!("parsing self-identity file {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SelfIdentity::default()),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

/// Write the per-provider self-identity file. Creates parent
/// directories as needed.
pub fn write_self_identity(provider: Provider, identity: &SelfIdentity) -> Result<()> {
    let path = self_id_path(provider)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let body = toml::to_string_pretty(identity).context("serializing self-identity TOML")?;
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// zad scope strings matching the verbs in `Permissions::allowed` /
/// `denied`. Exposed as a small BTreeSet so callers can hand it to
/// `Spotify::with_credentials` / `Ymusic::with_credentials`.
///
/// Spotify and YouTube Music share the same vocabulary — `search`,
/// `playlists.read`, `playlists.write`, `library.read`,
/// `library.write` — so a single helper suffices.
pub fn scopes_for_profile(profile: crate::permissions::Profile) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    out.insert("search".into());
    out.insert("playlists.read".into());
    out.insert("library.read".into());
    if matches!(profile, crate::permissions::Profile::Playlist) {
        out.insert("playlists.write".into());
    }
    out
}

/// Build a Spotify facade from the keychain credentials written by
/// `spotifai auth`.
pub fn load_spotify(scopes: BTreeSet<String>) -> Result<Spotify> {
    let creds = load_spotify_credentials()?;
    let config_path = self_id_path(Provider::Spotify)?;
    Ok(Spotify::with_credentials(creds, scopes, config_path))
}

/// Build a Spotify facade for "everything" (every scope spotifai
/// supports). Used by surfaces that need to dispatch verbs across
/// several scope buckets in a single client.
pub fn load_spotify_all() -> Result<Spotify> {
    load_spotify(all_scopes())
}

/// Build the underlying [`SpotifyHttp`] directly. Needed for verbs
/// the typed facade does not yet expose in zad 0.6.5
/// (`add_playlist_tracks`, `list_saved_albums`, `list_my_playlists`,
/// `get_playlist_tracks`).
pub fn load_spotify_http(scopes: BTreeSet<String>) -> Result<SpotifyHttp> {
    let creds = load_spotify_credentials()?;
    let config_path = self_id_path(Provider::Spotify)?;
    Ok(SpotifyHttp::with_store(
        creds.client_id,
        creds.refresh_token,
        scopes,
        config_path,
        creds.refresh_token_store,
    ))
}

/// Build a YouTube Music facade from the keychain credentials
/// written by `spotifai auth`.
pub fn load_ymusic(scopes: BTreeSet<String>) -> Result<Ymusic> {
    let creds = load_ymusic_credentials()?;
    let config_path = self_id_path(Provider::YouTubeMusic)?;
    Ok(Ymusic::with_credentials(creds, scopes, config_path))
}

pub fn load_ymusic_all() -> Result<Ymusic> {
    load_ymusic(all_scopes())
}

/// Build the underlying [`YmusicHttp`] directly. Needed for
/// `get_playlist_items` and any other verb the typed facade does not
/// expose.
pub fn load_ymusic_http(scopes: BTreeSet<String>) -> Result<YmusicHttp> {
    let creds = load_ymusic_credentials()?;
    let config_path = self_id_path(Provider::YouTubeMusic)?;
    Ok(YmusicHttp::with_store(
        creds.client_id,
        creds.client_secret,
        creds.refresh_token,
        scopes,
        config_path,
        creds.refresh_token_store,
    ))
}

fn all_scopes() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    out.insert("search".into());
    out.insert("playlists.read".into());
    out.insert("playlists.write".into());
    out.insert("library.read".into());
    out.insert("library.write".into());
    out
}

fn load_spotify_credentials() -> Result<SpotifyCredentials> {
    let client_id =
        load_keychain_or_hint("spotify", "client-id", "spotifai auth --provider spotify")?;
    let refresh_token =
        load_keychain_or_hint("spotify", "refresh", "spotifai auth --provider spotify")?;
    let store = KeychainRefreshStore::new(secrets::account("spotify", "refresh", Scope::Global));
    Ok(SpotifyCredentials {
        client_id,
        refresh_token,
        refresh_token_store: Some(Arc::new(store)),
    })
}

fn load_ymusic_credentials() -> Result<YmusicCredentials> {
    let client_id =
        load_keychain_or_hint("ymusic", "client-id", "spotifai auth --provider ymusic")?;
    let client_secret =
        load_keychain_or_hint("ymusic", "client-secret", "spotifai auth --provider ymusic")?;
    let refresh_token =
        load_keychain_or_hint("ymusic", "refresh", "spotifai auth --provider ymusic")?;
    let store = KeychainRefreshStore::new(secrets::account("ymusic", "refresh", Scope::Global));
    Ok(YmusicCredentials {
        client_id,
        client_secret,
        refresh_token,
        refresh_token_store: Some(Arc::new(store)),
    })
}

fn load_keychain_or_hint(service: &str, kind: &str, hint: &str) -> Result<String> {
    let account = secrets::account(service, kind, Scope::Global);
    secrets::load(&account)
        .with_context(|| format!("reading keychain entry `zad/{account}`"))?
        .ok_or_else(|| {
            anyhow!(
                "missing {service} {kind} in OS keychain (account `zad/{account}`); \
                 run `{hint}` to register credentials"
            )
        })
}
