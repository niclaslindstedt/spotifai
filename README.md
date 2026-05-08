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
# Install the pinned zad binary into ~/.spotifai/bin and write a default
# read-only permissions file at ~/.spotifai/permissions.toml.
spotifai install

# Authenticate with Spotify via zad (opens browser for OAuth).
~/.spotifai/bin/zad service create spotify
~/.spotifai/bin/zad service enable spotify

# Ask a natural-language question about your library — the agent talks
# to Spotify only through `spotifai api …` and self-restricts to the
# verbs in ~/.spotifai/permissions.toml.
spotifai ask "What are my most recently added albums?"
```

## Usage

```
spotifai <command> [options]

Commands:
  install         Install pinned zad binary into ~/.spotifai/bin and
                  scaffold the read-only permissions file
  api <args…>     Forward to `zad spotify …`
  ask [query…]    Start an interactive zag session about your Spotify
                  library, with the local permissions file injected
  help            Print help for a command

Options:
      --version           Print version
```

Run `spotifai help <command>` or see [`man/main.md`](man/main.md) for full flag reference.

## Configuration

| File | Purpose |
|---|---|
| `~/.spotifai/bin/zad` | Pinned zad binary, written by `spotifai install`. |
| `~/.spotifai/permissions.toml` | Read-only verb policy injected into the `spotifai ask` system prompt so the agent self-restricts. Defaults to read-only on first install; hand-edit `allowed` / `denied` to widen or narrow. |

Spotify credentials and zad's own permissions live under `~/.zad/` and
are managed by zad directly — see
[zad's docs/configuration.md](https://github.com/niclaslindstedt/zad/blob/main/docs/configuration.md)
for the full reference.

See [docs/configuration.md](docs/configuration.md) for the spotifai-side reference.

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