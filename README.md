# spotifai

A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).

[![ci](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml/badge.svg)](https://github.com/niclaslindstedt/spotifai/actions/workflows/ci.yml)
[![release](https://github.com/niclaslindstedt/spotifai/actions/workflows/release.yml/badge.svg)](https://github.com/niclaslindstedt/spotifai/actions/workflows/release.yml)
[![pages](https://github.com/niclaslindstedt/spotifai/actions/workflows/pages.yml/badge.svg)](https://github.com/niclaslindstedt/spotifai/actions/workflows/pages.yml)
[![crates](https://img.shields.io/crates/v/spotifai.svg)](https://crates.io/crates/spotifai)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Why?

- Ask about your library and playlists in plain language — no memorising API endpoints
- Build new playlists conversationally with `spotifai playlist`, while `spotifai ask` stays read-only
- Clean up your library with `spotifai clean` — delete playlists, remove tracks, unsave items, with a confirmation gate before every destructive call
- Per-command permission profiles, signed at install time, so the agent can only use the verbs each surface needs
- Multiple backing providers — pick one with `--provider` (Spotify by default, YouTube Music via zad ≥ 0.6.0)
- Zero duplicated agent or music-service code — delegates entirely to zag and zad
- Composable with shell scripts and other CLI tools for automation

## Supported providers

| Provider slug | Display name  | Notes |
|---|---|---|
| `spotify` (default) | Spotify       | OAuth 2.0 PKCE, one developer app per user. |
| `ymusic`            | YouTube Music | Google OAuth 2.0 Desktop-app credentials, talks to YouTube Data API v3. |

The provider abstraction is built so a third backend (Tidal, Apple Music, …) is one new variant in `src/providers.rs` plus the matching policy/example block.

## Prerequisites

- Rust stable (≥ 1.88) and Cargo — install via [rustup](https://rustup.rs/)
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

Example output uses `#>` so the block stays valid `sh`; the website
terminal renders those lines as program output.

```sh
# Ask your library a question
spotifai ask "what are my most recently added albums?"
#> == spotifai ask (Spotify) ==
#> permissions: ~/.spotifai/permissions/spotify/ask.toml
#> starting interactive zag session — Ctrl+D to exit
#>
#> Your 5 most recently added albums:
#>   1. Midnights — Taylor Swift             (added 2 days ago)
#>   2. Igor — Tyler, The Creator            (added 5 days ago)
#>   3. Currents — Tame Impala               (added 1 week ago)
#>   4. Blonde — Frank Ocean                 (added 2 weeks ago)
#>   5. To Pimp a Butterfly — Kendrick Lamar (added 3 weeks ago)

# Build a focus playlist conversationally
spotifai playlist "a 30-minute focus playlist with no vocals"
#> == spotifai playlist (Spotify) ==
#> permissions: ~/.spotifai/permissions/spotify/playlist.toml
#> starting interactive zag session — Ctrl+D to exit
#>
#> Searching for instrumental tracks around 30 minutes…
#> Created `Focus · 30min` with 12 tracks (29:48).

# Clean up old saved songs (asks before deleting)
spotifai clean "remove all baby songs — my child is 15 now"
#> == spotifai clean (Spotify) ==
#> permissions: ~/.spotifai/permissions/spotify/clean.toml
#> starting interactive zag session — Ctrl+D to exit
#>
#> Found 38 saved tracks matching "baby" across your library.
#> Proceed with unsaving these 38 items? (yes/no)
#> > yes
#> Unsaved 38 tracks.

# Migrate your Spotify library to YouTube Music
spotifai export --provider spotify | spotifai import --provider ymusic
#> == spotifai export (Spotify) ==
#> fetching liked tracks…
#>   247 liked tracks
#> fetching saved albums…
#>   18 saved albums
#> fetching playlists…
#>   12 playlists
#> ✓  exported 247 liked items, 18 albums, 12 playlists (843 playlist tracks)
#> == spotifai import (YouTube Music) ==
#> fetching existing playlists on target…
#> 0 existing playlists on target
#> ✓  created `Focus · 30min` with 12 videos
#> ✓  created `Workout` with 41 videos
#> ✓  imported 12 playlists (843 tracks added, 0 skipped duplicate, 0 failed playlists, 24 unresolved tracks, 0 failed adds)
```

First-time setup is a one-off — run `spotifai install` (scaffolds and
signs the per-provider permissions files in `~/.spotifai/permissions/`)
and `spotifai auth` (OAuth 2.0 PKCE loopback for Spotify; pass
`--provider ymusic` for YouTube Music) before the examples above.

## Usage

```
spotifai <command> [options]

Commands:
  install            Guided setup: bootstrap the local signing key,
                     scaffold and sign every per-(provider, profile)
                     permissions file under
                     ~/.spotifai/permissions/<provider>/.
  auth   [args…]     Run an in-process OAuth loopback flow for the
                     active provider and store the resulting tokens
                     in the OS keychain. Accepts --client-id,
                     --client-secret (ymusic only), and --no-browser.
  api    <args…>     Run a typed call against the active provider
                     through the in-process zad library and print
                     JSON to stdout. Grammar: search "q",
                     playlists list/show/create/add, library tracks
                     list / library albums list (Spotify), library
                     list (ymusic). Requires `spotifai ask`/`playlist`
                     to have selected a (provider, profile) pair via
                     env vars; direct shell invocations exit with an
                     error.
  ask    [query…]    Read-only zag session over your library, with
                     ~/.spotifai/permissions/<provider>/ask.toml
                     injected.
  playlist [query…]  zag session that builds one new playlist for you,
                     with ~/.spotifai/permissions/<provider>/playlist.toml
                     injected.
  clean    [query…]  Destructive zag session for cleaning up your
                     library: deletes playlists, removes tracks from
                     playlists, unsaves tracks/albums. Uses
                     ~/.spotifai/permissions/<provider>/clean.toml.
                     The agent enumerates candidates and asks for
                     confirmation before every destructive call.
  export             Dump the user's library on the active provider
                     into the unified spotifai JSON schema. --provider
                     selects the source; --output PATH writes to a
                     file instead of stdout.
  import             Recreate playlists from a `spotifai export`
                     envelope on the active provider. Reads stdin or
                     --input PATH. Cross-provider migrations resolve
                     tracks via ISRC (Spotify) then title+artist
                     search; same-provider re-imports reuse the
                     embedded source ids. Existing playlists with
                     the same name are skipped.
  help               Print help for a command

Options:
      --provider <slug>   Backing provider for the surface (spotify | ymusic).
                          Default: spotify. Available on auth/ask/playlist/clean/export/import.
      --wait / --no-wait  Sleep through (or fail fast on) an active rate-limit
                          cooldown window (Spotify HTTP 429, or ymusic HTTP 429 /
                          Google-quota HTTP 403). Default: wait for ask/playlist/clean,
                          fail-fast for api/export/import. SPOTIFAI_WAIT overrides the default.
      --yolo              Run the underlying zag agent with maximum permissions —
                          skip every per-tool approval prompt. Only meaningful
                          for ask/playlist/clean; the (provider, profile) policy
                          is still enforced at the zad layer. The clean prompt's
                          confirmation gate is unaffected.
      --debug             Echo debug-level events to stderr (the file log under
                          ~/.local/state/spotifai/debug.log captures them either way).
      --help-agent        Print a compact, prompt-injectable description of
                          spotifai for splicing into an LLM prompt.
      --debug-agent       Print a compact troubleshooting context block for
                          prompt injection into a debugging session.
      --version           Print version
```

Run `spotifai help <command>` or see [`man/main.md`](man/main.md) for full flag reference.

## Configuration

| File | Purpose |
|---|---|
| `~/.spotifai/permissions/<provider>/` | Per-(provider, profile) policy files. `ask.toml` ships read-only; `playlist.toml` adds `playlists create`, `playlists add`, and `playlists rename`; `clean.toml` adds the destructive verbs (`playlists delete`, `playlists remove`, and the library-side unsave/unlike verbs) while denying `search` and every creator verb. Hand-edit `allowed` / `denied` to widen or narrow, then re-run `spotifai install` so every profile file is resigned. Verb names differ between providers (Spotify: `library tracks/albums …`; YouTube Music: `library list` / `library like|unlike`). |
| `~/.spotifai/<provider>.toml` | Per-provider self-identity captured at OAuth time (Spotify user id, YouTube channel id). Used by `playlists create`. |
| `~/.zad/signing/trusted.toml` | Per-machine signed trust store, populated by `spotifai install`. The zad library fails closed at load time on any permissions file whose signature is not registered here. |

Provider credentials are stored in the OS keychain (under the `zad`
service, accounts `spotify-client-id:global`, `spotify-refresh:global`,
`ymusic-client-id:global`, `ymusic-client-secret:global`,
`ymusic-refresh:global`). Run `spotifai auth` to (re-)register them.

See [docs/configuration.md](docs/configuration.md) for the spotifai-side reference and [docs/export_schema.md](docs/export_schema.md) for the export/import envelope schema.

## Examples

See [`examples/`](examples/) for runnable shell script demos.

## Troubleshooting

_Common failure modes and fixes._

- **`spotifai auth` hangs or fails (Spotify)** — confirm your Spotify app dashboard has `https://127.0.0.1` registered as an allowed redirect host. spotifai's loopback listener picks a random port and terminates TLS in-process with a per-session self-signed certificate; Spotify accepts any port on `127.0.0.1` once the host is registered.
- **`spotifai auth --provider ymusic` rejects the credential** — make sure the YouTube Data API v3 is enabled on the Google Cloud project and that your Google account is on the OAuth consent screen's test-user list while the app is in testing.
- **"missing credentials" from `spotifai api`** — run `spotifai auth` (Spotify) or `spotifai auth --provider ymusic` (YouTube Music) to register a Client ID and refresh token in the OS keychain.
- **Agent gives wrong results** — use `-v` to inspect reasoning steps and refine your query.

## Documentation

- [Getting started](docs/getting-started.md)
- [Configuration](docs/configuration.md)
- [Export schema](docs/export_schema.md)
- [Architecture](docs/architecture.md)
- [Troubleshooting](docs/troubleshooting.md)

## Community

- **Bugs and feature requests** — [GitHub Issues](https://github.com/niclaslindstedt/spotifai/issues)
- **Questions and discussion** — [GitHub Discussions](https://github.com/niclaslindstedt/spotifai/discussions)
- **Security reports** — see [SECURITY.md](SECURITY.md) (private channel — do not file public issues)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under [MIT](LICENSE).
