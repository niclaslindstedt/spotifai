# spotifai ask

> Start an interactive zag session pre-loaded to query the user's library on the active provider through `spotifai api …`, with the local permissions file injected so the agent self-restricts.

## Synopsis

```
spotifai ask [--provider <slug>] [QUERY]...
```

## Description

`spotifai ask` opens an interactive [zag](https://github.com/niclaslindstedt/zag) session. The session's system prompt:

1. Tells the agent that every interaction must go through the `spotifai api …` shell command (not the provider's API directly), because that is where zad's scope and permissions are enforced.
2. Inlines the contents of `~/.spotifai/permissions/<provider>/ask.toml` so the agent knows which `spotifai api` verbs it is allowed to invoke.
3. Substitutes provider-specific example calls (Spotify uses `library tracks/albums list`; YouTube Music uses `library list` over rated videos), so the agent does not propose verbs that do not exist on the active provider.

Before starting zag, `spotifai ask` ensures `~/.spotifai/permissions/<provider>/ask.toml` exists (scaffolding it with the default read-only policy if not). The zad library is consumed in-process — there is no separate binary to install — so no version check runs here.

The optional positional argument becomes the agent's first turn. With no argument, the session opens empty and waits for the user to type. Quit with `Ctrl+D` or whatever exit gesture the active zag provider uses.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider to query. One of `spotify`, `ymusic`. |
| `[QUERY]...` | trailing | — | Optional opening question. Joined with spaces and used as the agent's first turn. |

## Flags

`spotifai ask` owns `--provider`. zag's own flags are not exposed today — configure zag through its own config files (`~/.zag/...`) instead.

## Environment variables

`spotifai ask` reads no environment variables of its own. zag and its underlying provider (Claude / Codex / Gemini / Copilot / Ollama) inherit the parent environment, so any variables they consult (`ANTHROPIC_API_KEY`, etc.) are honoured. `spotifai ask` *sets* `SPOTIFAI_PROVIDER` and `SPOTIFAI_PROFILE` on its own process so child `spotifai api` shells route to the same `(provider, profile)` policy file the prompt was rendered with.

## Permissions

The injected policy lives at `~/.spotifai/permissions/<provider>/ask.toml`. On first install it's read-only:

- **Spotify allowed**: `search`, `playlists list`, `playlists show`, `library tracks list`, `library albums list`.
- **Spotify denied**: every mutating verb (`playlists create|rename|delete|add|remove`, `library tracks save|unsave`, `library albums save|unsave`).
- **YouTube Music allowed**: `search`, `playlists list`, `playlists show`, `library list`.
- **YouTube Music denied**: every mutating verb (`playlists create|rename|delete|add|remove`, `library like|unlike`).

The `ask` profile is independent of the `playlist` profile — edits to one do not affect the other. To widen `ask` past read-only, hand-edit `allowed` / `denied` and re-run `spotifai install` to resign the file; spotifai re-reads it on every `spotifai ask` invocation. The agent is forbidden in the system prompt from editing the file itself, so widening always requires a deliberate human edit. The permissions file is **advisory** — it constrains the agent via prompt injection — but zad's library-side trust check at load time is the authoritative gate: the file is rejected if its signature is not in `~/.zad/signing/trusted.toml`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | zag session ended cleanly. |
| 1 | Generic spotifai error (missing home directory, permissions parse error, tokio runtime build failure, prompt rendering failure). |
| 2 | Usage error parsing `spotifai ask` itself. |
| *N* | Any other code is propagated from zag's terminal exit. |

## Examples

Open the Spotify session with no opener and start typing:

```sh
spotifai ask
```

Ask a question against the YouTube Music library in one go:

```sh
spotifai ask --provider ymusic "what playlists do I have?"
```

Quote when the question contains shell-special characters:

```sh
spotifai ask "show me my 'on repeat' playlist"
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`api.md`](api.md) — the typed-dispatch shim the agent uses
- [`playlist.md`](playlist.md) — write-side counterpart for building new playlists
- [`install.md`](install.md) — bootstraps the trust store and scaffolds the permissions files
