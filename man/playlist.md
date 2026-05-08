# spotifai playlist

> Start an interactive zag session that builds one new playlist for the user on the active provider, with `~/.spotifai/permissions/<provider>/playlist.toml` injected so the agent self-restricts.

## Synopsis

```
spotifai playlist [--provider <slug>] [QUERY]...
```

## Description

`spotifai playlist` opens an interactive [zag](https://github.com/niclaslindstedt/zag) session. The session's system prompt:

1. Tells the agent that every interaction must go through the `spotifai api …` shell command (not the provider's API directly), because that is where zad's scope and permissions are enforced.
2. Inlines the contents of `~/.spotifai/permissions/<provider>/playlist.toml` so the agent knows which `spotifai api` verbs it is allowed to invoke.
3. Frames the task as a one-shot playlist build: search the catalogue, pick tracks/videos, create a new playlist, add the items, and optionally rename it. Destructive verbs (`playlists delete`, `playlists remove`) and library writes stay denied.

Before starting zag, `spotifai playlist` runs the same install/version check as `spotifai install` to make sure the pinned `~/.spotifai/bin/zad` binary is on disk, and writes a default `playlist` permissions file for the active provider if none exists yet.

The optional positional argument becomes the agent's first turn — usually a brief like `"a 30-minute focus playlist with no vocals"`. With no argument, the session opens empty and waits for the user to type. Quit with `Ctrl+D` or whatever exit gesture the active zag provider uses.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider for the new playlist. One of `spotify`, `ymusic`. |
| `[QUERY]...` | trailing | — | Optional brief. Joined with spaces and used as the agent's first turn. |

## Flags

`spotifai playlist` owns `--provider`. zag's own flags are not exposed today — configure zag through its own config files (`~/.zag/...`) instead.

## Environment variables

`spotifai playlist` reads no environment variables of its own. `spotifai playlist` *sets* `SPOTIFAI_PROVIDER` and `SPOTIFAI_PROFILE` on its own process so child `spotifai api` shells route to the same `(provider, playlist)` policy file the prompt was rendered with.

## Permissions

The injected policy lives at `~/.spotifai/permissions/<provider>/playlist.toml`. On first install:

- **Spotify allowed**: `search`, `playlists list`, `playlists show`, `playlists create`, `playlists add`, `playlists rename`, `library tracks list`, `library albums list`.
- **Spotify denied**: `playlists delete`, `playlists remove`, `library tracks save|unsave`, `library albums save|unsave`.
- **YouTube Music allowed**: `search`, `playlists list`, `playlists show`, `playlists create`, `playlists add`, `playlists rename`, `library list`.
- **YouTube Music denied**: `playlists delete`, `playlists remove`, `library like`, `library unlike`.

The `playlist` profile is independent of the `ask` profile. To narrow or widen `playlist`, hand-edit `allowed` / `denied` and re-run `spotifai install` to resign the file; spotifai re-reads it on every `spotifai playlist` invocation. The agent is forbidden in the system prompt from editing the file or invoking `zad <provider> permissions` itself, so widening always requires a deliberate human edit. The permissions file is **advisory** — it constrains the agent via prompt injection but is not enforced by zad. zad's own runtime gate continues to be the file at `~/.zad/services/<provider>/permissions.toml`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | zag session ended cleanly. |
| 1 | Generic spotifai error (zad install failure, missing home directory, permissions parse error, runtime build failure). |
| 2 | Usage error parsing `spotifai playlist` itself. |
| *N* | Any other code is propagated from zag's terminal exit. |

## Examples

Open the Spotify session with a brief and let the agent take it from there:

```sh
spotifai playlist "a 30-minute focus playlist with no vocals"
```

Build a playlist on YouTube Music:

```sh
spotifai playlist --provider ymusic "an upbeat 45-minute commute playlist"
```

Open the session empty and chat:

```sh
spotifai playlist
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`ask.md`](ask.md) — read-only counterpart for questions about your library
- [`api.md`](api.md) — the forward-routing shim the agent uses
- [`spotifai install`](main.md#spotifai-install) — installs zad and scaffolds the permissions files
