# Architecture of spotifai

## Module layout

```
src/
  main.rs        — argument parsing, top-level dispatch
  lib.rs         — module registry
  cli.rs         — clap-derived CLI surface (`--provider`, subcommands)
  providers.rs   — provider abstraction: Spotify, YouTube Music, …
  permissions.rs — per-(provider, profile) permission file format and helpers
  api.rs         — `spotifai api` forwarder to `zad <provider> …`
  auth.rs        — `spotifai auth` forwarder to `zad service create <provider>`
  install.rs     — `spotifai install` (zad binary, signing key, signed permissions)
  session.rs     — shared agent runner used by `ask` and `playlist`
  ask.rs         — `spotifai ask` (read-only zag session)
  playlist.rs    — `spotifai playlist` (one-shot playlist-builder zag session)
  export.rs      — `spotifai export` (deterministic JSON dump of the user's library)
  import.rs      — `spotifai import` (deterministic playlist recreation from an export envelope)
  output.rs      — terminal and JSON rendering
```

`main.rs` is intentionally thin: it calls `clap` to parse flags and delegates immediately to `cli::run`. All business logic lives in the library crate so it can be integration-tested without spawning a subprocess.

## Dependency direction

```
main.rs
  └── lib.rs
        ├── zag   (LLM agent runtime)
        └── zad   (Spotify / YouTube Music / … API client)
```

Neither `zag` nor `zad` imports `spotifai`. The CLI layer never calls a music-service API or the LLM directly — authentication, token refresh, retry, and rate-limiting are owned by `zad` and `zag` respectively.

## Provider abstraction

`src/providers.rs` owns the `Provider` enum and per-provider data:

- The CLI slug (`spotify`, `ymusic`) used by `--provider`, `SPOTIFAI_PROVIDER`, and the `<provider>/` directory under `~/.spotifai/permissions/`.
- The matching zad subcommand (`zad spotify …` / `zad ymusic …`).
- The zad service slug consumed by `zad service create <slug>`.
- A human-readable display name for prompts and CLI banners.
- A default permissions policy per profile (`Profile::Ask`, `Profile::Playlist`) — verbs differ between providers (Spotify exposes `library albums list`; YouTube Music exposes `library list` over rated videos).
- A prompt example block — provider-specific `spotifai api` invocations the LLM is expected to use.

Adding another provider (Tidal, Apple Music, …) is a single change in `providers.rs` plus a new `clap::ValueEnum` variant on `cli.rs::ProviderArg`. The rest of the codebase routes through `provider.*` accessors, so the new variant is picked up everywhere automatically.

## Request flow

1. User runs `spotifai ask --provider <slug> "..."` or `spotifai playlist --provider <slug> "..."`.
2. `main.rs` parses the query and dispatches via `cli::run` to the matching command module.
3. The command module picks a `(Provider, Profile)` pair, loads the matching `~/.spotifai/permissions/<provider>/<profile>.toml`, renders the versioned system prompt from `prompts/<name>/<version>.md` with the policy and the provider's example block injected, and sets `SPOTIFAI_PROVIDER` + `SPOTIFAI_PROFILE` so child `spotifai api` shells can route to the same file.
4. zag reasons over the query and emits one or more shell tool calls of the form `spotifai api <verb>`.
5. `spotifai api` reads `SPOTIFAI_PROVIDER` and `SPOTIFAI_PROFILE`, resolves them to the policy file, sets `ZAD_PERMISSIONS_PATH`, and forwards to `~/.spotifai/bin/zad <provider> <verb>`. zad's load-time trust check verifies the file is signed and that the verb is in the policy.
6. zad executes the API call against the active provider (Spotify Web API, YouTube Data API v3, …) and returns its result on stdout; zag synthesises a natural-language response from the tool output.
7. `output.rs` renders the response in the requested format (`text` or `json`) and writes it to stdout.

## Prompts

System prompts live under `prompts/<name>/<major>_<minor>_<patch>.md` and are versioned independently of the binary. The version directory acts as a changelog: keep old versions in place and write a new one when the prompt's behaviour changes meaningfully. The current `ask` and `playlist` prompts are templated with three placeholders — `{{ provider_name }}`, `{{ provider_examples }}`, `{{ permissions_block }}` — substituted at runtime from the `(provider, profile)` pair selected by the caller.

## Cross-cutting concerns

| Concern | Owner |
|---------|-------|
| Provider credentials and token refresh | `zad` |
| LLM API key and retry | `zag` |
| Provider abstraction (Spotify vs. YouTube Music vs. future) | `src/providers.rs` |
| Output formatting | `src/output.rs` |
| Config loading | `src/cli.rs` (clap) + `src/permissions.rs` (per-(provider, profile) files) |
| Error messages | command modules surface `zag`/`zad` errors with user-facing context |
