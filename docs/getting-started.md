# Getting started with spotifai

> A Rust CLI for managing your Spotify library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify integration).

## Install

```sh
cargo install spotifai
```

Confirm the binary is available:

```sh
spotifai --version
```

## Two agent surfaces

spotifai exposes the agent through two commands, each with its own permissions profile:

- **`spotifai ask`** is read-only. Use it for questions about your library.
- **`spotifai playlist`** can additionally create a new playlist, add tracks to it, and rename it. Use it when you want the agent to build something for you.

Both commands run `spotifai api …` under the hood; that shim looks up the matching profile file and pins it on zad. Direct `spotifai api …` invocations from a shell are intentionally rejected — call zad directly via `~/.spotifai/bin/zad spotify …` if you need that.

## Set up the local toolchain

`spotifai install` walks a four-step guided setup:

```sh
spotifai install
```

It will, in order:

1. Download the pinned zad binary into `~/.spotifai/bin/zad`.
2. Run `zad signing init` to mint a local Ed25519 signing key in your OS keychain and create the per-machine trust store at `~/.zad/signing/trusted.toml`.
3. Scaffold the per-profile permissions files at `~/.spotifai/permissions/ask.toml` (read-only) and `~/.spotifai/permissions/playlist.toml` (read + create/add/rename). Existing files are left alone.
4. Sign each profile file with `zad spotify permissions sign --local` so zad's load-time trust check accepts them on the first `spotifai api …` call.

Re-run `spotifai install` whenever you edit a profile file — the signing step runs unconditionally and resigns every file in place.

## Create a Spotify developer app

Spotify hands out **one developer app per user**, so you do this once:

1. Go to [developer.spotify.com/dashboard](https://developer.spotify.com/dashboard) and log in.
2. Click **Create app**, give it any name (e.g. "spotifai-local").
3. Under **Redirect URIs**, add `http://127.0.0.1` and save. zad's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
4. Copy the **Client ID** from the app settings. (The Client Secret is unused — zad uses an OAuth 2.0 PKCE *public-client* flow, which doesn't accept one.)

## First run

Authenticate — this forwards to `zad service create spotify` at zad's **global** scope and opens a browser for the Spotify consent screen:

```sh
spotifai auth
```

After granting access, zad captures the redirect on `http://127.0.0.1:<random-port>`, exchanges the authorization code for a refresh token, and stores `client_id` + the refresh token in your OS keychain. Configuration lands at `~/.zad/services/spotify/config.toml`. You only need to do this once — refresh tokens are minted on every later `spotifai api …` call automatically.

If you'd rather skip the interactive prompt, pass the Client ID up front:

```sh
spotifai auth --client-id <your-client-id>
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
# Ask anything about your library
spotifai ask "What playlists do I have?"

# Get JSON output for scripting
spotifai ask "List my saved albums" --output json | jq '.[].name'
```

Building a new playlist goes through `spotifai playlist`:

```sh
# Hand the agent a brief and let it pick tracks
spotifai playlist "a 30-minute focus playlist with no vocals"

# Or open the session empty and chat
spotifai playlist
```

The agent in `spotifai playlist` can search the catalogue, look at your existing playlists for inspiration, create one new playlist, add tracks to it, and rename it before it commits. It cannot delete playlists or remove tracks — those verbs stay denied even in this profile.

## Next steps

- [Configuration reference](configuration.md)
- [Architecture overview](architecture.md)
- [Troubleshooting](troubleshooting.md)
