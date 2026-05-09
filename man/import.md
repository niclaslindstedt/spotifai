# spotifai import

> Recreate playlists from a `spotifai export` envelope on the active provider.

## Synopsis

```
spotifai import [--provider <slug>] [--input PATH] [--dry-run]
```

## Description

`spotifai import` reads a JSON envelope produced by `spotifai export` and recreates its **playlists** on the target provider (`--provider`, default `spotify`). The envelope is read from stdin by default, or from `--input PATH` when set, so the canonical migration form is one pipeline:

```sh
spotifai export --provider spotify | spotifai import --provider ymusic
```

For each playlist in the envelope:

1. **Duplicate skip.** Before the loop, `spotifai import` fetches the user's existing playlists on the target (`zad <provider> playlists list`) once and caches their names. Any source playlist whose name (case-insensitive, trimmed) already exists on the target is **skipped with a warning**. This makes re-runs idempotent — a partial migration can be retried without producing duplicates of already-imported playlists.

2. **Track resolution.** When `source.service` in the envelope matches the target provider, every track's embedded `spotify_id` / `video_id` (or `uri` / `id`) is reused verbatim. When the providers differ — i.e. a cross-provider migration — every track is re-resolved on the target via `zad <provider> search`, ISRC first (`q=isrc:XXXX --type track`), then a title + primary-artist text fallback. The first hit on the target wins. Items that match neither query are reported as unresolved and skipped.

3. **Playlist creation.** A new playlist is created via `zad <provider> playlists create`, using the source name. Spotify accepts `--name`; YouTube Music accepts `--title`; the right flag is selected automatically.

4. **Track addition.** Resolved IDs are added to the new playlist via `zad <provider> playlists add`, chunked at 50 items per call to match both Spotify and YouTube Music API page caps.

Failures inside one playlist (an unresolvable track, a single failed `playlists add` chunk, even a failed `playlists create`) accumulate into the final summary and **do not abort** the import. Re-running with the same envelope picks up where the previous run left off because already-imported playlists are skipped on the duplicate-name guard.

`spotifai import` is **scope-limited to playlists**. The envelope's `liked_tracks`, `liked_videos`, and `saved_albums` buckets are intentionally ignored — those would require widening the `playlist` permission profile (`library tracks save`, `library albums save`, `library like`), which is out of scope for migration. If you need them, run the `spotifai api` write verbs directly with a profile of your own.

Before fetching, spotifai performs the same install/version check as `spotifai install`: if `~/.spotifai/bin/zad` is missing or stale, the release tagged in `.zadrc` is downloaded into place. The command then ensures `~/.spotifai/permissions/<provider>/playlist.toml` exists (scaffolding it if not) and pins `ZAD_PERMISSIONS_PATH` at that file. The `playlist` profile is reused because its `allowed` list already covers `search`, `playlists list/show/create/add` — no separate profile is scaffolded or signed.

Status messages (`== spotifai import (Spotify) ==`, per-playlist results, the final summary line) always go to stderr.

## Input schema

`spotifai import` accepts the envelope shape produced by `spotifai export` (see [`export.md`](export.md) for the full schema). The discriminator is `source.service`: `"spotify"` exports use `playlists[].tracks`; `"ymusic"` exports use `playlists[].videos`. `import` routes on this field automatically.

| Required field | Notes |
|---|---|
| `schema_version` | Must be one of the importer's supported versions (currently `"1"`). Mismatch is a fatal error. |
| `source.service` | Identifies the source provider. Used to decide whether to reuse IDs or to resolve via search. |
| `playlists` | Array. Each entry must have a `name` or `title` and a `tracks` or `videos` list (may be empty). |

## Arguments

`spotifai import` takes no positional arguments.

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider to import the playlists onto. One of `spotify`, `ymusic`. |
| `--input PATH`, `-i PATH` | path | — | Read the envelope from this file. Without it, the envelope is read from stdin. |
| `--dry-run` | bool | false | Preview the import without making any zad write calls. The duplicate-name pre-fetch and any cross-provider search calls still run because both are read-only and produce a more realistic preview. |

## Environment variables

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_PROVIDER` | set | Set to the active provider slug for the duration of the import so any spotifai helper that consults the variable resolves to the same provider. Not propagated outside the process. |
| `SPOTIFAI_PROFILE`  | set | Set to `playlist` for the duration of the import so any spotifai helper that consults the variable resolves to the matching profile file. Not propagated outside the process. |
| `ZAD_PERMISSIONS_PATH` | set | Forwarded to each spawned zad child as `~/.spotifai/permissions/<provider>/playlist.toml`. zad ≥ 0.3.0 reads this variable as an explicit override that bypasses the cwd-derived project slug. |

The spawned zad processes otherwise inherit the current environment, so any variables zad itself consults (OAuth tokens, keychain hints, etc.) are honoured.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | Import finished. Per-playlist or per-track failures during the loop are non-fatal — they accumulate into the final summary line on stderr. |
| 1   | Fatal error: zad install failure, input read failure, JSON parse failure, unsupported `schema_version`, missing `source.service`, malformed `playlists` array, or existing-playlist pre-fetch failure. |
| 2   | Usage error parsing `spotifai import` flags. |

## Examples

Same-provider re-import from a saved file:

```sh
spotifai import --provider spotify --input ~/backups/spotify-2026-05.json
```

Cross-provider migration (Spotify → YouTube Music) via pipe:

```sh
spotifai export --provider spotify | spotifai import --provider ymusic
```

Preview a migration without writing anything to the target account:

```sh
spotifai export --provider spotify | spotifai import --provider ymusic --dry-run
```

Pre-filter the envelope before importing — only the playlists named "Focus" and "Drive":

```sh
spotifai export --provider spotify | \
  jq '.playlists |= map(select(.name == "Focus" or .name == "Drive"))' | \
  spotifai import --provider spotify
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`export.md`](export.md) — the envelope this command consumes
- [`api.md`](api.md) — the underlying `spotifai api` shim that the import drives
- [`playlist.md`](playlist.md) — the agent-driven write surface that shares the `playlist` permission profile
- [`install.md`](main.md#spotifai-install) — the install/version check the import runs first
