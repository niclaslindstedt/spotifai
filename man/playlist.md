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

Before starting zag, `spotifai playlist` ensures `~/.spotifai/permissions/<provider>/playlist.toml` exists (scaffolding it with the default playlist-curator policy if not). The zad library is consumed in-process — there is no separate binary to install — so no version check runs here.

The optional positional argument becomes the agent's first turn — usually a brief like `"a 30-minute focus playlist with no vocals"`. With no argument, the session opens empty and waits for the user to type. Quit with `Ctrl+D` or whatever exit gesture the active zag provider uses.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider for the new playlist. One of `spotify`, `ymusic`. |
| `[QUERY]...` | trailing | — | Optional brief. Joined with spaces and used as the agent's first turn. |

## Flags

`spotifai playlist` owns `--provider`. The global `--wait` / `--no-wait` flags (see [`main.md`](main.md)) also apply — `spotifai playlist` defaults to `--wait` so the curator and its fan-out of search-subagents coordinate on shared rate-limit cooldowns. The global `--yolo` flag is also honoured: it forwards `auto_approve(true)` to the underlying zag `AgentBuilder` so the session skips every per-tool approval prompt. The `(provider, profile)` policy file at `~/.spotifai/permissions/<provider>/playlist.toml` is still enforced by `spotifai api` at the zad layer, so `--yolo` cannot widen the allowed verb list (destructive verbs stay denied) — it only suppresses zag's tool-approval gating on top. zag's other flags are not exposed today — configure zag through its own config files (`~/.zag/...`) instead.

## Rate-limit coordination

The curator workflow fans out search work across many sub-agents that all shell into `spotifai api search …`. Spotify (and YouTube Music) enforce rolling-window rate limits per application, and repeated hits escalate to a longer cooldown that affects every sibling at once. `spotifai playlist` sets `SPOTIFAI_WAIT=1` on its own process so every child `spotifai api` invocation consults zad's shared deadline file at `~/.zad/state/<service>/rate_limit.json` and sleeps through any active cooldown window. Spotify writes that deadline on `HTTP 429`; YouTube Music writes it on `HTTP 429` *or* on `HTTP 403` with a Google quota body — zad 0.9.0 promotes those 403s into the same `ZadError::RateLimited` shape, so the curator's fan-out gate is one branch regardless of provider. The system prompt tells sub-agents not to retry-storm on rate-limit errors and not to pass `--no-wait`, so a single misbehaving subagent cannot starve the rest of the fan-out. Pass `--no-wait` on the parent invocation to opt out — every sub-agent will then fail fast instead.

## Environment variables

`spotifai playlist` reads no environment variables of its own beyond the wait-mode override below. `spotifai playlist` *sets* `SPOTIFAI_PROVIDER`, `SPOTIFAI_PROFILE`, and `SPOTIFAI_WAIT` on its own process so child `spotifai api` shells route to the same `(provider, playlist)` policy file the prompt was rendered with and respect the same rate-limit-wait policy.

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_WAIT` | read & set | When unset, defaults to `1` for `spotifai playlist` so child shells (the curator's search-subagent fan-out) sleep through any active rate-limit cooldown (Spotify 429, or ymusic 429 / Google-quota 403) instead of fanning more requests at the wall. `--wait` / `--no-wait` on the command line override the default. Whatever value is resolved is then exported so every sub-agent's `spotifai api` invocation inherits the same policy. |

## Permissions

The injected policy lives at `~/.spotifai/permissions/<provider>/playlist.toml`. On first install:

- **Spotify allowed**: `search`, `playlists list`, `playlists show`, `playlists create`, `playlists add`, `playlists rename`, `library tracks list`, `library albums list`.
- **Spotify denied**: `playlists delete`, `playlists remove`, `library tracks save|unsave`, `library albums save|unsave`.
- **YouTube Music allowed**: `search`, `playlists list`, `playlists show`, `playlists create`, `playlists add`, `playlists rename`, `library list`.
- **YouTube Music denied**: `playlists delete`, `playlists remove`, `library like`, `library unlike`.

The `playlist` profile is independent of the `ask` profile. To narrow or widen `playlist`, hand-edit `allowed` / `denied` and re-run `spotifai install` to resign the file; spotifai re-reads it on every `spotifai playlist` invocation. The agent is forbidden in the system prompt from editing the file itself, so widening always requires a deliberate human edit. The permissions file is **advisory** — it constrains the agent via prompt injection — but zad's library-side trust check at load time is the authoritative gate: the file is rejected if its signature is not in `~/.zad/signing/trusted.toml`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | zag session ended cleanly. |
| 1 | Generic spotifai error (missing home directory, permissions parse error, tokio runtime build failure, prompt rendering failure). |
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
- [`api.md`](api.md) — the typed-dispatch shim the agent uses
- [`install.md`](install.md) — bootstraps the trust store and scaffolds the permissions files
