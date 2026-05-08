# spotifai export

> Dump the user's library on the active provider into one structured JSON document.

## Synopsis

```
spotifai export [--provider <slug>] [--output PATH] [--pretty]
```

## Description

`spotifai export` walks the user's library on the active provider (`--provider`, default `spotify`) and writes one JSON document containing every record needed to recreate the library elsewhere.

For **Spotify**:

- **Liked tracks** — every track the user has saved (`zad spotify library tracks list`), in the order Spotify returns them.
- **Saved albums** — every album the user has saved (`zad spotify library albums list`).
- **Playlists** — every playlist the user owns or follows (`zad spotify playlists list`), each with its full ordered track list (`zad spotify playlists show <id>`).

For **YouTube Music**:

- **Liked videos** — every video the user has rated as "like" (`zad ymusic library list`), in the order the API returns them. The YouTube Data API has no "saved albums" concept, so the album bucket is intentionally absent.
- **Playlists** — every playlist the user owns or follows (`zad ymusic playlists list`), each with its full ordered video list (`zad ymusic playlists show <id>`).

The command is deterministic — no LLM in the loop. Each list endpoint is paged with `--limit 50 --offset N` until a short page comes back, and each playlist is fetched individually for its tracks/videos. Records are embedded **verbatim** under the envelope, so any identifier zad already exposes (`isrc`, `spotify_id`, `video_id`, `added_at`, position, duration, etc.) flows through without spotifai having to track zad's schema. Importers route on the envelope's `source.service` field.

Before fetching, spotifai performs the same install/version check as `spotifai install`: if `~/.spotifai/bin/zad` is missing or stale, the release tagged in `.zadrc` is downloaded into place. The command then ensures `~/.spotifai/permissions/<provider>/ask.toml` exists (scaffolding it if not) and pins `ZAD_PERMISSIONS_PATH` at that file. The `ask` profile is reused because its `allowed` list already covers the read-only verbs the export needs; no separate profile is scaffolded or signed.

The JSON document goes to stdout by default, so `spotifai export | jq …` and `spotifai export > library.json` work as expected. Status messages (`== spotifai export (Spotify) ==`, per-step counts, the final summary line) always go to stderr so they never contaminate the JSON pipeline. Pass `--output PATH` to write straight to a file; parent directories are created if needed.

## Output schema

Spotify export:

```json
{
  "schema_version": "1",
  "exported_at": "2026-05-08T12:34:56Z",
  "source": {
    "service": "spotify",
    "tool": "spotifai",
    "tool_version": "0.1.0"
  },
  "liked_tracks": [ <verbatim items from `zad spotify library tracks list --json`> ],
  "saved_albums": [ <verbatim items from `zad spotify library albums list --json`> ],
  "playlists": [
    {
      <verbatim metadata from `zad spotify playlists show <id> --json`>,
      "tracks": [ <verbatim items, in order> ]
    }
  ]
}
```

YouTube Music export:

```json
{
  "schema_version": "1",
  "exported_at": "2026-05-08T12:34:56Z",
  "source": {
    "service": "ymusic",
    "tool": "spotifai",
    "tool_version": "0.1.0"
  },
  "liked_videos": [ <verbatim items from `zad ymusic library list --json`> ],
  "playlists": [
    {
      <verbatim metadata from `zad ymusic playlists show <id> --json`>,
      "videos": [ <verbatim items, in order> ]
    }
  ]
}
```

`schema_version` is bumped only when the envelope shape changes in a way an importer must care about. Additive changes (new fields zad starts emitting, new optional envelope fields) stay on `"1"`. Adding a new provider with a brand-new bucket is treated as additive — existing importers keep working by branching on `source.service`.

## Arguments

`spotifai export` takes no positional arguments.

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider whose library to export. One of `spotify`, `ymusic`. |
| `--output PATH`, `-o PATH` | path | — | Write the JSON document to this file instead of stdout. Parent directories are created if needed. |
| `--pretty` | bool | false | Pretty-print the JSON with two-space indent. Without this flag the document is one dense line, which is what most downstream tooling (importers, diffs) prefers. |

## Environment variables

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_PROVIDER` | set | Set to the active provider slug for the duration of the export so any spotifai helper that consults the variable resolves to the same provider the export is using. Not propagated outside the process. |
| `SPOTIFAI_PROFILE` | set | Set to `ask` for the duration of the export so any spotifai helper that consults the variable resolves to the same profile file the export uses. Not propagated outside the process. |
| `ZAD_PERMISSIONS_PATH` | set | Forwarded to each spawned zad child as `~/.spotifai/permissions/<provider>/ask.toml`. zad ≥ 0.3.0 reads this variable as an explicit override that bypasses the cwd-derived project slug. |

The spawned zad processes otherwise inherit the current environment, so any variables zad itself consults (OAuth tokens, keychain hints, etc.) are honoured.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | Export succeeded; JSON written. |
| 1   | Generic error: zad install failure, zad subprocess failure, JSON parse failure, missing home directory, or output write failure. |
| 2   | Usage error parsing `spotifai export` flags. |

A failure mid-way through the fetch loop aborts the export — the partial JSON is **not** flushed to disk or stdout. Re-run the command to retry from scratch.

## Examples

Pipe a Spotify export directly into `jq`:

```sh
spotifai export | jq '.playlists | length'
```

Write a pretty-printed YouTube Music snapshot to disk:

```sh
spotifai export --provider ymusic --pretty -o ~/backups/ymusic-2026-05.json
```

Inspect what came back without committing the file:

```sh
spotifai export --pretty | head -40
```

Quick check of the export's identifier coverage (how many liked Spotify tracks have ISRCs):

```sh
spotifai export | jq '[.liked_tracks[] | select(.isrc)] | length'
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`api.md`](api.md) — the underlying `spotifai api` shim that the export drives
- [`ask.md`](ask.md) — the read-only agent surface that shares the same permission profile
- [`install.md`](main.md#spotifai-install) — the install/version check the export runs first
