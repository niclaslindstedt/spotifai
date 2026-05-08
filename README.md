# spotifai

A Rust CLI for managing your Spotify library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify integration).

[![CI](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml/badge.svg)](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Why?

- Ask about your library and playlists in plain language — no memorising API endpoints
- Build new playlists conversationally with `spotifai playlist`, while `spotifai ask` stays read-only
- Per-command permission profiles, signed at install time, so the agent can only use the verbs each surface needs
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
# Walk the four-step guided setup: install the pinned zad binary into
# ~/.spotifai/bin, bootstrap the local Ed25519 signing key in your OS
# keychain, scaffold the per-profile permissions files under
# ~/.spotifai/permissions/ (ask.toml is read-only; playlist.toml adds
# create/add/rename), and sign each one. Re-run after editing any
# profile file to resign.
spotifai install

# Authenticate with Spotify (opens browser for the OAuth 2.0 PKCE flow).
# Forwards to `zad service create spotify` at zad's global scope, so the
# credential applies to every directory you later run `spotifai api …` from.
spotifai auth

# Ask a natural-language question about your library — `ask` is
# read-only and self-restricts to the verbs in
# ~/.spotifai/permissions/ask.toml.
spotifai ask "What are my most recently added albums?"

# Build a new playlist conversationally — `playlist` loads
# ~/.spotifai/permissions/playlist.toml so the agent can create one new
# playlist, add tracks to it, and rename it. Destructive verbs stay denied.
spotifai playlist "a 30-minute focus playlist with no vocals"
```

## Usage

```
spotifai <command> [options]

Commands:
  install            Guided setup: install pinned zad binary, bootstrap
                     local signing key, scaffold and sign every per-profile
                     permissions file under ~/.spotifai/permissions/
  auth [args…]       Forward to `zad service create spotify` (global scope)
                     to register a Spotify Client ID and run OAuth 2.0 PKCE
  api <args…>        Forward to `zad spotify …` with ZAD_PERMISSIONS_PATH
                     pinned to the active profile's file. Requires
                     `spotifai ask` or `spotifai playlist` to have selected
                     a profile; direct shell invocations exit with an error.
  ask [query…]       Read-only zag session over your Spotify library, with
                     ~/.spotifai/permissions/ask.toml injected.
  playlist [query…]  zag session that builds one new playlist for you, with
                     ~/.spotifai/permissions/playlist.toml injected.
  help               Print help for a command

Options:
      --version           Print version
```

Run `spotifai help <command>` or see [`man/main.md`](man/main.md) for full flag reference.

## Configuration

| File | Purpose |
|---|---|
| `~/.spotifai/bin/zad` | Pinned zad binary, written by `spotifai install`. |
| `~/.spotifai/permissions/` | Per-profile policy files. `ask.toml` ships read-only and is injected into the `spotifai ask` system prompt; `playlist.toml` adds `playlists create`, `playlists add`, and `playlists rename` for `spotifai playlist`. Hand-edit `allowed` / `denied` in either file to widen or narrow, then re-run `spotifai install` so every profile file is resigned and zad will load it again. |
| `~/.zad/signing/trusted.toml` | Per-machine signed trust store, populated by `zad signing init` during `spotifai install`. zad ≥ 0.4.0 fails closed at load time on any permissions file whose `[signature]` block is not in this trust store. |

Spotify credentials and zad's own permissions live under `~/.zad/` and
are managed by zad directly — see
[zad's docs/configuration.md](https://github.com/niclaslindstedt/zad/blob/main/docs/configuration.md)
for the full reference.

See [docs/configuration.md](docs/configuration.md) for the spotifai-side reference.

## Examples

See [`examples/`](examples/) for runnable shell script demos.

## Troubleshooting

_Common failure modes and fixes._

- **`spotifai auth` hangs or fails** — confirm your Spotify app dashboard has `http://127.0.0.1` registered as an allowed redirect host. zad's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
- **"no credentials" from `spotifai api`** — run `spotifai auth` to register a Spotify Client ID and refresh token at zad's global scope.
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