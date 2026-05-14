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

`spotifai ask` owns `--provider`. The global `--wait` / `--no-wait` flags (see [`main.md`](main.md)) also apply — `spotifai ask` defaults to `--wait` so sub-agents coordinate on shared rate-limit cooldowns. The global `--yolo` flag is also honoured: it forwards `auto_approve(true)` to the underlying zag `AgentBuilder` so the session skips every per-tool approval prompt. The `(provider, profile)` policy file at `~/.spotifai/permissions/<provider>/ask.toml` is still enforced by `spotifai api` at the zad layer, so `--yolo` cannot widen the allowed verb list — it only suppresses zag's tool-approval gating on top. zag's other flags are not exposed today — configure zag through its own config files (`~/.zag/...`) instead.

## Rate-limit coordination

`spotifai ask` sessions typically fan out into several sub-agents that each shell out to `spotifai api`. Spotify (and YouTube Music) enforce rolling-window rate limits per application, and the recovery path for repeated hits is a longer cooldown that affects every sibling at once. To prevent that, zad records the deadline from any rate-limit response at `~/.zad/state/<service>/rate_limit.json` and `spotifai api` consults it before issuing each request — Spotify writes that deadline on `HTTP 429`, YouTube Music on `HTTP 429` *or* on `HTTP 403` with a Google quota body (zad 0.9.0 promotes the latter into the same `ZadError::RateLimited` shape). `spotifai ask` sets `SPOTIFAI_WAIT=1` on its own process so every child `spotifai api` call inherits "sleep through the cooldown" behaviour; siblings that would otherwise hammer the provider into a longer cooldown stay paused until the window expires. Pass `--no-wait` to opt out (the session and every sub-agent will then fail fast with a `RateLimited` error on the next API call inside an active window).

## Environment variables

`spotifai ask` reads no environment variables of its own beyond the wait-mode override below. zag and its underlying provider (Claude / Codex / Gemini / Copilot / Ollama) inherit the parent environment, so any variables they consult (`ANTHROPIC_API_KEY`, etc.) are honoured. `spotifai ask` *sets* `SPOTIFAI_PROVIDER`, `SPOTIFAI_PROFILE`, and `SPOTIFAI_WAIT` on its own process so child `spotifai api` shells route to the same `(provider, profile)` policy file the prompt was rendered with and respect the same rate-limit-wait policy.

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_WAIT` | read & set | When unset, defaults to `1` for `spotifai ask` so child shells sleep through any active rate-limit cooldown (Spotify 429, or ymusic 429 / Google-quota 403). `--wait` / `--no-wait` on the command line override the default. Whatever value is resolved is then exported so every sub-agent's `spotifai api` invocation inherits the same policy. |

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
