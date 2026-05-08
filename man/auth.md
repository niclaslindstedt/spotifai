# spotifai auth

> Register OAuth credentials by forwarding to `zad service create <provider>`.

## Synopsis

```
spotifai auth [--provider <slug>] [ARGS]...
```

## Description

`spotifai auth` is a thin shim over `~/.spotifai/bin/zad service create <provider>`. The active provider is selected with `--provider` (default: `spotify`). For Spotify, zad runs an OAuth 2.0 PKCE public-client flow against `accounts.spotify.com`; for YouTube Music (zad ≥ 0.6.0), zad runs an OAuth 2.0 Desktop-app flow against Google. Either way the resulting credentials live in the OS keychain.

Credentials are intentionally registered at zad's **global** scope (`~/.zad/services/<provider>/config.toml`) rather than the cwd-derived project scope, so the same credential applies to every directory `spotifai api …` is invoked from. Pass `--local` explicitly if you want the project-scoped behaviour anyway — the shim does not strip flags.

Before exec'ing zad, spotifai performs the same install/version check as `spotifai install`:

1. If `~/.spotifai/bin/zad` is missing, the release tagged in `.zadrc` is downloaded into place.
2. If the binary exists, its `--version` is compared against the pinned tag (a leading `v` on either side is ignored). On a mismatch, the pinned release is downloaded and the wrong-version binary is replaced.
3. Once the managed path holds the pinned version, zad is invoked there.

Zad's stdout, stderr, and exit code are propagated verbatim — `spotifai auth` returns whatever zad returned.

## Prerequisites

### Spotify

You still need a Spotify developer app on the dashboard:

1. Open `https://developer.spotify.com/dashboard` and click **Create app**.
2. Under **Redirect URIs**, add `http://127.0.0.1` and save. zad's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
3. Copy the **Client ID** (the Client Secret is unused — PKCE is a public-client flow).

`spotifai auth` then takes the Client ID either interactively or via `--client-id`.

### YouTube Music

YouTube Music has no dedicated public API; zad talks to the YouTube Data API v3 with Google OAuth 2.0 Desktop-app credentials. Set up:

1. Open `https://console.cloud.google.com/`, create or pick a project, and enable the **YouTube Data API v3**.
2. Under **Credentials**, click **Create credentials → OAuth client ID** and choose **Desktop app**. Save the **Client ID** + **Client secret**.
3. Add yourself as a **test user** under the OAuth consent screen if the app is still in testing.

`spotifai auth --provider ymusic` then takes the Client ID + Client secret either interactively or via `--client-id` / `--client-secret`, and runs Google's loopback OAuth flow to mint a refresh token.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Provider to register credentials for. One of `spotify`, `ymusic`. |
| `[ARGS]...` | trailing | — | Arguments forwarded as-is to `zad service create <provider>`. Hyphen-prefixed flags are accepted; use `--` to defensively split spotifai's args from zad's. |

## Flags

`spotifai auth` itself only owns `--provider`. Everything else is forwarded to zad. Common pass-through flags include:

| Flag | Description |
|---|---|
| `--client-id <id>` | OAuth client ID (skips the interactive prompt). |
| `--client-secret <secret>` | OAuth client secret. Used by `ymusic`; ignored by Spotify (PKCE has no secret). |
| `--refresh-token <token>` / `--refresh-token-env <VAR>` | Use an out-of-band refresh token instead of running the browser flow (useful for CI). |
| `--no-browser` | Don't auto-open the consent screen; print the URL only. |
| `--non-interactive` | Fail instead of prompting for any missing value. |
| `--force` | Overwrite an existing global credential. |
| `--local` | Store credentials under the project slug instead of globally — opt-in only. |
| `--json` | Emit machine-readable JSON. |

To see zad's full flag list:

```sh
spotifai auth --provider <slug> --help
```

## Environment variables

`spotifai auth` does not read any environment variables of its own. The forwarded zad process inherits the current environment, so any variables zad consults (refresh-token env vars, `ZAD_HOME_OVERRIDE` for tests, etc.) are honoured.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | zad completed the OAuth flow and stored the credential. |
| 1   | Generic spotifai error (download/install failure, missing home directory, version mismatch after re-download). |
| 2   | Usage error parsing `spotifai auth` itself. |
| *N* | Any other code is propagated verbatim from `zad service create <provider>`. |

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
spotifai auth --provider ymusic \
    --client-id   <google-oauth-client-id> \
    --client-secret <google-oauth-client-secret>
```

Headless / CI with a pre-minted refresh token:

```sh
export SPOTIFY_REFRESH_TOKEN=...
spotifai auth \
    --client-id 1234567890abcdef1234567890abcdef \
    --refresh-token-env SPOTIFY_REFRESH_TOKEN \
    --no-browser --non-interactive
```

Force-overwrite an existing global credential:

```sh
spotifai auth --force
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`api.md`](api.md) — `spotifai api` reference (uses the credential registered here)
- [`spotifai install`](main.md#spotifai-install) — the same install/version check, run on its own
- zad's own [`man/spotify.md`](https://github.com/niclaslindstedt/zad/blob/main/man/spotify.md) and [`man/ymusic.md`](https://github.com/niclaslindstedt/zad/blob/main/man/ymusic.md) for the runtime verbs `spotifai api` forwards to
