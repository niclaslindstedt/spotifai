# spotifai export

> Dump the user's library on the active provider into the unified spotifai JSON schema.

## Synopsis

```
spotifai export [--provider <slug>] [--output PATH] [--pretty]
```

## Description

`spotifai export` walks the user's library on the active provider (`--provider`, default `spotify`) and writes one JSON document containing every record needed to recreate the library elsewhere, folded into the **unified spotifai schema** defined in [`docs/export_schema.md`](../docs/export_schema.md). Mappers in `src/export_schema.rs` translate each provider's response shape into the schema's `Track` / `Album` / `Playlist` types so the envelope is identical across providers — `spotifai import` reads the same shape regardless of source.

For **Spotify**:

- **`tracks`** — every saved track, mapped to the `Track` schema. ISRC is preserved (`isrc`), `source_ids.spotify` carries the Spotify track id for byte-exact same-provider re-imports.
- **`albums`** — every saved album, mapped to the `Album` schema.
- **`playlists`** — every playlist the user owns or follows, each with its full ordered track list embedded under the same `Track` schema.

For **YouTube Music**:

- **`tracks`** — every "liked" video, mapped to the `Track` schema. The YouTube Data API does not expose ISRC, so cross-provider migrations fall back to `<title> <primary artist>` text search. `source_ids.ymusic` carries the YouTube video id for same-provider re-imports.
- **`albums`** — always `[]`. The YouTube Data API has no "saved albums" concept.
- **`playlists`** — every playlist the user owns or follows, each with its full ordered video list embedded under the same `Track` schema.

The command is deterministic — no LLM in the loop. Reads happen through the in-process zad library facades (`zad::service::spotify::Spotify`, `zad::service::ymusic::Ymusic`) plus a few low-level `SpotifyHttp` / `YmusicHttp` calls for verbs the typed facades do not yet expose.

`spotifai export` ensures `~/.spotifai/permissions/<provider>/ask.toml` exists (scaffolding it if not) and pins `ZAD_PERMISSIONS_PATH` at that file. The `ask` profile is reused because its `allowed` list already covers the read-only verbs the export needs; no separate profile is scaffolded or signed.

The JSON document goes to stdout by default, so `spotifai export | jq …` and `spotifai export > library.json` work as expected. Status messages (`== spotifai export (Spotify) ==`, the permissions banner, the per-step summary line) always go to stderr so they never contaminate the JSON pipeline. Pass `--output PATH` to write straight to a file; parent directories are created if needed.

### Pagination caveat (zad 0.8.0)

zad 0.8.0's typed facades cap most list endpoints at 50 items per call and do not yet expose `offset`. The export is therefore best-effort up to 50 saved tracks, 50 saved albums, 50 playlists, and 50 tracks per playlist. Heavier libraries are truncated and a warning is emitted to stderr per category that hit the cap. A future zad release that surfaces pagination will lift this transparently — no schema change required.

## Output schema

See [`docs/export_schema.md`](../docs/export_schema.md) for the authoritative reference. The top-level envelope:

```json
{
  "schema_version": "1",
  "exported_at": "2026-05-09T12:34:56Z",
  "source": {
    "service": "spotify",
    "user": { "id": "alice", "display_name": "Alice" },
    "tool": "spotifai",
    "tool_version": "0.1.0"
  },
  "tracks":    [ /* Track[] — liked songs / liked videos */ ],
  "albums":    [ /* Album[] — saved albums (Spotify only; [] on ymusic) */ ],
  "playlists": [ /* Playlist[] — each carries its own ordered Track[] */ ]
}
```

`schema_version` is bumped only when the envelope shape changes in a way an importer must care about. Additive changes (new optional fields) stay on `"1"`.

## Arguments

`spotifai export` takes no positional arguments.

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider whose library to export. One of `spotify`, `ymusic`. |
| `--output PATH`, `-o PATH` | path | — | Write the JSON document to this file instead of stdout. Parent directories are created if needed. |
| `--pretty` | bool | false | Pretty-print the JSON with two-space indent. Without this flag the document is one dense line, which is what most downstream tooling (importers, diffs) prefers. |

The global `--wait` / `--no-wait` flags (see [`main.md`](main.md)) also apply. `spotifai export` defaults to fail-fast (`--no-wait`) so a user-driven export surfaces 429s immediately instead of stalling silently; pass `--wait` when running concurrently with `spotifai ask` / `spotifai playlist` to share their cooldown coordination.

## Environment variables

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_PROVIDER` | set | Set to the active provider slug for the duration of the export so any spotifai helper that consults the variable resolves to the same provider the export is using. Not propagated outside the process. |
| `SPOTIFAI_PROFILE` | set | Set to `ask` for the duration of the export so any spotifai helper that consults the variable resolves to the same profile file the export uses. Not propagated outside the process. |
| `SPOTIFAI_WAIT` | read | Same semantics as for `spotifai api`: `1` → sleep through an active 429 cooldown window before each zad call; `0` → fail fast. Defaults to fail-fast for a direct `spotifai export` invocation. The CLI `--wait` / `--no-wait` flags override the env var. |
| `ZAD_PERMISSIONS_PATH` | set | Pinned to `~/.spotifai/permissions/<provider>/ask.toml` for the duration of the export. zad's library-side trust check honours this variable as an explicit override that bypasses the cwd-derived project slug. |

OAuth tokens are read from the OS keychain by zad on every call; no environment variable is consulted for credentials.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | Export succeeded; JSON written. |
| 1   | Generic error: zad library call failure, JSON serialization failure, missing home directory, or output write failure. |
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
spotifai export | jq '[.tracks[] | select(.isrc)] | length'
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`import.md`](import.md) — recreates playlists from the envelope produced here
- [`api.md`](api.md) — the typed-dispatch shim the export drives
- [`ask.md`](ask.md) — the read-only agent surface that shares the same permission profile
- [`install.md`](install.md) — bootstraps the trust store and scaffolds the permissions files
- [`../docs/export_schema.md`](../docs/export_schema.md) — full schema reference
