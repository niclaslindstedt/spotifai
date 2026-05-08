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
| `install`  | Walk the four-step setup that makes `spotifai api` usable: download the pinned zad binary into `~/.spotifai/bin/zad`, bootstrap the local Ed25519 signing key, scaffold every per-`(provider, profile)` file under `~/.spotifai/permissions/<provider>/`, and sign each one so zad's load-time trust check passes. Idempotent. |
| `auth`     | Forward to `zad service create <provider>` (global scope) to register OAuth credentials for the chosen provider. Spotify uses one developer app per user; YouTube Music uses Google OAuth 2.0 Desktop-app credentials. |
| `api`      | Forward to `zad <provider> …` after verifying the pinned zad binary. Requires the active profile (set by `ask` or `playlist`); direct shell invocations error out. The forwarded child gets `ZAD_PERMISSIONS_PATH` pinned to the matching `~/.spotifai/permissions/<provider>/<profile>.toml`. |
| `ask`      | Read-only zag session about the user's library on the active provider, with `~/.spotifai/permissions/<provider>/ask.toml` injected into the system prompt. |
| `playlist` | zag session that builds one new playlist for the user on the active provider, with `~/.spotifai/permissions/<provider>/playlist.toml` injected. Adds `playlists create`, `playlists add`, and `playlists rename`; destructive verbs stay denied. |
| `export`   | Dump the user's library on the active provider — liked tracks/videos, saved albums (Spotify only), and playlists with full ordered track lists — into one structured JSON document. Designed to be portable enough to re-import on another music service later. Defaults to stdout; `--output` redirects to a file. |
| `import`   | Recreate playlists from a `spotifai export` envelope on the active provider. Reads from stdin by default or `--input PATH`. Same-provider re-imports reuse the embedded IDs; cross-provider migrations (e.g. Spotify → YouTube Music) resolve each track on the target via `zad <provider> search` (ISRC first, then title + primary artist). Existing playlists with the same name are skipped. |
| `help`     | Show help text. |

### `spotifai install`

Walks a four-step guided setup. Each step prints a header so a first-time user can see what is happening.

1. **Install zad binary.** Ensures `~/.spotifai/bin/zad` matches the version pinned in `.zadrc` (baked in at build time). Spotifai forward-routes its `api …` subcommands to this exact path, so the binary on `$PATH` is intentionally never used. Re-runs are no-ops once the right version is present.
2. **Bootstrap signing key.** Runs `zad signing init`, which mints a fresh Ed25519 keypair in the OS keychain (account `signing:v1`) and writes a self-signed empty trust store at `~/.zad/signing/trusted.toml`. Idempotent — when a key already exists, the command prints its fingerprint and leaves the keychain untouched.
3. **Write default permission profiles.** Scaffolds every `<provider>/<profile>.toml` file under `~/.spotifai/permissions/`. `ask.toml` ships read-only (allows `search`, `playlists list/show`, the read-side library verbs); `playlist.toml` adds `playlists create`, `playlists add`, and `playlists rename` for `spotifai playlist`. Verb names differ between providers — e.g. Spotify exposes `library tracks list` and `library albums list`, while YouTube Music exposes a single `library list` over rated videos. Hand-edits to existing files are preserved across re-runs.
4. **Sign permission profiles.** Runs `zad <provider> permissions sign --local` once per `(provider, profile)` pair, with `ZAD_PERMISSIONS_PATH` pinned at the matching file. zad ≥ 0.4.0 fails closed at load time on permission files that are not in the per-machine trust store; signing here is what unblocks the first `spotifai api …` call. The step runs unconditionally on every `install` invocation, so re-running `spotifai install` after a hand-edit resigns every file.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--force` | bool | false | Re-download the zad binary even if the existing one already matches the pinned version. (Does not overwrite existing permissions files; signing always re-runs regardless.) |

### `spotifai auth`

Runs `~/.spotifai/bin/zad service create <provider>` to register OAuth credentials at zad's global scope. Spotify hands out one developer app per user, so the credential intentionally lives at `~/.zad/services/spotify/config.toml` and applies to every directory `spotifai api …` is invoked from. YouTube Music uses Google OAuth 2.0 "Desktop app" credentials at `~/.zad/services/ymusic/config.toml` (client ID + client secret + refresh token). See [`auth.md`](auth.md) for the full reference, including which zad flags pass through.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Which provider to register credentials for. One of `spotify`, `ymusic`. |

### `spotifai api`

Forward-routes everything after `api` to `~/.spotifai/bin/zad <provider> …`, with `ZAD_PERMISSIONS_PATH` injected so the file backing the **active** `(provider, profile)` pair is consulted. Both axes are selected by the parent `spotifai ask` / `spotifai playlist` / `spotifai export` command via the `SPOTIFAI_PROVIDER` and `SPOTIFAI_PROFILE` env vars; direct shell invocations exit with a usage error pointing the user at those commands (or at running `~/.spotifai/bin/zad <provider> …` directly with `ZAD_PERMISSIONS_PATH` set by hand). See [`api.md`](api.md) for the full reference.

`spotifai api` does **not** take its own `--provider` flag — it would be swallowed by the trailing-var-arg pass-through. Set `SPOTIFAI_PROVIDER` if you must invoke `api` outside of a parent agent surface; otherwise use the parent command's `--provider` flag.

### `spotifai ask`

Start an interactive zag session pre-loaded with a system prompt that explains how to use `spotifai api …` and injects `~/.spotifai/permissions/<provider>/ask.toml` so the agent self-restricts to the listed verbs.

| Argument / Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider to query. One of `spotify`, `ymusic`. |
| `[query…]` | string | — | Optional opening question. Joined with spaces and used as the agent's first turn. With no argument the session opens empty and waits for the user to type. |

The agent talks to the active provider only through `spotifai api …` (no direct API calls), and is instructed in the system prompt never to widen the policy itself. To loosen the surface, edit `~/.spotifai/permissions/<provider>/ask.toml` directly — the file is re-read on every `spotifai ask` invocation, and `spotifai install` resigns it. Run `spotifai install --force` to rewrite the binary; the permissions files are never overwritten without your edit.

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

The JSON document goes to stdout by default; status messages always go to stderr so `spotifai export | jq …` and `spotifai export > library.json` work as expected. The envelope's `source.service` field carries the provider slug (`spotify` / `ymusic`); Spotify exports populate `liked_tracks` and `saved_albums`, while YouTube Music exports populate `liked_videos` and leave the album bucket out (the YouTube Data API has no "saved albums" concept). See [`export.md`](export.md) for the full reference, including the envelope schema.

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
| `--version` | bool | false | Print version and exit. |
| `--help`    | bool | false | Print help and exit. |

## Environment variables

| Variable | Description |
|---|---|
| `SPOTIFAI_PROVIDER` | Read by `spotifai api` to pick the zad subcommand and the matching `<provider>/` directory under `~/.spotifai/permissions/`. Set on the user's behalf by `ask` / `playlist` / `export`; defaults to `spotify` when unset for backwards compatibility. |
| `SPOTIFAI_PROFILE`  | Read by `spotifai api` to pick the profile file under the active provider's directory (`ask.toml` / `playlist.toml`). Required for `api` to run; missing or unknown values exit with a usage error. |

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

- [`auth.md`](auth.md) — `spotifai auth` reference
- [`api.md`](api.md) — `spotifai api` reference
- [`ask.md`](ask.md) — `spotifai ask` reference
- [`playlist.md`](playlist.md) — `spotifai playlist` reference
- [`export.md`](export.md) — `spotifai export` reference
- [`import.md`](import.md) — `spotifai import` reference
- `spotifai commands`
- `spotifai docs`
