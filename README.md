# spotifai

A Rust CLI for managing your Spotify library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify integration).

[![CI](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml/badge.svg)](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Why?

- Ask about your library and playlists in plain language — no memorising API endpoints
- Create, rename, and delete playlists from the terminal in a single command
- Remove or reorder tracks without opening a browser
- Zero duplicated agent or Spotify integration code — delegates entirely to zag and zad
- Composable with shell scripts and other CLI tools for automation


## Prerequisites

- Rust stable (≥ 1.78) and Cargo — install via [rustup](https://rustup.rs/)
- A Spotify developer app — create one at [developer.spotify.com/dashboard](https://developer.spotify.com/dashboard) and note your **Client ID** and **Client Secret**
- The redirect URI `http://localhost:8888/callback` added to the app's allowed redirect URIs

## Install

```sh
cargo install spotifai
```

Or build from source:

```sh
git clone https://github.com/niclaslindstedt/spotifai.git
cd spotifai
make build
```

## Quick start

```sh
# Authenticate (opens browser for Spotify OAuth)
spotifai auth

# Ask a natural-language question about your library
spotifai ask "What are my most recently added albums?"

# Create a playlist
spotifai ask "Create a playlist called Morning Run with upbeat tracks"
```

## Usage

```
spotifai <command> [options]

Commands:
  install       Download and install the pinned zad binary into ~/.spotifai/bin
  auth          Authenticate with Spotify (OAuth flow)
  ask <query>   Send a natural-language query to the agent
  help          Print help for a command

Options:
  -o, --output <format>   Output format: text (default), json
  -v, --verbose           Print agent reasoning steps
      --version           Print version
```

Run `spotifai help <command>` or see [`man/main.md`](man/main.md) for full flag reference.

## Configuration

spotifai reads configuration from environment variables and an optional config file at `~/.config/spotifai/config.toml`.

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `client_id` | `SPOTIFY_CLIENT_ID` | — | Spotify app client ID (required) |
| `client_secret` | `SPOTIFY_CLIENT_SECRET` | — | Spotify app client secret (required) |
| `redirect_uri` | `SPOTIFY_REDIRECT_URI` | `http://localhost:8888/callback` | OAuth redirect URI |
| `model` | `SPOTIFAI_MODEL` | provider default | LLM model passed to zag |

See [docs/configuration.md](docs/configuration.md) for the full reference.

## Examples

See [`examples/`](examples/) for runnable shell script demos.

## Troubleshooting

_Common failure modes and fixes._

- **`auth` hangs or fails** — confirm `redirect_uri` in your Spotify app dashboard matches the configured value exactly.
- **`SPOTIFY_CLIENT_ID` not set** — export it or add it to `~/.config/spotifai/config.toml`.
- **Agent gives wrong results** — use `-v` to inspect reasoning steps and refine your query.

## Documentation

- [Getting started](docs/getting-started.md)
- [Configuration](docs/configuration.md)
- [Architecture](docs/architecture.md)
- [Troubleshooting](docs/troubleshooting.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under [MIT](LICENSE).