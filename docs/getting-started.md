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

## Set up the local toolchain

`spotifai install` walks a four-step guided setup:

```sh
spotifai install
```

It will, in order:

1. Download the pinned zad binary into `~/.spotifai/bin/zad`.
2. Run `zad signing init` to mint a local Ed25519 signing key in your OS keychain and create the per-machine trust store at `~/.zad/signing/trusted.toml`.
3. Write a default read-only `~/.spotifai/permissions.toml` (allows `search`, `playlists list/show`, `library {tracks,albums} list`; denies every mutating verb).
4. Sign that permissions file with `zad spotify permissions sign --local` so zad's load-time trust check accepts it on the first `spotifai api …` call.

Re-run `spotifai install` whenever you edit `~/.spotifai/permissions.toml` — the signing step runs unconditionally and resigns the file in place.

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

```sh
# Ask anything about your library
spotifai ask "What playlists do I have?"

# Create a playlist
spotifai ask "Create a playlist called Focus Music"

# Add tracks
spotifai ask "Add three lo-fi instrumental tracks to Focus Music"

# Get JSON output for scripting
spotifai ask "List my saved albums" --output json | jq '.[].name'
```

## Next steps

- [Configuration reference](configuration.md)
- [Architecture overview](architecture.md)
- [Troubleshooting](troubleshooting.md)
