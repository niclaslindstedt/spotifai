# spotifai export schema

`spotifai export` writes a JSON document with the same shape
regardless of the source provider. `spotifai import` reads the same
shape regardless of the target provider. Mappers in
[`src/export_schema.rs`](../src/export_schema.rs) fold provider-
specific responses (Spotify Web API, YouTube Data API v3) into the
unified types defined here.

The schema is the **public contract** between exporter and importer
— hand-edited envelopes that follow this layout import as cleanly as
machine-generated ones do.

## Goals

- Spotify → spotifai → Spotify roundtrips are exact (same-provider
  re-imports use `source_ids[<service>]` for byte-exact matching).
- Spotify → spotifai → YouTube Music migrations resolve every track
  through search (ISRC first, title + primary artist fallback) using
  the *same* code path the YouTube → Spotify direction uses.
- A future Apple Music / Tidal / Deezer backend lands by adding one
  mapper pair and one slug — the schema does not move.

## Top-level envelope

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
  "tracks": [ /* Track[] — liked songs / liked videos */ ],
  "albums": [ /* Album[] — saved albums (Spotify only) */ ],
  "playlists": [ /* Playlist[] */ ]
}
```

| Field            | Type     | Notes                                                                         |
| ---------------- | -------- | ----------------------------------------------------------------------------- |
| `schema_version` | string   | Currently `"1"`. Bumped on breaking changes; importers reject other values.   |
| `exported_at`    | string   | ISO 8601 UTC timestamp at the moment the export ran.                          |
| `source.service` | string   | Lowercase service slug. One of `"spotify"`, `"ymusic"`.                       |
| `source.user`    | object?  | Authenticated user identity at the source provider; see [User](#user).        |
| `source.tool`    | string   | Always `"spotifai"`.                                                          |
| `source.tool_version` | string | `Cargo.toml` version of the spotifai build that produced the envelope.    |
| `tracks`         | Track[]  | Liked songs (Spotify) and liked videos (YouTube Music), unified.              |
| `albums`         | Album[]  | Saved albums (Spotify). Empty `[]` on YouTube Music exports.                  |
| `playlists`      | Playlist[] | User's playlists, each with the full ordered track list embedded.           |

## Track

```json
{
  "title": "Billie Jean",
  "artists": ["Michael Jackson"],
  "album": "Thriller",
  "duration_ms": 293826,
  "isrc": "USRC17607839",
  "added_at": "2024-05-01T12:00:00Z",
  "source_ids": { "spotify": "abc" },
  "raw": { /* verbatim source JSON, kept for diagnostics */ }
}
```

| Field         | Type        | Notes                                                                                  |
| ------------- | ----------- | -------------------------------------------------------------------------------------- |
| `title`       | string      | Display title.                                                                          |
| `artists`     | string[]    | Display order. Always plain strings, never nested objects.                              |
| `album`       | string?     | Optional. YouTube Music videos generally have no album.                                |
| `duration_ms` | u64?        | Optional. Spotify tracks; YouTube Music videos do not surface this in current zad. |
| `isrc`        | string?     | International Standard Recording Code. Spotify exposes it; YouTube does not. When present this is the canonical cross-provider identifier — importers try `isrc:<value>` first.        |
| `added_at`    | string?     | ISO 8601 UTC. Filled by Spotify saved-tracks (the user's library).                     |
| `source_ids`  | { [service]: id } | Per-provider identifier. Keys are lowercase service slugs (`"spotify"`, `"ymusic"`); values are the provider's item id. Used for same-provider re-imports — skips search entirely. |
| `raw`         | any?        | Verbatim record from the source provider. Not consumed by `spotifai import` — kept for diagnostics and provider-specific tooling. |

The same `Track` shape is used at the top level (liked items) **and**
inside each `Playlist.tracks`. There is no separate type for "track
inside a playlist".

## Album

```json
{
  "title": "Thriller",
  "artists": ["Michael Jackson"],
  "total_tracks": 9,
  "release_date": "1982-11-30",
  "added_at": "2023-12-01T08:00:00Z",
  "source_ids": { "spotify": "1ATL5GLyefJaxhQzSPVrLX" },
  "raw": { /* … */ }
}
```

YouTube Music has no "saved albums" concept; ymusic exports always
have `albums: []`.

## Playlist

```json
{
  "name": "Focus",
  "description": "Heads-down work playlist",
  "public": false,
  "owner": { "id": "alice", "display_name": "Alice" },
  "tracks": [ /* Track[] — ordered */ ],
  "source_ids": { "spotify": "37i9dQZF1DWWQRwui0ExPn" },
  "raw": { /* … */ }
}
```

| Field         | Type      | Notes                                                                |
| ------------- | --------- | -------------------------------------------------------------------- |
| `name`        | string    | Display name.                                                        |
| `description` | string?   | Optional.                                                            |
| `public`      | bool?     | Carries through to the import (a public playlist on the source becomes a public playlist on the target). |
| `owner`       | [User](#user)? | Owner at the source. Most users only export playlists they own; this also captures playlists they followed but did not author. |
| `tracks`      | Track[]   | Ordered list. Importers preserve the order.                          |
| `source_ids`  | { [service]: id } | Per-provider playlist id. Today the importer treats every entry as a fresh playlist; the field is kept so a future "update existing playlist" mode can find the right target. |
| `raw`         | any?      | Verbatim source record.                                              |

## User

```json
{ "id": "alice", "display_name": "Alice" }
```

| Field          | Type    | Notes                                                                |
| -------------- | ------- | -------------------------------------------------------------------- |
| `id`           | string  | Provider-specific identifier. Spotify user id, YouTube channel id.   |
| `display_name` | string? | Optional human-friendly label.                                       |

## Cross-provider conversion semantics

`spotifai import --provider <target>` checks whether
`source.service == <target>`:

- **Same-provider** (Spotify → Spotify, YouTube → YouTube): each
  track's `source_ids[<service>]` is used verbatim. No search calls.
- **Cross-provider** (Spotify → YouTube Music, YouTube → Spotify):
  every track is resolved against the target via search.

  - Spotify-target: try `isrc:<isrc>` first, fall back to
    `<title> <primary artist>`. Both queries take the first hit.
  - YouTube-target: ISRC search is unsupported; fall straight to
    `<title> <primary artist>`. Take the first video hit.

  Tracks that fail to resolve are accumulated in the import report
  (`tracks_unresolved`) and skipped. Playlists are never aborted by
  unresolved tracks.

## Pagination caveat (zad 0.6.4)

zad 0.6.4's typed facades cap most list endpoints at 50 items and
do not yet expose `offset`. The export is therefore best-effort up
to 50 saved tracks, 50 saved albums, 50 playlists, and 50 tracks
per playlist. Heavier libraries are truncated and a warning is
emitted to stderr per category that hit the cap. A future zad
release that surfaces pagination will lift this transparently — no
schema change required.

## Round-trip example

```sh
# Export from Spotify
spotifai export --provider spotify --output spotify.json

# Re-import on Spotify (same-provider, exact match via source_ids)
spotifai import --provider spotify --input spotify.json

# Migrate to YouTube Music (cross-provider, ISRC + text search)
spotifai import --provider ymusic --input spotify.json
```

The third command never touches Spotify; the second never touches
YouTube. Both consume the same envelope file.
