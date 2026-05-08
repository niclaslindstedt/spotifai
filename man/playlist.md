# spotifai playlist

> Start an interactive zag session that builds one new Spotify playlist for the user, with `~/.spotifai/permissions/playlist.toml` injected so the agent self-restricts.

## Synopsis

```
spotifai playlist [QUERY]...
```

## Description

`spotifai playlist` opens an interactive [zag](https://github.com/niclaslindstedt/zag) session. The session's system prompt:

1. Tells the agent that every Spotify interaction must go through the `spotifai api …` shell command (not the Spotify Web API directly), because that is where zad's scope and permissions are enforced.
2. Inlines the contents of `~/.spotifai/permissions/playlist.toml` so the agent knows which `spotifai api` verbs it is allowed to invoke.
3. Frames the task as a one-shot playlist build: search the catalogue, pick tracks, create a new playlist, add the tracks, and optionally rename it. Destructive verbs (`playlists delete`, `playlists remove`) and library writes stay denied.

Before starting zag, `spotifai playlist` runs the same install/version check as `spotifai install` to make sure the pinned `~/.spotifai/bin/zad` binary is on disk, and writes a default `playlist` permissions file if none exists yet.

The optional positional argument becomes the agent's first turn — usually a brief like `"a 30-minute focus playlist with no vocals"`. With no argument, the session opens empty and waits for the user to type. Quit with `Ctrl+D` or whatever exit gesture the active zag provider uses.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `[QUERY]...` | trailing | — | Optional brief. Joined with spaces and used as the agent's first turn. |

## Flags

`spotifai playlist` itself takes no flags. zag's own flags are not exposed today — configure zag through its own config files (`~/.zag/...`) instead.

## Environment variables

`spotifai playlist` reads no environment variables of its own. zag and its underlying provider (Claude / Codex / Gemini / Copilot / Ollama) inherit the parent environment, so any variables they consult (`ANTHROPIC_API_KEY`, etc.) are honoured.

## Permissions

The injected policy lives at `~/.spotifai/permissions/playlist.toml`. On first install:

- **Allowed**: `search`, `playlists list`, `playlists show`, `playlists create`, `playlists add`, `playlists rename`, `library tracks list`, `library albums list`.
- **Denied**: `playlists delete`, `playlists remove`, `library tracks save|unsave`, `library albums save|unsave`.

The `playlist` profile is independent of the `ask` profile (`~/.spotifai/permissions/ask.toml`) — edits to one do not affect the other. To narrow or widen `playlist`, hand-edit `allowed` / `denied` and re-run `spotifai install` to resign the file; spotifai re-reads it on every `spotifai playlist` invocation. The agent is forbidden in the system prompt from editing the file or invoking `zad spotify permissions` itself, so widening always requires a deliberate human edit. The permissions file is **advisory** — it constrains the agent via prompt injection but is not enforced by zad. zad's own runtime gate continues to be the file at `~/.zad/services/spotify/permissions.toml`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | zag session ended cleanly. |
| 1 | Generic spotifai error (zad install failure, missing home directory, permissions parse error, runtime build failure). |
| 2 | Usage error parsing `spotifai playlist` itself. |
| *N* | Any other code is propagated from zag's terminal exit. |

## Examples

Open the session with a brief and let the agent take it from there:

```sh
spotifai playlist "a 30-minute focus playlist with no vocals"
```

Open the session empty and chat:

```sh
spotifai playlist
```

Quote when the brief contains shell-special characters:

```sh
spotifai playlist "tracks like 'kind of blue', but vocal jazz"
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`ask.md`](ask.md) — read-only counterpart for questions about your library
- [`api.md`](api.md) — the forward-routing shim the agent uses to talk to Spotify
- [`spotifai install`](main.md#spotifai-install) — installs zad and scaffolds the permissions files
