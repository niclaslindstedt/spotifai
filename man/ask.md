# spotifai ask

> Start an interactive zag session pre-loaded to query the user's Spotify library through `spotifai api …`, with the local permissions file injected so the agent self-restricts.

## Synopsis

```
spotifai ask [QUERY]...
```

## Description

`spotifai ask` opens an interactive [zag](https://github.com/niclaslindstedt/zag) session. The session's system prompt:

1. Tells the agent that every Spotify interaction must go through the `spotifai api …` shell command (not the Spotify Web API directly), because that is where zad's scope and permissions are enforced.
2. Inlines the contents of `~/.spotifai/permissions.toml` so the agent knows which `spotifai api` verbs it is allowed to invoke.

Before starting zag, `spotifai ask` runs the same install/version check as `spotifai install` to make sure the pinned `~/.spotifai/bin/zad` binary is on disk, and writes a default read-only permissions file if none exists yet.

The optional positional argument becomes the agent's first turn. With no argument, the session opens empty and waits for the user to type. Quit with `Ctrl+D` or whatever exit gesture the active zag provider uses.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `[QUERY]...` | trailing | — | Optional opening question. Joined with spaces and used as the agent's first turn. |

## Flags

`spotifai ask` itself takes no flags. zag's own flags are not exposed today — configure zag through its own config files (`~/.zag/...`) instead.

## Environment variables

`spotifai ask` reads no environment variables of its own. zag and its underlying provider (Claude / Codex / Gemini / Copilot / Ollama) inherit the parent environment, so any variables they consult (`ANTHROPIC_API_KEY`, etc.) are honoured.

## Permissions

The injected policy lives at `~/.spotifai/permissions.toml`. On first install it's read-only:

- **Allowed**: `search`, `playlists list`, `playlists show`, `library tracks list`, `library albums list`.
- **Denied**: every mutating verb (`playlists create|rename|delete|add|remove`, `library tracks save|unsave`, `library albums save|unsave`).

Hand-edit the file to widen the surface; spotifai re-reads it on every `spotifai ask` invocation. The agent is forbidden in the system prompt from editing the file or invoking `zad spotify permissions` itself, so widening always requires a deliberate human edit. The permissions file is **advisory** — it constrains the agent via prompt injection but is not enforced by zad. zad's own runtime gate continues to be the file at `~/.zad/services/spotify/permissions.toml`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | zag session ended cleanly. |
| 1 | Generic spotifai error (zad install failure, missing home directory, permissions parse error, runtime build failure). |
| 2 | Usage error parsing `spotifai ask` itself. |
| *N* | Any other code is propagated from zag's terminal exit. |

## Examples

Open the session with no opener and start typing:

```sh
spotifai ask
```

Ask about the library in one go:

```sh
spotifai ask What kinds of music do I listen to most?
```

Quote when the question contains shell-special characters:

```sh
spotifai ask "show me my 'on repeat' playlist"
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`api.md`](api.md) — the forward-routing shim the agent uses to talk to Spotify
- [`spotifai install`](main.md#spotifai-install) — installs zad and scaffolds the permissions file
