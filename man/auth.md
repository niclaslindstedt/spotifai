# spotifai auth

> Register Spotify OAuth credentials by forwarding to `zad service create spotify`.

## Synopsis

```
spotifai auth [ARGS]...
```

## Description

`spotifai auth` is a thin shim over `~/.spotifai/bin/zad service create spotify`. It runs zad's OAuth 2.0 PKCE flow against Spotify, captures the authorization-code redirect on `http://127.0.0.1:<port>`, exchanges it for a refresh token, and stores the resulting `client_id` + refresh token in the OS keychain.

Spotify hands out **one developer app per user**, so the credential is intentionally registered at zad's **global** scope (`~/.zad/services/spotify/config.toml`) rather than the cwd-derived project scope. The same credential then applies to every directory `spotifai api …` is invoked from. Pass `--local` explicitly if you want the project-scoped behaviour anyway — the shim does not strip flags.

Before exec'ing zad, spotifai performs the same install/version check as `spotifai install`:

1. If `~/.spotifai/bin/zad` is missing, the release tagged in `.zadrc` is downloaded into place.
2. If the binary exists, its `--version` is compared against the pinned tag (a leading `v` on either side is ignored). On a mismatch, the pinned release is downloaded and the wrong-version binary is replaced.
3. Once the managed path holds the pinned version, zad is invoked there.

Zad's stdout, stderr, and exit code are propagated verbatim — `spotifai auth` returns whatever zad returned.

## Prerequisites

You still need a Spotify developer app on the dashboard:

1. Open `https://developer.spotify.com/dashboard` and click **Create app**.
2. Under **Redirect URIs**, add `http://127.0.0.1` and save. zad's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
3. Copy the **Client ID** (the Client Secret is unused — PKCE is a public-client flow).

`spotifai auth` then takes the Client ID either interactively or via `--client-id`.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `[ARGS]...` | trailing | — | Arguments forwarded as-is to `zad service create spotify`. Hyphen-prefixed flags are accepted; use `--` to defensively split spotifai's args from zad's. |

## Flags

`spotifai auth` itself takes no flags. Anything that looks like a flag after the `auth` keyword is forwarded to zad. Common pass-through flags include:

| Flag | Description |
|---|---|
| `--client-id <id>` | Spotify application Client ID (skips the interactive prompt). |
| `--refresh-token <token>` / `--refresh-token-env <VAR>` | Use an out-of-band refresh token instead of running the browser flow (useful for CI). |
| `--no-browser` | Don't auto-open Spotify's consent screen; print the URL only. |
| `--non-interactive` | Fail instead of prompting for any missing value. |
| `--force` | Overwrite an existing global credential. |
| `--local` | Store credentials under the project slug instead of globally — opt-in only. |
| `--json` | Emit machine-readable JSON. |

To see zad's full flag list:

```sh
spotifai auth --help
```

## Environment variables

`spotifai auth` does not read any environment variables of its own. The forwarded zad process inherits the current environment, so any variables zad consults (refresh-token env vars, `ZAD_HOME_OVERRIDE` for tests, etc.) are honoured.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | zad completed the OAuth flow and stored the credential. |
| 1   | Generic spotifai error (download/install failure, missing home directory, version mismatch after re-download). |
| 2   | Usage error parsing `spotifai auth` itself. |
| *N* | Any other code is propagated verbatim from `zad service create spotify`. |

## Examples

Run the interactive browser flow (default):

```sh
spotifai auth
```

Skip the prompt by passing the Client ID up front:

```sh
spotifai auth --client-id 1234567890abcdef1234567890abcdef
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
- zad's own [`man/spotify.md`](https://github.com/niclaslindstedt/zad/blob/main/man/spotify.md) for the runtime verbs `spotifai api` forwards to
