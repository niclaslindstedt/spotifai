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

1. User runs `spotifai ask "..."`.
2. `main.rs` parses the query and output-format flag, then calls `lib::run`.
3. `lib::run` loads the versioned system prompt from `prompts/` and passes it together with the user's query to `zag`.
4. `zag` reasons over the query and emits one or more tool calls (e.g. `list_playlists`, `create_playlist`).
5. `lib.rs` maps each tool call to the corresponding `zad` function and collects the results.
6. `zag` synthesises a natural-language response from the tool results.
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
