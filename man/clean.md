# spotifai clean

> Start an interactive zag session pre-loaded to clean up the user's library on the active provider — deleting playlists, removing tracks from playlists, and unsaving items — with the local permissions file injected so the agent self-restricts and a strict read-then-confirm-then-delete workflow baked into the system prompt.

## Synopsis

```
spotifai clean [--provider <slug>] [QUERY]...
```

## Description

`spotifai clean` opens an interactive [zag](https://github.com/niclaslindstedt/zag) session for **destructive** library cleanup. The session's system prompt:

1. Tells the agent that every interaction must go through the `spotifai api …` shell command (not the provider's API directly), because that is where zad's scope and permissions are enforced.
2. Inlines the contents of `~/.spotifai/permissions/<provider>/clean.toml` so the agent knows which `spotifai api` verbs it is allowed to invoke. The default profile strips out the public-catalogue `search` and the creator verbs (`playlists create|add|rename`, `library save|like`) and adds the destructive verbs.
3. Substitutes provider-specific example calls (Spotify uses `library tracks/albums …`; YouTube Music uses `library …` over rated videos). The example block enumerates every verb the provider supports, not every verb this surface is allowed to use — the permissions block is authoritative.
4. Requires the agent to follow a **read → list → confirm → delete** workflow: enumerate the candidate set, render it back to the user with a count (and a representative sample if the list is long), and wait for an explicit "yes/no" affirmative reply before issuing any `playlists delete`, `playlists remove`, `library tracks unsave`, `library albums unsave`, or `library unlike` call.

Before starting zag, `spotifai clean` ensures `~/.spotifai/permissions/<provider>/clean.toml` exists (scaffolding it with the default destructive policy if not). The zad library is consumed in-process — there is no separate binary to install — so no version check runs here.

The optional positional argument becomes the agent's first turn. With no argument, the session opens empty and waits for the user to type. Quit with `Ctrl+D` or whatever exit gesture the active zag provider uses.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `--provider <slug>` | enum | `spotify` | Backing provider whose library to clean up. One of `spotify`, `ymusic`. |
| `[QUERY]...` | trailing | — | Optional opening instruction (e.g. `"remove all baby songs"`). Joined with spaces and used as the agent's first turn. |

## Flags

`spotifai clean` owns `--provider`. The global `--wait` / `--no-wait` flags (see [`main.md`](main.md)) also apply — `spotifai clean` defaults to `--wait` so sub-agents coordinate on shared rate-limit cooldowns. The global `--yolo` flag is also honoured: it forwards `auto_approve(true)` to the underlying zag `AgentBuilder` so the session skips every per-tool approval prompt. The `(provider, profile)` policy file at `~/.spotifai/permissions/<provider>/clean.toml` is still enforced by `spotifai api` at the zad layer, so `--yolo` cannot widen the allowed verb list — it only suppresses zag's tool-approval gating on top. zag's other flags are not exposed today — configure zag through its own config files (`~/.zag/...`) instead.

`--yolo` does not remove the in-prompt confirmation gate. The agent is instructed to ask "Proceed with deleting these N items? (yes/no)" before every destructive call regardless. To deviate from that workflow you would have to hand-edit the prompt template.

## Rate-limit coordination

`spotifai clean` sessions typically run a few read calls to enumerate candidates and then one or more batched destructive calls. Spotify (and YouTube Music) enforce rolling-window rate limits per application, and the recovery path for repeated 429s is a longer cooldown that affects every sibling at once. To prevent that, zad 0.8.0 records the deadline from any 429 response at `~/.zad/state/<service>/rate_limit.json` and `spotifai api` consults it before issuing each request. `spotifai clean` sets `SPOTIFAI_WAIT=1` on its own process so every child `spotifai api` call inherits "sleep through the cooldown" behaviour; siblings that would otherwise hammer the provider into a longer cooldown stay paused until the window expires. Pass `--no-wait` to opt out (the session and every sub-agent will then fail fast with a `RateLimited` error on the next API call inside an active window).

## Environment variables

`spotifai clean` reads no environment variables of its own beyond the wait-mode override below. zag and its underlying provider (Claude / Codex / Gemini / Copilot / Ollama) inherit the parent environment, so any variables they consult (`ANTHROPIC_API_KEY`, etc.) are honoured. `spotifai clean` *sets* `SPOTIFAI_PROVIDER`, `SPOTIFAI_PROFILE`, and `SPOTIFAI_WAIT` on its own process so child `spotifai api` shells route to the same `(provider, profile)` policy file the prompt was rendered with and respect the same rate-limit-wait policy.

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_WAIT` | read & set | When unset, defaults to `1` for `spotifai clean` so child shells sleep through any active 429 cooldown. `--wait` / `--no-wait` on the command line override the default. Whatever value is resolved is then exported so every sub-agent's `spotifai api` invocation inherits the same policy. |

## Permissions

The injected policy lives at `~/.spotifai/permissions/<provider>/clean.toml`. On first install it allows only the verbs needed to inspect the user's library plus the destructive ones:

- **Spotify allowed**: `playlists list`, `playlists show`, `library tracks list`, `library albums list`, `playlists delete`, `playlists remove`, `library tracks unsave`, `library albums unsave`.
- **Spotify denied**: `search`, `playlists create`, `playlists add`, `playlists rename`, `library tracks save`, `library albums save`.
- **YouTube Music allowed**: `playlists list`, `playlists show`, `library list`, `playlists delete`, `playlists remove`, `library unlike`.
- **YouTube Music denied**: `search`, `playlists create`, `playlists add`, `playlists rename`, `library like`.

`search` is denied on purpose — `clean` works on items the user already has, not on the public catalogue. The `clean` profile is independent of the `ask` and `playlist` profiles — edits to one do not affect the others. To widen `clean` (e.g. to allow `playlists rename` so you can rename old playlists during a cleanup pass), hand-edit `allowed` / `denied` and re-run `spotifai install` to resign the file; spotifai re-reads it on every `spotifai clean` invocation. The agent is forbidden in the system prompt from editing the file itself, so widening always requires a deliberate human edit. The permissions file is **advisory** — it constrains the agent via prompt injection — but zad's library-side trust check at load time is the authoritative gate: the file is rejected if its signature is not in `~/.zad/signing/trusted.toml`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | zag session ended cleanly. |
| 1 | Generic spotifai error (missing home directory, permissions parse error, tokio runtime build failure, prompt rendering failure). |
| 2 | Usage error parsing `spotifai clean` itself. |
| *N* | Any other code is propagated from zag's terminal exit. |

## Examples

Remove every "baby" song on Spotify after confirming the list:

```sh
spotifai clean "remove all baby songs — my child is 15 now"
```

Delete a named playlist on YouTube Music:

```sh
spotifai clean --provider ymusic "delete my 'old phone' playlist"
```

Unsave old saved albums:

```sh
spotifai clean "unsave every saved album from before 2010"
```

Open the session with no opener and start typing:

```sh
spotifai clean
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`api.md`](api.md) — the typed-dispatch shim the agent uses
- [`ask.md`](ask.md) — read-only counterpart
- [`playlist.md`](playlist.md) — write-side counterpart for building new playlists
- [`install.md`](install.md) — bootstraps the trust store and scaffolds the permissions files
