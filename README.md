# spotifai

A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).

[![CI](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml/badge.svg)](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Why?

- Ask about your library and playlists in plain language — no memorising API endpoints
- Build new playlists conversationally with `spotifai playlist`, while `spotifai ask` stays read-only
- Per-command permission profiles, signed at install time, so the agent can only use the verbs each surface needs
- Multiple backing providers — pick one with `--provider` (Spotify by default, YouTube Music via zad ≥ 0.6.0)
- Zero duplicated agent or music-service code — delegates entirely to zag and zad
- Composable with shell scripts and other CLI tools for automation

## Supported providers

| Provider slug | Display name  | Backing zad subcommand | Notes |
|---|---|---|---|
| `spotify` (default) | Spotify       | `zad spotify` | OAuth 2.0 PKCE, one developer app per user. |
| `ymusic`            | YouTube Music | `zad ymusic`  | Google OAuth 2.0 Desktop-app credentials, talks to YouTube Data API v3. Requires zad ≥ 0.6.0. |

The provider abstraction is built so a third backend (Tidal, Apple Music, …) is one new variant in `src/providers.rs` plus the matching policy/example block.

## Prerequisites

- Rust stable (≥ 1.78) and Cargo — install via [rustup](https://rustup.rs/)
- For Spotify: a [Spotify developer app](https://developer.spotify.com/dashboard) — note your **Client ID** and add `http://127.0.0.1` as a redirect host
- For YouTube Music: a [Google Cloud OAuth 2.0 Desktop client](https://console.cloud.google.com/) with the YouTube Data API v3 enabled — note your **Client ID** and **Client secret**

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
# keychain, scaffold the per-provider permissions files under
# ~/.spotifai/permissions/<provider>/ (ask.toml is read-only;
# playlist.toml adds create/add/rename), and sign each one. Re-run
# after editing any profile file to resign.
spotifai install

# Authenticate with Spotify (opens browser for the OAuth 2.0 PKCE flow).
# Forwards to `zad service create spotify` at zad's global scope, so the
# credential applies to every directory you later run `spotifai api …` from.
spotifai auth

# (Optional) Authenticate with YouTube Music. Same flow, against Google.
spotifai auth --provider ymusic

# Ask a natural-language question about your Spotify library — `ask` is
# read-only and self-restricts to the verbs in
# ~/.spotifai/permissions/spotify/ask.toml.
spotifai ask "What are my most recently added albums?"

# Same question against YouTube Music — uses the ymusic profile and
# ymusic-shaped verbs (no albums; library list covers rated videos).
spotifai ask --provider ymusic "What playlists do I have?"

# Build a new Spotify playlist conversationally — `playlist` loads
# ~/.spotifai/permissions/spotify/playlist.toml so the agent can create
# one new playlist, add tracks to it, and rename it. Destructive verbs
# stay denied.
spotifai playlist "a 30-minute focus playlist with no vocals"

# Build a new YouTube Music playlist.
spotifai playlist --provider ymusic "an upbeat 45-minute commute playlist"

# Migrate your Spotify library to YouTube Music — playlists are recreated
# on the target, with tracks resolved by ISRC (then title + artist) on the
# new provider. Existing playlists with the same name are skipped, so re-runs
# are idempotent.
spotifai export --provider spotify | spotifai import --provider ymusic
```

## Usage

```
spotifai <command> [options]

Commands:
  install            Guided setup: install pinned zad binary, bootstrap
                     local signing key, scaffold and sign every per-
                     (provider, profile) permissions file under
                     ~/.spotifai/permissions/<provider>/.
  auth   [args…]     Forward to `zad service create <provider>` (global
                     scope) to register OAuth credentials. --provider
                     selects the backend (default: spotify).
  api    <args…>     Forward to `zad <provider> …` with ZAD_PERMISSIONS_PATH
                     pinned to the active profile's file. Requires
                     `spotifai ask`/`playlist`/`export` to have selected
                     a (provider, profile) pair via env vars; direct
                     shell invocations exit with an error.
  ask    [query…]    Read-only zag session over your library, with
                     ~/.spotifai/permissions/<provider>/ask.toml injected.
  playlist [query…]  zag session that builds one new playlist for you,
                     with ~/.spotifai/permissions/<provider>/playlist.toml
                     injected.
  export             Dump the user's library on the active provider into
                     one JSON document. --provider selects the backend;
                     --output PATH writes to a file instead of stdout.
  import             Recreate playlists from a `spotifai export` envelope
                     on the active provider. Reads stdin or --input PATH.
                     Cross-provider migrations resolve tracks via ISRC
                     then title+artist search; same-provider re-imports
                     reuse the embedded IDs. Existing playlists with the
                     same name are skipped.
  help               Print help for a command

Options:
      --provider <slug>   Backing provider for the surface (spotify | ymusic).
                          Default: spotify. Available on auth/ask/playlist/export/import.
      --version           Print version
```

Run `spotifai help <command>` or see [`man/main.md`](man/main.md) for full flag reference.

## Configuration

| File | Purpose |
|---|---|
| `~/.spotifai/bin/zad` | Pinned zad binary, written by `spotifai install`. |
| `~/.spotifai/permissions/<provider>/` | Per-(provider, profile) policy files. `ask.toml` ships read-only; `playlist.toml` adds `playlists create`, `playlists add`, and `playlists rename`. Hand-edit `allowed` / `denied` to widen or narrow, then re-run `spotifai install` so every profile file is resigned and zad will load it again. Verb names differ between providers (Spotify: `library tracks/albums …`; YouTube Music: `library list` / `library like|unlike`). |
| `~/.zad/signing/trusted.toml` | Per-machine signed trust store, populated by `zad signing init` during `spotifai install`. zad ≥ 0.4.0 fails closed at load time on any permissions file whose `[signature]` block is not in this trust store. |

Provider credentials and zad's own runtime permissions live under `~/.zad/services/<provider>/...` and are managed by zad directly — see [zad's docs/configuration.md](https://github.com/niclaslindstedt/zad/blob/main/docs/configuration.md) for the full reference.

See [docs/configuration.md](docs/configuration.md) for the spotifai-side reference.

## Examples

See [`examples/`](examples/) for runnable shell script demos.

## Troubleshooting

_Common failure modes and fixes._

- **`spotifai auth` hangs or fails (Spotify)** — confirm your Spotify app dashboard has `http://127.0.0.1` registered as an allowed redirect host. zad's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
- **`spotifai auth --provider ymusic` rejects the credential** — make sure the YouTube Data API v3 is enabled on the Google Cloud project and that your Google account is on the OAuth consent screen's test-user list while the app is in testing.
- **"no credentials" from `spotifai api`** — run `spotifai auth` (Spotify) or `spotifai auth --provider ymusic` (YouTube Music) to register a Client ID and refresh token at zad's global scope.
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
