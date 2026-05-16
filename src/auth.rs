//! `spotifai auth` — register OAuth credentials in-process using
//! zad's auth primitives, then write the resulting tokens into the
//! OS keychain at the same accounts the zad library reads
//! (`spotify-client-id:global`, `spotify-refresh:global`,
//! `ymusic-refresh:global`).
//!
//! Spotify uses Spotify's OAuth 2.0 authorization-code flow with
//! PKCE — a public client, no `client_secret`; the redirect lands
//! on an `https://127.0.0.1` loopback listener spotifai opens for
//! the duration of the flow and terminates TLS in-process with a
//! self-signed certificate.
//!
//! YouTube Music uses Google's **OAuth 2.0 device flow** (RFC 8628)
//! against the shared TVHTML5 client. There is no per-user
//! `client_id` / `client_secret` to register — the TVHTML5 pair is
//! compiled into zad. spotifai prints a short URL and a 9-character
//! code, then polls Google until the user finishes approval on any
//! browser.
//!
//! On success spotifai also probes the provider's "self" endpoint
//! (`GET /me` for Spotify, `my_channel` browse for YouTube Music)
//! and persists the authenticated identity to
//! `~/.spotifai/<provider>.toml`. `create_playlist` reads it back
//! later so the agent surface does not have to re-fetch the user id
//! on every call.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use zad::oauth::{
    KeychainRefreshStore, LoopbackConfig, RedirectScheme, TokenSet, run_loopback_flow,
};
use zad::secrets::{self, Scope};
use zad::service::spotify::{self as zad_spotify, SpotifyHttp};
use zad::service::ymusic::YmusicHttp;
use zad::service::ymusic::oauth_device::{DeviceFlowConfig, run_device_flow};

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
    let _scope = output::section(
        &format!("spotifai auth ({})", provider.display_name()),
        "auth",
    );
    match provider {
        Provider::Spotify => run_spotify(opts).await,
        Provider::YouTubeMusic => run_ymusic(opts).await,
    }
}

async fn run_spotify(opts: AuthOptions) -> Result<()> {
    let client_id = match opts.client_id {
        Some(s) => s,
        None => match secrets::load(&secrets::account("spotify", "client-id", Scope::Global))
            .map_err(|e| anyhow!("reading stored Spotify client-id from keychain failed: {e}"))?
        {
            Some(stored) => {
                output::detail(&format!(
                    "using stored client_id ({}…); pass --client-id to override",
                    stored.chars().take(8).collect::<String>()
                ));
                stored
            }
            None => {
                print_spotify_setup_hint();
                output::prompt("Spotify client_id")?
            }
        },
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
    output::action("running OAuth loopback flow (Spotify, PKCE)");
    output::detail(&format!("requested scopes: {}", cfg.scopes.join(" ")));
    let tokens = run_loopback_flow(&cfg, opts.open_browser)
        .await
        .map_err(|e| anyhow!("Spotify OAuth failed: {e}"))?;
    let refresh = require_refresh(&tokens, "Spotify")?;
    if let Some(scope) = tokens.scope.as_deref() {
        output::detail(&format!("granted scopes: {scope}"));
    }

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
    // create_playlist. Spotify's PKCE flow rotates the refresh token
    // on every /api/token call, so the probe's refresh would invalidate
    // the just-stored token unless we hand the client the same
    // KeychainRefreshStore the runtime path uses — that way the
    // rotated value lands in the keychain before `auth` returns.
    let probe = SpotifyHttp::with_store(
        client_id,
        refresh,
        BTreeSet::new(),
        PathBuf::new(),
        Some(Arc::new(KeychainRefreshStore::new(secrets::account(
            "spotify",
            "refresh",
            Scope::Global,
        )))),
    );
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
    if opts.client_id.is_some() || opts.client_secret.is_some() {
        output::warn(
            "--client-id / --client-secret are ignored for YouTube Music — \
             zad authenticates via OAuth 2.0 device flow against Google's \
             shared TVHTML5 client, whose credentials ship with the binary.",
        );
    }
    print_ymusic_setup_hint();

    let cfg = DeviceFlowConfig::default();
    output::action("running OAuth device flow (Google TVHTML5)");
    let _ = opts.open_browser; // device flow shows a URL; opening is per-OS and left to the user
    let tokens = run_device_flow(&cfg, |code| {
        output::newline();
        output::detail(&format!("Visit:  {}", code.verification_url));
        output::detail(&format!("Enter:  {}", code.user_code));
        output::detail(&format!(
            "Waiting up to {}s for approval (polling every {}s)…",
            code.expires_in, code.interval
        ));
    })
    .await
    .map_err(|e| anyhow!("YouTube Music device flow failed: {e}"))?;
    let refresh = require_refresh(&tokens, "YouTube Music")?;
    if let Some(scope) = tokens.scope.as_deref() {
        output::detail(&format!("granted scopes: {scope}"));
        warn_if_youtube_scope_missing(scope);
    }

    // The TVHTML5 client constants live in zad; the only per-user
    // ymusic secret is the refresh token. Best-effort delete on the
    // unused slots keeps the keychain free of stale values.
    let _ = secrets::delete(&secrets::account("ymusic", "client-id", Scope::Global));
    let _ = secrets::delete(&secrets::account("ymusic", "client-secret", Scope::Global));
    secrets::store(
        &secrets::account("ymusic", "refresh", Scope::Global),
        &refresh,
    )
    .map_err(|e| anyhow!("storing YouTube Music refresh token in keychain failed: {e}"))?;

    output::status("credentials written to OS keychain");

    // Capture the channel id via my_channel. The TVHTML5 device
    // flow only grants the `youtube` scope; OpenID Connect's
    // userinfo endpoint rejects those tokens (HTTP 401), so identity
    // comes from InnerTube alone. Google rarely rotates refresh
    // tokens but the probe's `access_token` path persists rotations
    // only if a store is wired up — match the Spotify branch.
    let probe = YmusicHttp::with_store(
        String::new(),
        String::new(),
        refresh,
        BTreeSet::new(),
        PathBuf::new(),
        Some(Arc::new(KeychainRefreshStore::new(secrets::account(
            "ymusic",
            "refresh",
            Scope::Global,
        )))),
    );
    let mut identity = SelfIdentity::default();
    match probe.my_channel().await {
        Ok(ch) => {
            identity.channel_id = Some(ch.id.clone());
            identity.display_name = ch.snippet.as_ref().and_then(|s| s.title.clone());
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

/// Inspect the space-separated `scope` claim Google returned and
/// surface a clear warning when neither the read+write nor the
/// read-only YouTube scope is present. Without one of these the
/// `/playlists?mine=true` calls used by `export` and `import` will
/// fail with `ACCESS_TOKEN_SCOPE_INSUFFICIENT` later.
///
/// Google rolled out granular permissions in 2024: even when spotifai
/// requests `https://www.googleapis.com/auth/youtube`, the user can
/// uncheck the "YouTube" box on the consent screen while leaving the
/// basic-profile box ticked. The OAuth flow then succeeds (Google
/// happily issues a token with `openid email` only) but every
/// subsequent YouTube API call fails. Catching this at auth time
/// keeps the diagnosis attached to the user action that caused it.
fn warn_if_youtube_scope_missing(scope: &str) {
    const WRITE: &str = "https://www.googleapis.com/auth/youtube";
    const READ: &str = "https://www.googleapis.com/auth/youtube.readonly";
    let granted: Vec<&str> = scope.split_ascii_whitespace().collect();
    let has_write = granted.contains(&WRITE);
    let has_read = granted.contains(&READ);
    if !has_write && !has_read {
        output::warn(
            "Google's consent screen did not grant any YouTube scope. \
             `spotifai export` / `import` / `playlist` will fail with \
             ACCESS_TOKEN_SCOPE_INSUFFICIENT until you re-run \
             `spotifai auth --provider ymusic` and tick the \"YouTube\" \
             checkbox on Google's consent screen (it's separate from the \
             basic-profile checkbox under granular permissions).",
        );
    } else if !has_write {
        output::warn(
            "Google's consent screen granted only `youtube.readonly`; \
             write surfaces (`spotifai import`, `playlist create/add`) \
             will fail until you re-run `spotifai auth --provider ymusic` \
             and grant the full \"Manage your YouTube account\" permission.",
        );
    }
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
    output::newline();
    output::hint("First-time setup — you need a Spotify developer app to get a client_id:");
    output::detail("1. Open https://developer.spotify.com/dashboard");
    output::detail("2. Click \"Create app\" — name it anything (e.g. \"spotifai-local\")");
    output::detail("3. Under \"Redirect URIs\", add: https://127.0.0.1");
    output::detail("   (any port works once the host is registered)");
    output::detail("4. Save, then copy the \"Client ID\" from the app settings");
    output::detail("   (the Client Secret is not used — spotifai uses PKCE)");
    output::newline();
}

/// First-time setup hint shown before launching the YouTube Music
/// device-flow handshake. There is no per-user OAuth client to
/// register — zad ships the shared TVHTML5 credentials — so this
/// is a brief explanation rather than the multi-step Google Cloud
/// walk-through the Data API flow used to require.
fn print_ymusic_setup_hint() {
    output::newline();
    output::hint("YouTube Music uses Google's OAuth 2.0 device flow.");
    output::detail("1. spotifai prints a short URL and a 9-character code.");
    output::detail("2. Visit the URL in any browser (it does not have to be on this machine).");
    output::detail("3. Sign in to the YouTube account whose library you want to use.");
    output::detail("4. Type the code, approve, and come back here.");
    output::newline();
}

// `BTreeSet` import kept available for callers that pass scope sets
// straight through; not used in this module today.
#[allow(dead_code)]
fn _scope_set(scopes: &[String]) -> BTreeSet<String> {
    scopes.iter().cloned().collect()
}
