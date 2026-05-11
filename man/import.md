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

1. **Duplicate skip.** Before the loop, `spotifai import` fetches the user's existing playlists on the target (`playlists list`) once via the in-process zad library and caches their names. Any source playlist whose name (case-insensitive, trimmed) already exists on the target is **skipped with a warning**. This makes re-runs idempotent — a partial migration can be retried without producing duplicates of already-imported playlists.

2. **Track resolution.** When `source.service` in the envelope matches the target provider, every track's embedded `source_ids[<service>]` is reused verbatim. When the providers differ — i.e. a cross-provider migration — every track is re-resolved on the target via the typed `search` request: on Spotify, ISRC first (`q=isrc:<value>`) then a `<title> <primary artist>` text fallback; on YouTube Music, ISRC search is unsupported so the title + primary-artist fallback runs straight away. The first hit on the target wins. Items that match neither query are reported as unresolved and skipped.

3. **Playlist creation.** A new playlist is created via the typed `playlists create` request on the in-process zad facade. Spotify accepts `--name`; YouTube Music accepts `--title`; the right field is selected automatically per provider.

4. **Track addition.** Resolved IDs are added to the new playlist via the typed `playlists add` request, chunked at 50 items per call to match both Spotify and YouTube Music API page caps.

Failures inside one playlist (an unresolvable track, a single failed `playlists add` chunk, even a failed `playlists create`) accumulate into the final summary and **do not abort** the import. Re-running with the same envelope picks up where the previous run left off because already-imported playlists are skipped on the duplicate-name guard.

`spotifai import` is **scope-limited to playlists**. The envelope's top-level `tracks` (liked songs / liked videos) and `albums` (saved albums) buckets are intentionally ignored — recreating them would require widening the `playlist` permission profile (`library tracks save`, `library albums save`, `library like`), which is out of scope for migration. If you need them, run the `spotifai api` write verbs directly with a profile of your own.

`spotifai import` ensures `~/.spotifai/permissions/<provider>/playlist.toml` exists (scaffolding it if not) and pins `ZAD_PERMISSIONS_PATH` at that file. The `playlist` profile is reused because its `allowed` list already covers `search`, `playlists list/show/create/add` — no separate profile is scaffolded or signed.

Status messages (`== spotifai import (Spotify) ==`, per-playlist results, the final summary line) always go to stderr.

## Input schema

`spotifai import` accepts the envelope shape produced by `spotifai export` (see [`export.md`](export.md) and [`../docs/export_schema.md`](../docs/export_schema.md) for the full schema). The discriminator is `source.service`: when it matches `--provider`, embedded `source_ids[<service>]` are reused verbatim; otherwise every track is resolved on the target via search. Unlike previous releases, `tracks` always lives under `playlists[].tracks` regardless of source — the unified schema does not have a separate `videos` field.

| Required field | Notes |
|---|---|
| `schema_version` | Must be one of the importer's supported versions (currently `"1"`). Mismatch is a fatal error. |
| `source.service` | Identifies the source provider (`spotify` or `ymusic`). Used to decide whether to reuse `source_ids` or to resolve via search. |
| `playlists` | Array. Each entry must have a `name` and a `tracks` array (may be empty). |

## Arguments

`spotifai import` takes no positional arguments.

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider to import the playlists onto. One of `spotify`, `ymusic`. |
| `--input PATH`, `-i PATH` | path | — | Read the envelope from this file. Without it, the envelope is read from stdin. |
| `--dry-run` | bool | false | Preview the import without making any zad write calls. The duplicate-name pre-fetch and any cross-provider search calls still run because both are read-only and produce a more realistic preview. |

The global `--wait` / `--no-wait` flags (see [`main.md`](main.md)) also apply. `spotifai import` defaults to fail-fast (`--no-wait`); a cross-provider migration that resolves thousands of tracks may want `--wait` so an early 429 from the resolver doesn't abort the whole run.

## Environment variables

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_PROVIDER` | set | Set to the active provider slug for the duration of the import so any spotifai helper that consults the variable resolves to the same provider. Not propagated outside the process. |
| `SPOTIFAI_PROFILE`  | set | Set to `playlist` for the duration of the import so any spotifai helper that consults the variable resolves to the matching profile file. Not propagated outside the process. |
| `SPOTIFAI_WAIT` | read | Same semantics as for `spotifai api`: `1` → sleep through an active 429 cooldown window before each zad call; `0` → fail fast. Defaults to fail-fast for a direct `spotifai import` invocation. The CLI `--wait` / `--no-wait` flags override the env var. |
| `ZAD_PERMISSIONS_PATH` | set | Pinned to `~/.spotifai/permissions/<provider>/playlist.toml` for the duration of the import. zad's library-side trust check honours this variable as an explicit override that bypasses the cwd-derived project slug. |

OAuth tokens are read from the OS keychain by zad on every call; no environment variable is consulted for credentials.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | Import finished. Per-playlist or per-track failures during the loop are non-fatal — they accumulate into the final summary line on stderr. |
| 1   | Fatal error: input read failure, JSON parse failure, unsupported `schema_version`, missing `source.service`, malformed `playlists` array, or existing-playlist pre-fetch failure. |
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
- [`api.md`](api.md) — the typed-dispatch shim the import drives
- [`playlist.md`](playlist.md) — the agent-driven write surface that shares the `playlist` permission profile
- [`install.md`](install.md) — bootstraps the trust store and scaffolds the permissions files
- [`../docs/export_schema.md`](../docs/export_schema.md) — full schema reference for the envelope
