# Architecture of spotifai

## Module layout

```
src/
  main.rs    — argument parsing, top-level dispatch
  lib.rs     — core pipeline: query → agent → Spotify actions
  output.rs  — terminal and JSON rendering
```

`main.rs` is intentionally thin: it calls `clap` (or equivalent) to parse flags and delegates immediately to `lib.rs`. All business logic lives in the library crate so it can be integration-tested without spawning a subprocess.

## Dependency direction

```
main.rs
  └── lib.rs
        ├── zag   (LLM agent runtime)
        └── zad   (Spotify API client)
```

Neither `zag` nor `zad` imports `spotifai`. The CLI layer never calls Spotify or LLM APIs directly — authentication, token refresh, retry, and rate-limiting are owned by `zad` and `zag` respectively.

## Request flow

1. User runs `spotifai ask "..."` or `spotifai playlist "..."`.
2. `main.rs` parses the query and dispatches via `cli::run` to the matching command module (`ask::run` / `playlist::run`).
3. The command module picks a `Profile` (`Ask` or `Playlist`), loads the matching `~/.spotifai/permissions/<profile>.toml`, renders the versioned system prompt from `prompts/<name>/<version>.md` with the policy injected, and sets `SPOTIFAI_PROFILE` so child `spotifai api` shells can route to the same file.
4. zag reasons over the query and emits one or more shell tool calls of the form `spotifai api <verb>`.
5. `spotifai api` reads `SPOTIFAI_PROFILE`, resolves it to the profile's permissions file, sets `ZAD_PERMISSIONS_PATH`, and forwards to `~/.spotifai/bin/zad spotify <verb>`. zad's load-time trust check verifies the file is signed and that the verb is in the policy.
6. zad executes the Spotify Web API call and returns its result on stdout; zag synthesises a natural-language response from the tool output.
7. `output.rs` renders the response in the requested format (`text` or `json`) and writes it to stdout.

## Prompts

System prompts live under `prompts/<name>/<major>_<minor>_<patch>.md` and are versioned independently of the binary. The version directory acts as a changelog: keep old versions in place and write a new one when the prompt's behaviour changes meaningfully.

## Cross-cutting concerns

| Concern | Owner |
|---------|-------|
| Spotify OAuth and token refresh | `zad` |
| LLM API key and retry | `zag` |
| Output formatting | `src/output.rs` |
| Config loading | `src/lib.rs` (reads `~/.config/spotifai/config.toml` and env vars) |
| Error messages | `src/lib.rs` surfaces `zag`/`zad` errors with user-facing context |
