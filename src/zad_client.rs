//! Provider-specific helpers around the `zad` Rust library.
//!
//! `zad::service::spotify::Spotify` and `zad::service::ymusic::Ymusic`
//! are the two typed facades spotifai talks to in-process. Each one
//! needs OAuth credentials that `spotifai auth` writes into the OS
//! keychain (account names `spotify-client-id:global`,
//! `spotify-refresh:global`, and `ymusic-refresh:global` — the
//! ymusic TVHTML5 client_id / client_secret are zad constants and
//! not stored per user), plus the authenticated user's identifier
//! (Spotify user id, YouTube channel id) which `create_playlist`
//! requires. The identifier is captured at OAuth time and persisted
//! at `~/.spotifai/<provider>.toml`.
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
use zad::rate_limit;
use zad::secrets::{self, Scope};
use zad::service::spotify::{Spotify, SpotifyCredentials, SpotifyHttp};
use zad::service::ymusic::{Ymusic, YmusicCredentials, YmusicHttp};

use crate::providers::Provider;

/// Env var read by every spotifai surface to decide whether a stale
/// rate-limit window should block (`SPOTIFAI_WAIT=1`) or fail fast
/// (unset / `0`).
///
/// `spotifai ask` and `spotifai playlist` set this to `1` so every
/// child `spotifai api` invocation a sub-agent spawns inherits
/// "respect the cooldown" behaviour automatically. Spotify enforces
/// rolling-window rate limits per *application*, so multiple agents
/// hammering the same client at once will trip 429s without this
/// coordination. YouTube Music expresses the same situation as a
/// `HTTP 403` with a Google quota body (`quotaExceeded`,
/// `rateLimitExceeded`, …); zad 0.9.0 promotes those 403s to the
/// same on-disk deadline as 429s so the same gate applies. A single
/// shared on-disk deadline (`~/.zad/state/<service>/rate_limit.json`)
/// is what lets sibling processes coordinate; this flag governs how
/// each one reacts when the deadline is in the future.
pub const SPOTIFAI_WAIT_ENV: &str = "SPOTIFAI_WAIT";

/// Resolve the active wait-mode from the CLI flag, the env var, and
/// a caller-supplied default.
///
/// CLI takes precedence when explicitly set. An unset CLI flag falls
/// back to [`SPOTIFAI_WAIT_ENV`] (`1`/`true`/`yes`/`on` → wait;
/// `0`/`false`/`no`/`off` → fail-fast). When neither is set the
/// `default_wait` argument decides — interactive surfaces pass `true`
/// so multi-agent sessions coordinate; one-shot commands pass `false`
/// so user-driven invocations surface 429s instead of sleeping
/// silently. Sub-agents that spawn `spotifai api` inherit the env var
/// `spotifai ask` / `spotifai playlist` set, so the whole agent tree
/// shares one switch.
pub fn wait_mode_with_default(cli_flag: Option<bool>, default_wait: bool) -> bool {
    if let Some(b) = cli_flag {
        return b;
    }
    match std::env::var(SPOTIFAI_WAIT_ENV) {
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" | "" => false,
            _ => default_wait,
        },
        Err(_) => default_wait,
    }
}

/// Convenience wrapper around [`wait_mode_with_default`] for callers
/// that want the historical "fail-fast by default" behaviour.
pub fn wait_mode(cli_flag: Option<bool>) -> bool {
    wait_mode_with_default(cli_flag, false)
}

/// zad's per-service slug used as the directory under
/// `~/.zad/state/<service>/` for the shared rate-limit deadline file.
/// Kept independent of [`Provider::as_str`] so renames stay localized.
pub fn rate_limit_service(provider: Provider) -> &'static str {
    match provider {
        Provider::Spotify => "spotify",
        Provider::YouTubeMusic => "ymusic",
    }
}

/// Consult zad's shared rate-limit state before issuing a zad call.
///
/// With `wait = true` and a still-active rate-limit deadline
/// persisted by a sibling process, this sleeps until the deadline
/// (capped at one hour per invocation — for the YouTube Music daily
/// quota, which can be ~23h out, the call returns
/// [`zad::ZadError::RateLimited`] after the capped sleep so the user
/// can decide whether to keep waiting). With `wait = false` it
/// returns an error wrapping [`zad::ZadError::RateLimited`] so the
/// caller fails fast. With no recorded deadline it is a no-op
/// regardless of `wait`. Always safe to call before every zad
/// operation; the typical fast path is one disk stat.
///
/// The deadline file is written by the Spotify client on `HTTP 429`
/// and by the YouTube Music client on either `HTTP 429` or `HTTP 403`
/// with a Google quota body — see zad 0.9.0's
/// [`zad::google_quota`](https://docs.rs/zad/0.9.0/zad/google_quota/index.html)
/// for the 403 classification rules.
pub async fn precall_check(provider: Provider, wait: bool) -> Result<()> {
    let service = rate_limit_service(provider);
    rate_limit::precall_check(service, wait)
        .await
        .map_err(|e| anyhow!("{e}"))
}

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
    /// Optional email captured at auth time (Spotify `/me`). ymusic
    /// leaves this empty — the TVHTML5 device flow does not grant
    /// the OpenID Connect scopes needed for Google's userinfo
    /// endpoint.
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
/// the typed facade does not yet expose in zad 0.8.0
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

/// Convert a [`zad::ZadError`] into an [`anyhow::Error`], rewriting
/// well-known failure modes with a concrete remediation. Today this
/// only catches Google's `ACCESS_TOKEN_SCOPE_INSUFFICIENT` — the error
/// the YouTube Data API returns when the access token's granted scopes
/// don't include `youtube` or `youtube.readonly`. The raw zad message
/// is preserved as the underlying cause so debug builds keep the full
/// JSON body; the user-facing surface gets a short, actionable line.
pub fn map_zad(e: zad::ZadError) -> anyhow::Error {
    let raw = format!("{e}");
    if is_youtube_scope_insufficient(&raw) {
        return anyhow!(
            "YouTube rejected the call with `ACCESS_TOKEN_SCOPE_INSUFFICIENT`. \
             Re-run `spotifai auth --provider ymusic` and on Google's consent \
             screen tick *all* checkboxes — under \"granular permissions\" the \
             YouTube box is separate from the basic-profile box, and only \
             ticking the latter produces a refresh token that cannot call the \
             YouTube Data API. Underlying error: {raw}"
        );
    }
    anyhow!("{raw}")
}

fn is_youtube_scope_insufficient(msg: &str) -> bool {
    msg.contains("ACCESS_TOKEN_SCOPE_INSUFFICIENT")
        || (msg.contains("403")
            && msg.contains("insufficientPermissions")
            && msg.contains("youtube"))
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
    // InnerTube uses Google's TVHTML5 client with constants compiled
    // into zad, so the refresh token is the only per-user secret.
    // `client_id` / `client_secret` are passed as empty strings;
    // `YmusicHttp` ignores them.
    let refresh_token =
        load_keychain_or_hint("ymusic", "refresh", "spotifai auth --provider ymusic")?;
    let store = KeychainRefreshStore::new(secrets::account("ymusic", "refresh", Scope::Global));
    Ok(YmusicCredentials {
        client_id: String::new(),
        client_secret: String::new(),
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
