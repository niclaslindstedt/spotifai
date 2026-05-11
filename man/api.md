# spotifai api

> Dispatch a typed call into the in-process zad library and print the JSON response.

## Synopsis

```
spotifai api [ARGS]...
```

## Description

`spotifai api` parses everything after `api` as a typed verb in the zad-facing grammar (`search "query"`, `playlists list`, `playlists show <id>`, `playlists create --name|--title <name>`, `playlists add <playlist-id> <id…>`, `library tracks list`, `library albums list` on Spotify, `library list` on YouTube Music). The verb is dispatched **in-process** through the `zad` Rust library: Spotify calls go through `zad::service::spotify::Spotify` (with `SpotifyHttp` for raw verbs the typed facade does not yet expose); YouTube Music calls go through `zad::service::ymusic::Ymusic` / `YmusicHttp`. The response is serialized to pretty JSON on stdout.

`spotifai api` requires a parent spotifai command to have selected a permission profile. `spotifai ask`, `spotifai playlist`, `spotifai export`, and `spotifai import` set `SPOTIFAI_PROFILE` (`ask` or `playlist`) and `SPOTIFAI_PROVIDER` (the provider slug) before launching zag; child shells then inherit both variables when the agent runs `spotifai api …`. Direct invocations from a user shell exit with a usage error (code `2`) if `SPOTIFAI_PROFILE` is unset — there is intentionally no implicit default for the profile axis. `SPOTIFAI_PROVIDER` falls back to `spotify` when unset, for backwards compatibility with shells written against the original Spotify-only `spotifai`.

For each call, `spotifai api` pins `ZAD_PERMISSIONS_PATH` at the matching `~/.spotifai/permissions/<provider>/<profile>.toml`. zad's library-side trust check verifies the file is in the per-machine trust store at `~/.zad/signing/trusted.toml` and that the verb is in the policy. spotifai overrides any inherited `ZAD_PERMISSIONS_PATH` so an agent cannot escalate by setting the variable itself before invoking the shim.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `[ARGS]...` | trailing | — | The verb plus its arguments. Hyphen-prefixed flags (`--name`, `--title`, `--limit`, …) are accepted; use `--` to defensively split spotifai's args from the verb's. |

## Flags

`spotifai api` itself takes no positional flags. Anything that looks like a flag after the `api` keyword is parsed as part of the verb.

There is intentionally **no** `--provider` flag on `api`: clap's trailing-var-arg parsing would swallow it. Use `SPOTIFAI_PROVIDER` (or, more typically, just the parent `--provider` flag on `ask` / `playlist` / `export` / `import`).

The global `--wait` / `--no-wait` flags (see [`main.md`](main.md)) apply when placed *before* the `api` keyword (`spotifai --wait api playlists list`). They are also picked up from the `SPOTIFAI_WAIT` env var that `spotifai ask` / `spotifai playlist` set on the user's behalf, so a sub-agent's `spotifai api …` shell inherits the policy without anyone having to thread it through the argv. Default for direct invocations is fail-fast (`--no-wait`).

### Verb flags

`search`:

| Flag | Type | Default | Description |
|---|---|---|---|
| `--type / -t <kind>` | repeated string | `track` | Item kinds to search (`track`, `album`, `artist`, `playlist` on Spotify; `video`, `playlist`, `channel` on YouTube Music — `track` maps to `video`, `artist` to `channel`). |
| `--limit / -l <N>` | integer 1–10 | `10` | Page size. Spotify's `/search` caps `limit` at 10. |
| `--fields / -f <list>` | comma-separated, repeatable | (all) | Project each result item down to just the named fields. Aliases: `title→name`, `artist→artists` (joined to a comma-separated string), `album→album.name`, `duration→duration_ms`. Unknown names fall through to a raw object-key lookup. Use this to slash token cost when an agent consumes the result. |
| `--format <json\|text>` | enum | `json` | `json` keeps the existing pretty-printed envelope; `text` emits one item per line with the requested fields tab-separated in the order given. `--format text` requires `--fields`. |
| `--json` / `--pretty` | flag | — | Legacy no-ops that select JSON output. Prefer `--format json`. |

`playlists show`:

| Flag | Type | Default | Description |
|---|---|---|---|
| `--limit / -l <N>` | integer 1–50 | `50` | How many tracks of the playlist to fetch. |
| `--fields / -f <list>` | comma-separated, repeatable | (all) | Project each track down to just the named fields. Same aliases as `search` — `title`, `artist`, `album`, `id`, `uri`, `duration`. For Spotify the projector transparently unwraps the `{item: <track>, added_at: ...}` envelope each playlist-track is wrapped in; for YouTube Music it resolves `id` to the underlying `contentDetails.videoId` rather than the playlist-item record id. |
| `--format <json\|text>` | enum | `json` | `json` keeps the existing pretty-printed envelope; `text` emits one track per line with the requested fields tab-separated in the order given. `--format text` requires `--fields`. |
| `--json` / `--pretty` | flag | — | Legacy no-ops that select JSON output. Prefer `--format json`. |

Other verbs (`playlists list/create/add`, `library …`) accept `--limit`, `--json`, and `--pretty` only.

## Environment variables

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_PROVIDER` | read | Selects which zad service facade (`Spotify` / `Ymusic` / …) and which `~/.spotifai/permissions/<provider>/` directory the call routes through. Set by `spotifai ask` / `playlist` / `export` / `import` on the user's behalf. Unset is treated as `spotify` for backwards compatibility; an unknown value fails with a usage error. |
| `SPOTIFAI_PROFILE` | read | Selects which `<profile>.toml` to point zad at. Set by the parent surface (`ask` for read-only, `playlist` for the curator profile) on the user's behalf. Treated as an internal coupling, not a user knob: missing or unknown values fail with a usage error. |
| `SPOTIFAI_WAIT` | read | Controls how `spotifai api` reacts when zad 0.8.0's shared rate-limit deadline file (`~/.zad/state/<service>/rate_limit.json`) shows the active provider is still in a 429 cooldown window. `1`/`true`/`yes`/`on` → sleep until the deadline and continue; `0`/`false`/`no`/`off` → return an error wrapping `zad::ZadError::RateLimited` so the caller fails fast. Set on the user's behalf by `spotifai ask` / `spotifai playlist` so sub-agent fan-outs coordinate on one quota. The CLI `--wait` / `--no-wait` flags override the env var. |
| `ZAD_PERMISSIONS_PATH` | set | Set on the in-process zad call to `~/.spotifai/permissions/<provider>/<profile>.toml`. Always overrides any inherited value so an agent cannot escalate by setting the zad variable itself before invoking the shim. zad ≥ 0.3.0 reads this variable as an explicit override that bypasses the cwd-derived project slug. |

OAuth tokens are read from the OS keychain by zad on every call; no environment variable is consulted for credentials.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | The dispatched zad call succeeded; JSON written to stdout. |
| 1   | Generic spotifai error: verb parse failure, missing `~/.spotifai/permissions/<provider>/<profile>.toml`, missing home directory, tokio runtime failure, or a zad library error (HTTP failure, OAuth refresh failure, schema violation). |
| 2   | `SPOTIFAI_PROFILE` is unset / unknown. |

## Examples

List your playlists (active provider; defaults to Spotify):

```sh
SPOTIFAI_PROFILE=ask spotifai api playlists list
```

Search the catalogue:

```sh
SPOTIFAI_PROFILE=ask spotifai api search "billie jean"
```

Project a search down to a few fields (small JSON):

```sh
SPOTIFAI_PROFILE=ask spotifai api search "billie jean" \
    --fields title,artist,album,id
```

Drop the JSON envelope entirely — one item per line, tab-separated — for the cheapest agent-readable shape:

```sh
SPOTIFAI_PROFILE=ask spotifai api search "billie jean" \
    --fields title,artist,id --format text
```

Read an existing playlist's tracks in the same compact shape:

```sh
SPOTIFAI_PROFILE=ask spotifai api playlists show 37i9dQZF1DXcBWIGoYBM5M \
    --fields title,artist,id --format text
```

Drive a YouTube Music call directly (rare; usually go through `spotifai ask --provider ymusic`):

```sh
SPOTIFAI_PROVIDER=ymusic SPOTIFAI_PROFILE=ask spotifai api playlists list
```

Create a new playlist on Spotify (requires the `playlist` profile):

```sh
SPOTIFAI_PROFILE=playlist spotifai api playlists create --name "Focus"
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`auth.md`](auth.md) — register OAuth credentials before running `api …`
- [`ask.md`](ask.md) — read-only agent surface that drives `spotifai api` for you
- [`playlist.md`](playlist.md) — playlist-builder agent surface
- [`install.md`](install.md) — bootstraps the trust store and the per-(provider, profile) permissions files
