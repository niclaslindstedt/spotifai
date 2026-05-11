# spotifai

> A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).

## Synopsis

```
spotifai [OPTIONS] [COMMAND]
```

## Description

`spotifai` is a thin shell around two upstream tools — **zag** (the LLM agent runtime) and **zad** (the music-service API client). The agent surfaces (`ask`, `playlist`, `export`) target a single backing music **provider** at a time, selected with `--provider <name>` (default: `spotify`). Today the supported providers are:

| Provider slug | Display name  | Backing zad subcommand |
|---|---|---|
| `spotify` | Spotify       | `zad spotify`           |
| `ymusic`  | YouTube Music | `zad ymusic` (zad ≥ 0.6.0) |

Adding another provider is a single change in `src/providers.rs` and is picked up by every command automatically.

## Subcommands

| Command | Description |
|---|---|
| `install`  | Walk the three-step setup that makes the agent surfaces usable: bootstrap the local Ed25519 signing key in the OS keychain, scaffold every per-`(provider, profile)` file under `~/.spotifai/permissions/<provider>/`, and sign each one so zad's load-time trust check passes. Idempotent. |
| `auth`     | Run an in-process OAuth loopback flow for the chosen provider and write the resulting tokens into the OS keychain. Spotify uses PKCE (no client secret); YouTube Music uses Google OAuth 2.0 Desktop-app credentials. |
| `api`      | Dispatch a typed call into the in-process zad library and print the JSON response. Requires the active profile (set by `ask`, `playlist`, `export`, or `import`); direct shell invocations error out. The matching `~/.spotifai/permissions/<provider>/<profile>.toml` is consulted via `ZAD_PERMISSIONS_PATH`. |
| `ask`      | Read-only zag session about the user's library on the active provider, with `~/.spotifai/permissions/<provider>/ask.toml` injected into the system prompt. |
| `playlist` | zag session that builds one new playlist for the user on the active provider, with `~/.spotifai/permissions/<provider>/playlist.toml` injected. Adds `playlists create`, `playlists add`, and `playlists rename`; destructive verbs stay denied. |
| `export`   | Dump the user's library on the active provider — liked tracks/videos, saved albums (Spotify only), and playlists with full ordered track lists — into one structured JSON document. Designed to be portable enough to re-import on another music service later. Defaults to stdout; `--output` redirects to a file. |
| `import`   | Recreate playlists from a `spotifai export` envelope on the active provider. Reads from stdin by default or `--input PATH`. Same-provider re-imports reuse the embedded IDs; cross-provider migrations (e.g. Spotify → YouTube Music) resolve each track on the target via `zad <provider> search` (ISRC first, then title + primary artist). Existing playlists with the same name are skipped. |
| `commands` | Machine-readable command index (§12.4). With no argument, lists every command and its usage signature, one per line. With `<name>`, prints the full usage spec for that command. Add `--examples` to print realistic example invocations instead. |
| `man`      | Print an embedded reference manpage (§12.3). With no argument, lists every command that has a manpage. With a `<command>` argument, prints `man/<command>.md`. |
| `docs`     | Print an embedded conceptual doc (§12.3). With no argument, lists every available topic. With a `<topic>` argument, prints `docs/<topic>.md`. |
| `help`     | Show help text. |

### `spotifai install`

Walks a three-step guided setup. Each step prints a header so a first-time user can see what is happening.

1. **Bootstrap signing key.** Mints a fresh Ed25519 keypair in the OS keychain (account `zad/signing:v1`) via `zad::permissions::signing::load_or_create_from_keychain` and writes a self-signed empty trust store at `~/.zad/signing/trusted.toml`. Idempotent — when a key already exists, the call returns its fingerprint and leaves the keychain untouched.
2. **Write default permission profiles.** Scaffolds every `<provider>/<profile>.toml` file under `~/.spotifai/permissions/`. `ask.toml` ships read-only (allows `search`, `playlists list/show`, the read-side library verbs); `playlist.toml` adds `playlists create`, `playlists add`, and `playlists rename` for `spotifai playlist`. Verb names differ between providers — e.g. Spotify exposes `library tracks list` and `library albums list`, while YouTube Music exposes a single `library list` over rated videos. Hand-edits to existing files are preserved across re-runs.
3. **Sign permission profiles.** Calls `zad::permissions::signing::sign_unsigned` once per `(provider, profile)` pair and upserts each resulting signature into the per-machine trust store at `~/.zad/signing/trusted.toml`. zad ≥ 0.4.0 fails closed at load time on permission files that are not in the trust store; signing here is what unblocks the first agent surface call. The step runs unconditionally on every `install` invocation, so re-running `spotifai install` after a hand-edit resigns every file.

`spotifai install` takes no flags.

### `spotifai auth`

Runs an in-process OAuth loopback flow for the active provider via `zad::oauth::run_loopback_flow` and writes the resulting tokens into the OS keychain under the `zad` service. Spotify uses an OAuth 2.0 PKCE public-client flow (no `client_secret`) and terminates a per-session HTTPS loopback listener with a self-signed certificate. YouTube Music (zad ≥ 0.6.0) uses a Google OAuth 2.0 Desktop-app flow (HTTP loopback, with a `client_secret`). After the flow, spotifai probes the provider's "self" endpoint and persists the captured user/channel id at `~/.spotifai/<provider>.toml` so `playlists create` can reuse it. See [`auth.md`](auth.md) for the full reference.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Which provider to register credentials for. One of `spotify`, `ymusic`. |
| `--client-id <id>` | string | — | Skip the interactive prompt for the OAuth client id. |
| `--client-secret <secret>` | string | — | Required for `ymusic`; rejected for Spotify (PKCE has no secret). |
| `--no-browser` | bool | false | Don't auto-open the consent screen; print the URL on stderr only. |

### `spotifai api`

Parses everything after `api` as a typed verb and dispatches it into the in-process zad library — `zad::service::spotify::Spotify` for Spotify and `zad::service::ymusic::Ymusic` for YouTube Music — with `ZAD_PERMISSIONS_PATH` pinned at the file backing the **active** `(provider, profile)` pair. Both axes are selected by the parent `spotifai ask` / `spotifai playlist` / `spotifai export` / `spotifai import` command via the `SPOTIFAI_PROVIDER` and `SPOTIFAI_PROFILE` env vars; direct shell invocations exit with a usage error pointing the user at those commands. See [`api.md`](api.md) for the full reference.

`spotifai api` does **not** take its own `--provider` flag — it would be swallowed by the trailing-var-arg pass-through. Set `SPOTIFAI_PROVIDER` if you must invoke `api` outside of a parent agent surface; otherwise use the parent command's `--provider` flag.

### `spotifai ask`

Start an interactive zag session pre-loaded with a system prompt that explains how to use `spotifai api …` and injects `~/.spotifai/permissions/<provider>/ask.toml` so the agent self-restricts to the listed verbs.

| Argument / Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider to query. One of `spotify`, `ymusic`. |
| `[query…]` | string | — | Optional opening question. Joined with spaces and used as the agent's first turn. With no argument the session opens empty and waits for the user to type. |

The agent talks to the active provider only through `spotifai api …` (no direct API calls), and is instructed in the system prompt never to widen the policy itself. To loosen the surface, edit `~/.spotifai/permissions/<provider>/ask.toml` directly — the file is re-read on every `spotifai ask` invocation, and `spotifai install` resigns it. The permissions files are never overwritten without your edit.

### `spotifai playlist`

Start an interactive zag session pre-loaded to build a new playlist on the active provider. Loads `~/.spotifai/permissions/<provider>/playlist.toml`, which extends the `ask` policy with `playlists create`, `playlists add`, and `playlists rename`. Destructive verbs (`playlists delete`, `playlists remove`) and library writes (`library tracks save/unsave`, `library albums save/unsave`, or `library like/unlike` for YouTube Music) stay denied even in this profile.

| Argument / Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider for the new playlist. One of `spotify`, `ymusic`. |
| `[query…]` | string | — | Optional brief (e.g. `"a 30-min focus playlist"`). Joined with spaces and used as the agent's first turn. With no argument the session opens empty and waits for the user to type. |

Like `ask`, the agent in `playlist` only talks to the provider through `spotifai api …` and is instructed not to widen the policy itself. Edit `~/.spotifai/permissions/<provider>/playlist.toml` and re-run `spotifai install` to resign the file when you change `allowed` / `denied`.

### `spotifai export`

Walk the user's library on the active provider and write one JSON document containing every record needed to recreate the library elsewhere. Reuses the read-only `ask` permission profile (no new profile to scaffold or sign). Records are embedded verbatim under the envelope, so any identifier zad already exposes (`isrc`, `spotify_id`, `video_id`, `added_at`, position, duration, …) flows through to a future importer.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider whose library to export. One of `spotify`, `ymusic`. |
| `--output PATH`, `-o PATH` | path | — | Write the JSON document to this file instead of stdout. Parent directories are created if needed. |
| `--pretty` | bool | false | Pretty-print the JSON with two-space indent. |

The JSON document goes to stdout by default; status messages always go to stderr so `spotifai export | jq …` and `spotifai export > library.json` work as expected. The envelope's `source.service` field carries the provider slug (`spotify` / `ymusic`); both providers populate the unified top-level `tracks` array (liked songs / liked videos), Spotify also populates `albums` (saved albums; YouTube Music leaves it `[]` because the YouTube Data API has no "saved albums" concept), and `playlists` is the full ordered set with each track embedded under the same `Track` schema. See [`export.md`](export.md) for the full reference and [`../docs/export_schema.md`](../docs/export_schema.md) for the schema definition.

### `spotifai import`

Recreate playlists from a `spotifai export` envelope on the active provider. The canonical migration form is one pipeline (`spotifai export --provider spotify | spotifai import --provider ymusic`), but the envelope can also be read from `--input PATH`. Same-provider re-imports reuse the embedded `spotify_id` / `video_id` directly; cross-provider migrations resolve each track on the target via `zad <provider> search` — ISRC first, then a title + primary-artist text fallback. Items that match neither query are reported as unresolved and skipped. Playlists whose name already exists on the target are skipped with a warning, which makes re-runs idempotent. Reuses the `playlist` permission profile (no new profile to scaffold or sign). Liked tracks, liked videos, and saved albums in the envelope are intentionally ignored.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider to import the playlists onto. One of `spotify`, `ymusic`. |
| `--input PATH`, `-i PATH` | path | — | Read the envelope from this file. Without it, the envelope is read from stdin. |
| `--dry-run` | bool | false | Preview what would be created without making any zad write calls. The duplicate-name pre-fetch and any cross-provider search calls still run. |

Per-playlist or per-track failures inside the loop accumulate into the final summary on stderr and do not abort the import. The command exits 0 unless a fatal error (input parse, schema mismatch, install failure) prevents the loop from running. See [`import.md`](import.md) for the full reference.

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--debug`         | bool | false | Echo `debug`-level diagnostics to stderr in addition to the always-on `debug.log`. The log file captures `debug` regardless of this flag (§19.2 / §19.3). Global — works on every subcommand. |
| `--help-agent`    | bool | false | Print a compact, prompt-injectable description of `spotifai` to stdout and exit (§12.1). Designed to be spliced into an LLM prompt via command substitution: `claude "$(spotifai --help-agent) — list my playlists"`. |
| `--debug-agent`   | bool | false | Print a compact troubleshooting context block — log paths, config locations, env vars, common failure modes — to stdout and exit (§12.2). Designed for command substitution into a debugging prompt. |
| `--wait`          | bool | (see below) | When the active provider is in a 429 cooldown window (deadline persisted by zad 0.8.0 at `~/.zad/state/<service>/rate_limit.json`), sleep until the deadline and continue instead of failing fast. No-op when no cooldown is recorded. Default: `true` for the interactive surfaces (`ask`, `playlist`) so multiple sub-agents coordinate cleanly; `false` for one-shot commands (`api`, `export`, `import`) so a user-driven invocation surfaces 429s loudly. The `SPOTIFAI_WAIT` env var overrides the default; an explicit flag overrides both. Global — works on every subcommand. |
| `--no-wait`       | bool | (see above) | Force fail-fast behaviour even when `SPOTIFAI_WAIT=1` is set. Mutually exclusive with `--wait`. |
| `--version`       | bool | false | Print version and exit. |
| `--help`          | bool | false | Print help and exit. |

### `spotifai commands`

Machine-readable command index (§12.4). The output is plain text on stdout with no ANSI escapes, in a line format that does not change across patch releases. Commands, flag specifications, and example invocations all come from `src/commands_index.rs`, the same source of truth `--help-agent` reads from.

| Argument / Flag | Type | Default | Description |
|---|---|---|---|
| `[name]` | string | — | Command to look up. Without it, every command is listed. |
| `--examples` | bool | false | Print realistic example invocations instead of the usage spec. Combine with `[name]` to scope to one command. |

```sh
spotifai commands              # list every command, one per line
spotifai commands ask          # full usage spec for one command
spotifai commands --examples   # realistic examples for every command
spotifai commands export --examples
```

### `spotifai man`

Print an embedded reference manpage (§12.3). The full `man/<command>.md` directory is compiled into the binary via `include_str!`, so `spotifai man` works offline and never fetches at runtime.

| Argument | Type | Default | Description |
|---|---|---|---|
| `[command]` | string | — | Command whose manpage to print. With no argument, lists every available manpage. |

### `spotifai docs`

Print an embedded conceptual doc (§12.3). The `docs/` directory is compiled into the binary the same way as `man`. Topics map onto the file stems under `docs/`.

| Argument | Type | Default | Description |
|---|---|---|---|
| `[topic]` | string | — | Topic whose doc to print (`getting-started`, `configuration`, `architecture`, `export-schema`, `troubleshooting`). With no argument, lists every available topic. |

## Log file

Every `spotifai` invocation appends to a persistent debug log at a
platform-appropriate location. The log captures every level — including
`debug` — so a failed run can be triaged without re-running the command
with extra flags.

| Platform | Path |
|---|---|
| Linux   | `~/.local/state/spotifai/debug.log` |
| macOS   | `~/Library/Application Support/spotifai/debug.log` |
| Windows | `%APPDATA%\spotifai\debug.log` |

The file rolls forever — there is no built-in rotation in v1. Truncate
it manually (`: > ~/.local/state/spotifai/debug.log`) or wire up
`logrotate`. The `SPOTIFAI_LOG` environment variable accepts a
[`tracing_subscriber::EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html)
directive (e.g. `SPOTIFAI_LOG=spotifai=trace,zad=debug`) for surgical
verbosity tweaks; the default is `debug`.

## Environment variables

| Variable | Description |
|---|---|
| `SPOTIFAI_PROVIDER` | Read by `spotifai api` to pick the zad subcommand and the matching `<provider>/` directory under `~/.spotifai/permissions/`. Set on the user's behalf by `ask` / `playlist` / `export`; defaults to `spotify` when unset for backwards compatibility. |
| `SPOTIFAI_PROFILE`  | Read by `spotifai api` to pick the profile file under the active provider's directory (`ask.toml` / `playlist.toml`). Required for `api` to run; missing or unknown values exit with a usage error. |
| `SPOTIFAI_WAIT`     | Read by every `spotifai` invocation to decide whether to sleep through an active 429 cooldown (`1`/`true`/`yes`/`on` → wait; `0`/`false`/`no`/`off` → fail-fast). Set on the user's behalf by `spotifai ask` and `spotifai playlist` to `1` so child `spotifai api` shells coordinate. The CLI `--wait` / `--no-wait` flags override the env var. |
| `SPOTIFAI_LOG`      | [`tracing_subscriber::EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html) directive controlling the verbosity of the `debug.log` writer. Defaults to `debug`. |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Generic error |
| 2 | Usage error |

## Examples

```sh
spotifai --help
spotifai ask "What are my most-played albums?"
spotifai playlist --provider ymusic "an upbeat 45-minute commute playlist"
spotifai export --provider ymusic --pretty -o ~/backups/ymusic.json
spotifai export --provider spotify | spotifai import --provider ymusic --dry-run
spotifai auth --provider ymusic --client-id <id> --client-secret <secret>
```

## See also

- [`install.md`](install.md) — `spotifai install` reference
- [`auth.md`](auth.md) — `spotifai auth` reference
- [`api.md`](api.md) — `spotifai api` reference
- [`ask.md`](ask.md) — `spotifai ask` reference
- [`playlist.md`](playlist.md) — `spotifai playlist` reference
- [`export.md`](export.md) — `spotifai export` reference
- [`import.md`](import.md) — `spotifai import` reference
