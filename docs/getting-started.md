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

## Create a Spotify developer app

1. Go to [developer.spotify.com/dashboard](https://developer.spotify.com/dashboard) and log in.
2. Click **Create app**, give it any name (e.g. "spotifai-local").
3. Under **Redirect URIs**, add `http://localhost:8888/callback` and save.
4. Copy the **Client ID** and **Client Secret** from the app settings.

## Configure credentials

Export them as environment variables (or add to `~/.config/spotifai/config.toml`):

```sh
export SPOTIFY_CLIENT_ID=your_client_id
export SPOTIFY_CLIENT_SECRET=your_client_secret
```

## First run

Authenticate — this opens a browser for the Spotify OAuth consent screen:

```sh
spotifai auth
```

After granting access the browser redirects to localhost and spotifai stores a token in `~/.config/spotifai/token.json`. You only need to do this once (tokens are refreshed automatically).

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
