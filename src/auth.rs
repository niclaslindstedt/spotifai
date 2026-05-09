//! `spotifai auth` — register OAuth credentials in-process using
//! zad's PKCE/loopback primitives, then write the resulting tokens
//! into the OS keychain at the same accounts the zad library reads
//! (`spotify-client-id:global`, `spotify-refresh:global`,
//! `ymusic-client-id:global`, `ymusic-client-secret:global`,
//! `ymusic-refresh:global`).
//!
//! Spotify uses Spotify's OAuth 2.0 authorization-code flow with
//! PKCE — a public client, no `client_secret`. YouTube Music uses
//! Google's OAuth 2.0 "Desktop app" flow which still issues a
//! confidential `client_secret`. Either way the redirect lands on a
//! `127.0.0.1` loopback listener spotifai opens for the duration of
//! the flow; Spotify additionally requires `https://` so the listener
//! terminates TLS in-process with a fresh self-signed certificate per
//! flow.
//!
//! On success spotifai also probes the provider's "self" endpoint
//! (`GET /me` for Spotify, `GET /userinfo` + `GET /channels?mine=true`
//! for YouTube Music) and persists the authenticated identity to
//! `~/.spotifai/<provider>.toml`. `create_playlist` reads it back
//! later so the agent surface does not have to re-fetch the user id
//! on every call.

use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use zad::oauth::{LoopbackConfig, RedirectScheme, TokenSet, run_loopback_flow};
use zad::secrets::{self, Scope};
use zad::service::spotify::{self as zad_spotify, SpotifyHttp};
use zad::service::ymusic::{self as zad_ymusic, YmusicHttp};

use crate::output;
use crate::providers::Provider;
use crate::zad_client::{self, SelfIdentity};

/// Options understood by `spotifai auth`. Anything outside this
/// small set is rejected so the OAuth shape stays predictable.
#[derive(Debug, Default)]
pub struct AuthOptions {
    /// OAuth client id. If unset, spotifai prompts on stdin.
    pub client_id: Option<String>,
    /// OAuth client secret. Required for YouTube Music; rejected for
    /// Spotify (Spotify is PKCE-only / no client secret).
    pub client_secret: Option<String>,
    /// Open the user's default browser at the auth URL. Defaults to
    /// `true`; `--no-browser` flips it off so headless environments
    /// can copy/paste the URL into another machine.
    pub open_browser: bool,
}

/// Parse the trailing argument vector accepted by
/// `spotifai auth [-- ARGS…]`.
pub fn parse_args(user_args: &[String]) -> Result<AuthOptions> {
    let mut opts = AuthOptions {
        open_browser: true,
        ..AuthOptions::default()
    };
    let mut iter = user_args.iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--client-id" => {
                opts.client_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--client-id needs a value"))?
                        .clone(),
                );
            }
            s if s.starts_with("--client-id=") => {
                opts.client_id = Some(s["--client-id=".len()..].to_string());
            }
            "--client-secret" => {
                opts.client_secret = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--client-secret needs a value"))?
                        .clone(),
                );
            }
            s if s.starts_with("--client-secret=") => {
                opts.client_secret = Some(s["--client-secret=".len()..].to_string());
            }
            "--no-browser" => opts.open_browser = false,
            other => bail!(
                "unsupported flag `{other}`; spotifai auth accepts only \
                 --client-id, --client-secret, --no-browser"
            ),
        }
    }
    Ok(opts)
}

/// Drive the in-process OAuth flow for `provider` and persist the
/// resulting credentials in the OS keychain.
pub fn run(provider: Provider, user_args: &[String]) -> Result<()> {
    let opts = parse_args(user_args)?;
    if matches!(provider, Provider::Spotify) && opts.client_secret.is_some() {
        bail!(
            "Spotify auth uses PKCE only — passing --client-secret is unsupported. \
             Re-run without that flag."
        );
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    rt.block_on(run_async(provider, opts))
}

async fn run_async(provider: Provider, opts: AuthOptions) -> Result<()> {
    output::header(&format!("spotifai auth ({})", provider.display_name()));
    match provider {
        Provider::Spotify => run_spotify(opts).await,
        Provider::YouTubeMusic => run_ymusic(opts).await,
    }
}

async fn run_spotify(opts: AuthOptions) -> Result<()> {
    let client_id = match opts.client_id {
        Some(s) => s,
        None => {
            print_spotify_setup_hint();
            prompt_for("Spotify client_id")?
        }
    };

    // zad's spotify_scopes_for() expects zad-level scopes; we ask
    // for the union so a single set of credentials covers every
    // surface (ask, playlist, export, import).
    let zad_scopes = full_zad_scopes();
    let mut scopes = zad_spotify::spotify_scopes_for(&zad_scopes);
    // Some Spotify scopes are required even for the unscoped probe
    // we run after the loopback flow (e.g. capturing the user id).
    // Spotify's `/me` only needs `user-read-private`; grab it so the
    // probe always works regardless of the requested zad scopes.
    if !scopes.iter().any(|s| s == "user-read-private") {
        scopes.push("user-read-private".into());
    }
    scopes.sort();
    scopes.dedup();

    let cfg = LoopbackConfig {
        service_name: "spotify",
        display_name: "Spotify",
        auth_url: zad_spotify::AUTH_URL.into(),
        token_url: zad_spotify::TOKEN_URL.into(),
        client_id: client_id.clone(),
        client_secret: None,
        scopes,
        extra_auth_params: vec![("show_dialog".into(), "true".into())],
        timeout: Duration::from_secs(120),
        redirect_scheme: RedirectScheme::Https,
    };
    let tokens = run_loopback_flow(&cfg, opts.open_browser)
        .await
        .map_err(|e| anyhow!("Spotify OAuth failed: {e}"))?;
    let refresh = require_refresh(&tokens, "Spotify")?;

    secrets::store(
        &secrets::account("spotify", "client-id", Scope::Global),
        &client_id,
    )
    .map_err(|e| anyhow!("storing Spotify client-id in keychain failed: {e}"))?;
    secrets::store(
        &secrets::account("spotify", "refresh", Scope::Global),
        &refresh,
    )
    .map_err(|e| anyhow!("storing Spotify refresh token in keychain failed: {e}"))?;

    output::status("credentials written to OS keychain");

    // Probe /me to capture the Spotify user id used by
    // create_playlist.
    let probe = SpotifyHttp::unscoped(client_id, refresh);
    match probe.me().await {
        Ok(me) => {
            let identity = SelfIdentity {
                user_id: Some(me.id.clone()),
                channel_id: None,
                display_name: me.display_name,
                email: me.email,
            };
            zad_client::write_self_identity(Provider::Spotify, &identity)?;
            output::status(&format!("authenticated as `{}`", me.id));
        }
        Err(e) => {
            output::warn(&format!(
                "OAuth succeeded but `/me` probe failed: {e}. \
                 `create_playlist` will fail until the probe runs successfully."
            ));
        }
    }
    Ok(())
}

async fn run_ymusic(opts: AuthOptions) -> Result<()> {
    if opts.client_id.is_none() || opts.client_secret.is_none() {
        print_ymusic_setup_hint();
    }
    let client_id = match opts.client_id {
        Some(s) => s,
        None => prompt_for("YouTube Music (Google OAuth) client_id")?,
    };
    let client_secret = match opts.client_secret {
        Some(s) => s,
        None => prompt_for("YouTube Music (Google OAuth) client_secret")?,
    };

    let zad_scopes = full_zad_scopes();
    let scopes = zad_ymusic::youtube_scopes_for(&zad_scopes);

    let cfg = LoopbackConfig {
        service_name: "ymusic",
        display_name: "YouTube Music",
        auth_url: zad_ymusic::AUTH_URL.into(),
        token_url: zad_ymusic::TOKEN_URL.into(),
        client_id: client_id.clone(),
        client_secret: Some(client_secret.clone()),
        scopes,
        // `access_type=offline` + `prompt=consent` together force
        // Google to mint a fresh refresh_token even if the user has
        // already granted the scopes for this client.
        extra_auth_params: vec![
            ("access_type".into(), "offline".into()),
            ("prompt".into(), "consent".into()),
            ("include_granted_scopes".into(), "true".into()),
        ],
        timeout: Duration::from_secs(120),
        redirect_scheme: RedirectScheme::Http,
    };
    let tokens = run_loopback_flow(&cfg, opts.open_browser)
        .await
        .map_err(|e| anyhow!("YouTube Music OAuth failed: {e}"))?;
    let refresh = require_refresh(&tokens, "YouTube Music")?;

    secrets::store(
        &secrets::account("ymusic", "client-id", Scope::Global),
        &client_id,
    )
    .map_err(|e| anyhow!("storing YouTube Music client-id in keychain failed: {e}"))?;
    secrets::store(
        &secrets::account("ymusic", "client-secret", Scope::Global),
        &client_secret,
    )
    .map_err(|e| anyhow!("storing YouTube Music client-secret in keychain failed: {e}"))?;
    secrets::store(
        &secrets::account("ymusic", "refresh", Scope::Global),
        &refresh,
    )
    .map_err(|e| anyhow!("storing YouTube Music refresh token in keychain failed: {e}"))?;

    output::status("credentials written to OS keychain");

    // Capture the channel id and email via userinfo + my_channel.
    let probe = YmusicHttp::unscoped(client_id, client_secret, refresh);
    let mut identity = SelfIdentity::default();
    match probe.userinfo().await {
        Ok(info) => {
            identity.email = info.email;
            identity.display_name = info.name;
        }
        Err(e) => output::warn(&format!("userinfo probe failed: {e}")),
    }
    match probe.my_channel().await {
        Ok(ch) => {
            identity.channel_id = Some(ch.id.clone());
            if identity.display_name.is_none() {
                identity.display_name = ch.snippet.as_ref().and_then(|s| s.title.clone());
            }
            zad_client::write_self_identity(Provider::YouTubeMusic, &identity)?;
            output::status(&format!("authenticated channel `{}`", ch.id));
        }
        Err(e) => {
            zad_client::write_self_identity(Provider::YouTubeMusic, &identity)?;
            output::warn(&format!(
                "OAuth succeeded but `channels?mine=true` probe failed: {e}"
            ));
        }
    }
    Ok(())
}

/// zad-level scopes spotifai always requests at OAuth time. Asking
/// for the full superset means a user goes through the consent
/// screen exactly once even if they later switch profiles.
fn full_zad_scopes() -> Vec<String> {
    let mut out: Vec<String> = [
        "search",
        "playlists.read",
        "playlists.write",
        "library.read",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    out.sort();
    out.dedup();
    out
}

fn require_refresh(tokens: &TokenSet, label: &str) -> Result<String> {
    tokens.refresh_token.clone().ok_or_else(|| {
        anyhow!(
            "{label} OAuth response did not include a refresh_token. \
             Re-run with `--no-browser` and check the consent screen — \
             you may need to revoke the existing grant first."
        )
    })
}

/// First-time setup hint shown before prompting for a Spotify
/// `client_id`. The exact URL and redirect host match what the
/// loopback flow expects, so a user who follows these steps verbatim
/// will pass the OAuth handshake without bouncing through the docs.
fn print_spotify_setup_hint() {
    output::info("");
    output::info("First-time setup — you need a Spotify developer app to get a client_id:");
    output::info("");
    output::info("  1. Open https://developer.spotify.com/dashboard");
    output::info("  2. Click \"Create app\" — name it anything (e.g. \"spotifai-local\")");
    output::info("  3. Under \"Redirect URIs\", add: https://127.0.0.1");
    output::info("     (any port works once the host is registered)");
    output::info("  4. Save, then copy the \"Client ID\" from the app settings");
    output::info("     (the Client Secret is not used — spotifai uses PKCE)");
    output::info("");
}

/// First-time setup hint shown before prompting for YouTube Music
/// (Google OAuth) credentials. Links go straight to the API library
/// and credentials pages so the user does not have to navigate the
/// Google Cloud console themselves.
fn print_ymusic_setup_hint() {
    output::info("");
    output::info("First-time setup — you need Google OAuth credentials with the YouTube Data API:");
    output::info("");
    output::info("  1. Enable the API:");
    output::info("     https://console.cloud.google.com/apis/library/youtube.googleapis.com");
    output::info("  2. Create an OAuth client:");
    output::info("     https://console.cloud.google.com/apis/credentials");
    output::info("     → \"Create credentials\" → \"OAuth client ID\" → application type \"Desktop app\"");
    output::info("  3. While the consent screen is in Testing, add yourself as a test user:");
    output::info("     https://console.cloud.google.com/apis/credentials/consent");
    output::info("  4. Copy the \"Client ID\" and \"Client secret\" from the new credential");
    output::info("");
}

fn prompt_for(label: &str) -> Result<String> {
    use std::io::{BufRead as _, Write as _};
    let stderr = std::io::stderr();
    {
        let mut handle = stderr.lock();
        write!(handle, "{label}: ").context("writing prompt")?;
        handle.flush().context("flushing prompt")?;
    }
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("reading prompt input")?;
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        bail!("empty value entered for `{label}`");
    }
    Ok(trimmed)
}

// `BTreeSet` import kept available for callers that pass scope sets
// straight through; not used in this module today.
#[allow(dead_code)]
fn _scope_set(scopes: &[String]) -> BTreeSet<String> {
    scopes.iter().cloned().collect()
}
