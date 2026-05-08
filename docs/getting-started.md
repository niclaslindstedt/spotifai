# Getting started with spotifai

> A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).

## Install

```sh
cargo install spotifai
```

Confirm the binary is available:

```sh
spotifai --version
```

## Two agent surfaces, two providers

spotifai exposes the agent through two commands, each with its own permissions profile:

- **`spotifai ask`** is read-only. Use it for questions about your library.
- **`spotifai playlist`** can additionally create a new playlist, add tracks/videos to it, and rename it. Use it when you want the agent to build something for you.

Both commands run `spotifai api …` under the hood; that shim looks up the matching profile file and pins it on zad. Direct `spotifai api …` invocations from a shell are intentionally rejected — call zad directly via `~/.spotifai/bin/zad <provider> …` if you need that.

Both commands take `--provider <slug>` (default: `spotify`). Today the supported providers are:

| Slug | Display name | Notes |
|---|---|---|
| `spotify` (default) | Spotify       | OAuth 2.0 PKCE, one developer app per user. |
| `ymusic`            | YouTube Music | Google OAuth 2.0 Desktop-app credentials, talks to YouTube Data API v3. Requires zad ≥ 0.6.0. |

## Set up the local toolchain

`spotifai install` walks a four-step guided setup:

```sh
spotifai install
```

It will, in order:

1. Download the pinned zad binary into `~/.spotifai/bin/zad`.
2. Run `zad signing init` to mint a local Ed25519 signing key in your OS keychain and create the per-machine trust store at `~/.zad/signing/trusted.toml`.
3. Scaffold the per-(provider, profile) permissions files at `~/.spotifai/permissions/<provider>/ask.toml` (read-only) and `~/.spotifai/permissions/<provider>/playlist.toml` (read + create/add/rename), for every supported provider. Existing files are left alone.
4. Sign each profile file with `zad <provider> permissions sign --local` so zad's load-time trust check accepts them on the first `spotifai api …` call.

Re-run `spotifai install` whenever you edit a profile file — the signing step runs unconditionally and resigns every file in place.

## Create a developer app

### Spotify

Spotify hands out **one developer app per user**, so you do this once:

1. Go to [developer.spotify.com/dashboard](https://developer.spotify.com/dashboard) and log in.
2. Click **Create app**, give it any name (e.g. "spotifai-local").
3. Under **Redirect URIs**, add `http://127.0.0.1` and save. zad's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
4. Copy the **Client ID** from the app settings. (The Client Secret is unused — zad uses an OAuth 2.0 PKCE *public-client* flow.)

### YouTube Music

YouTube Music has no dedicated public API; zad talks to YouTube Data API v3 with Google OAuth credentials.

1. Go to [console.cloud.google.com](https://console.cloud.google.com/), create or pick a project, and enable **YouTube Data API v3** under "APIs & Services".
2. Under **Credentials**, click **Create credentials → OAuth client ID** and pick **Desktop app**. Save the **Client ID** + **Client secret**.
3. While the OAuth consent screen is in **Testing**, add yourself as a test user.

## First run

Authenticate. For Spotify, this opens a browser for the consent screen:

```sh
spotifai auth
```

For YouTube Music:

```sh
spotifai auth --provider ymusic
```

After granting access, zad captures the redirect on `http://127.0.0.1:<random-port>`, exchanges the authorization code for a refresh token, and stores it in your OS keychain. Configuration lands at `~/.zad/services/<provider>/config.toml`. You only need to do this once per provider — refresh tokens are minted on every later `spotifai api …` call automatically.

If you'd rather skip the interactive prompt, pass the credentials up front:

```sh
spotifai auth --client-id <your-client-id>
spotifai auth --provider ymusic --client-id <id> --client-secret <secret>
```

For headless / CI setups, supply a pre-minted refresh token:

```sh
export SPOTIFY_REFRESH_TOKEN=...
spotifai auth \
    --client-id <your-client-id> \
    --refresh-token-env SPOTIFY_REFRESH_TOKEN \
    --no-browser --non-interactive
```

## Your first query

Read-only questions go through `spotifai ask`:

```sh
# Default: Spotify
spotifai ask "What playlists do I have?"

# YouTube Music
spotifai ask --provider ymusic "What playlists do I have?"

# Get JSON output for scripting
spotifai ask "List my saved albums" --output json | jq '.[].name'
```

Building a new playlist goes through `spotifai playlist`:

```sh
# Default: Spotify
spotifai playlist "a 30-minute focus playlist with no vocals"

# YouTube Music
spotifai playlist --provider ymusic "a 30-minute focus playlist with no vocals"

# Or open the session empty and chat
spotifai playlist
```

The agent in `spotifai playlist` can search the catalogue, look at your existing playlists for inspiration, create one new playlist, add tracks/videos to it, and rename it before it commits. It cannot delete playlists or remove items — those verbs stay denied even in this profile.

## Next steps

- [Configuration reference](configuration.md)
- [Architecture overview](architecture.md)
- [Troubleshooting](troubleshooting.md)
