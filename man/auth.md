# spotifai auth

> Run an in-process OAuth loopback flow and write the resulting tokens into the OS keychain.

## Synopsis

```
spotifai auth [--provider <slug>] [--client-id <id>] [--client-secret <secret>] [--no-browser]
```

## Description

`spotifai auth` runs an OAuth flow for the active provider in-process via `zad::oauth::run_loopback_flow` and persists the resulting credentials in the OS keychain under the `zad` service. After the OAuth flow, spotifai also probes the provider's "self" endpoint and persists the captured user/channel id at `~/.spotifai/<provider>.toml` so `playlists create` and other later calls can reuse it without re-fetching.

For **Spotify**, zad runs an OAuth 2.0 PKCE *public-client* flow against `accounts.spotify.com`. There is no `client_secret`. The redirect lands on a `127.0.0.1:<random-port>` HTTPS loopback listener that spotifai opens for the duration of the flow; TLS is terminated in-process with a fresh self-signed certificate per session. After the flow, spotifai probes `GET /me` to capture the Spotify user id.

For **YouTube Music** (zad ≥ 0.6.0), zad runs an OAuth 2.0 *Desktop-app* flow against Google. The redirect lands on a `127.0.0.1:<random-port>` HTTP loopback listener (Google does not require HTTPS for desktop-app clients). After the flow, spotifai probes `GET /userinfo` and `GET /channels?mine=true` to capture the email, display name, and YouTube channel id.

Credentials are written to the OS keychain at the same accounts the zad library reads:

| Provider | Account | Stored value |
|---|---|---|
| Spotify        | `zad/spotify-client-id:global`     | OAuth client id |
| Spotify        | `zad/spotify-refresh:global`       | Refresh token |
| YouTube Music  | `zad/ymusic-client-id:global`      | OAuth client id |
| YouTube Music  | `zad/ymusic-client-secret:global`  | OAuth client secret |
| YouTube Music  | `zad/ymusic-refresh:global`        | Refresh token |

## Prerequisites

### Spotify

You still need a Spotify developer app on the dashboard:

1. Open `https://developer.spotify.com/dashboard` and click **Create app**.
2. Under **Redirect URIs**, add `https://127.0.0.1` and save. spotifai's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
3. Copy the **Client ID** (the Client Secret is unused — PKCE is a public-client flow).

`spotifai auth` then takes the Client ID either interactively or via `--client-id`.

### YouTube Music

YouTube Music has no dedicated public API; zad talks to the YouTube Data API v3 with Google OAuth 2.0 Desktop-app credentials. Set up:

1. Open `https://console.cloud.google.com/`, create or pick a project, and enable the **YouTube Data API v3**.
2. Under **Credentials**, click **Create credentials → OAuth client ID** and choose **Desktop app**. Save the **Client ID** + **Client secret**.
3. Add yourself as a **test user** under the OAuth consent screen if the app is still in testing.

`spotifai auth --provider ymusic` then takes the Client ID + Client secret either interactively or via `--client-id` / `--client-secret`, and runs Google's loopback OAuth flow to mint a refresh token.

## Arguments

`spotifai auth` takes no positional arguments.

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Provider to register credentials for. One of `spotify`, `ymusic`. |
| `--client-id <id>` | string | — | OAuth client id. Skips the interactive stdin prompt. Also accepts `--client-id=<id>`. |
| `--client-secret <secret>` | string | — | OAuth client secret. Required for `ymusic`; rejected for Spotify (PKCE has no secret). Also accepts `--client-secret=<secret>`. |
| `--no-browser` | bool | false | Don't auto-open the consent screen — print the auth URL on stderr and wait for the redirect. Useful when the loopback listener is reachable from another machine over SSH port-forwarding. |

Unknown flags are rejected with a usage error so the OAuth shape stays predictable.

## Environment variables

`spotifai auth` reads no environment variables of its own. The OS keychain backend (`secret-service` on Linux, Keychain on macOS, Credential Manager on Windows) is selected by the runtime in the usual platform-default way.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | OAuth flow completed and credentials were written to the keychain. The post-flow `/me` (Spotify) or `userinfo` + `channels?mine=true` (YouTube Music) probe may still emit a warning, but does not fail the command. |
| 1 | Generic error: OAuth flow failed (invalid client id/secret, redirect mismatch, user cancellation, network failure), refresh token missing from the response, keychain write failure, or runtime build failure. |
| 2 | Usage error: unknown flag, missing flag value, or `--client-secret` passed for Spotify. |

## Examples

Run the interactive Spotify browser flow (default):

```sh
spotifai auth
```

Skip the prompt by passing the Client ID up front:

```sh
spotifai auth --client-id 1234567890abcdef1234567890abcdef
```

Authenticate against YouTube Music with credentials supplied non-interactively:

```sh
spotifai auth --provider ymusic \
    --client-id   <google-oauth-client-id> \
    --client-secret <google-oauth-client-secret>
```

Print the auth URL but don't open the browser (e.g. when running over SSH):

```sh
spotifai auth --no-browser
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`api.md`](api.md) — `spotifai api` reference (uses the credential registered here)
- [`install.md`](install.md) — must run before `auth` to bootstrap the trust store
