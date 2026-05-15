# spotifai auth

> Run an in-process OAuth flow and write the resulting tokens into the OS keychain.

## Synopsis

```
spotifai auth [--provider <slug>] [--client-id <id>] [--no-browser]
```

## Description

`spotifai auth` runs an OAuth flow for the active provider in-process and persists the resulting credentials in the OS keychain under the `zad` service. After the OAuth flow, spotifai also probes the provider's "self" endpoint and persists the captured user/channel id at `~/.spotifai/<provider>.toml` so `playlists create` and other later calls can reuse it without re-fetching.

For **Spotify**, zad runs an OAuth 2.0 PKCE *public-client* flow against `accounts.spotify.com` via `zad::oauth::run_loopback_flow`. There is no `client_secret`. The redirect lands on a `127.0.0.1:<random-port>` HTTPS loopback listener that spotifai opens for the duration of the flow; TLS is terminated in-process with a fresh self-signed certificate per session. After the flow, spotifai probes `GET /me` to capture the Spotify user id.

For **YouTube Music**, zad runs Google's OAuth 2.0 **device flow** (RFC 8628) against the shared TVHTML5 client via `zad::service::ymusic::oauth_device::run_device_flow`. spotifai prints a short URL and a 9-character user code; you visit the URL in any browser (it does not have to be on this machine), sign in to the YouTube account whose library you want to use, and approve. After the flow, spotifai probes `userinfo` and `my_channel` to capture the email, display name, and YouTube channel id. There is no per-user OAuth client to register — the TVHTML5 `client_id` / `client_secret` ship with zad.

Credentials are written to the OS keychain at the same accounts the zad library reads:

| Provider | Account | Stored value |
|---|---|---|
| Spotify        | `zad/spotify-client-id:global`     | OAuth client id |
| Spotify        | `zad/spotify-refresh:global`       | Refresh token |
| YouTube Music  | `zad/ymusic-refresh:global`        | Refresh token |

## Prerequisites

### Spotify

You still need a Spotify developer app on the dashboard:

1. Open `https://developer.spotify.com/dashboard` and click **Create app**.
2. Under **Redirect URIs**, add `https://127.0.0.1` and save. spotifai's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
3. Copy the **Client ID** (the Client Secret is unused — PKCE is a public-client flow).

`spotifai auth` then takes the Client ID either interactively or via `--client-id`.

### YouTube Music

There is no developer-app step. zad's runtime client talks to YouTube Music's internal InnerTube backend at `music.youtube.com/youtubei/v1` and authenticates via Google's OAuth 2.0 device flow against the shared TVHTML5 client; the `client_id` / `client_secret` ship inside zad. Just run `spotifai auth --provider ymusic`, follow the printed URL + code on any browser, and approve.

## Arguments

`spotifai auth` takes no positional arguments.

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Provider to register credentials for. One of `spotify`, `ymusic`. |
| `--client-id <id>` | string | — | Spotify only — OAuth client id. Skips the interactive stdin prompt. Also accepts `--client-id=<id>`. Ignored on `ymusic` (the TVHTML5 client is fixed). |
| `--client-secret <secret>` | string | — | Rejected for Spotify (PKCE has no secret); ignored on `ymusic` (the TVHTML5 client is fixed). Retained for source-compat with older command lines. |
| `--no-browser` | bool | false | Spotify: don't auto-open the consent screen — print the auth URL on stderr and wait for the redirect. Useful when the loopback listener is reachable from another machine over SSH port-forwarding. YouTube Music: no-op — the device-flow URL is always printed for the user to open manually. |

Unknown flags are rejected with a usage error so the OAuth shape stays predictable.

## Environment variables

`spotifai auth` reads no environment variables of its own. The OS keychain backend (`secret-service` on Linux, Keychain on macOS, Credential Manager on Windows) is selected by the runtime in the usual platform-default way.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | OAuth flow completed and credentials were written to the keychain. The post-flow `/me` (Spotify) or `userinfo` + `my_channel` (YouTube Music) probe may still emit a warning, but does not fail the command. |
| 1 | Generic error: OAuth flow failed (invalid client id, redirect mismatch, user cancellation, network failure, device-code expiry), refresh token missing from the response, keychain write failure, or runtime build failure. |
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

Authenticate against YouTube Music:

```sh
spotifai auth --provider ymusic
# spotifai prints a verification URL and a 9-character code;
# open the URL on any browser, type the code, sign in, approve.
```

Print the Spotify auth URL but don't open the browser (e.g. when running over SSH):

```sh
spotifai auth --no-browser
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`api.md`](api.md) — `spotifai api` reference (uses the credential registered here)
- [`install.md`](install.md) — must run before `auth` to bootstrap the trust store
