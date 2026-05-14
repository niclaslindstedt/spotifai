# Architecture of spotifai

## Module layout

```
src/
  main.rs           — argument parsing, top-level dispatch
  lib.rs            — module registry
  cli.rs            — clap-derived CLI surface (`--provider`, `--wait`/`--no-wait`, `--yolo`, subcommands)
  providers.rs      — provider abstraction: Spotify, YouTube Music, …
  permissions.rs    — per-(provider, profile) permission file format and helpers
  api.rs            — `spotifai api` typed dispatcher into the zad library
  api_fields.rs     — `--fields` / `--format` projection for `api` results
  auth.rs           — `spotifai auth` forwarder to `zad service create <provider>`
  install.rs        — `spotifai install` (zad binary, signing key, signed permissions)
  session.rs        — shared agent runner used by `ask`, `playlist`, and `clean`
  ask.rs            — `spotifai ask` (read-only zag session)
  playlist.rs       — `spotifai playlist` (one-shot playlist-builder zag session)
  clean.rs          — `spotifai clean` (destructive library-cleanup zag session)
  export.rs         — `spotifai export` (deterministic JSON dump of the user's library)
  export_schema.rs  — provider-agnostic export envelope
  import.rs         — `spotifai import` (deterministic playlist recreation from an export envelope)
  output.rs         — terminal and JSON rendering (the central §19.4 output module)
  logging.rs        — always-on debug.log writer + `--debug` stderr echo
  commands_index.rs — built-in command catalogue surfaced via `--help` and `help-agent`
  help_agent.rs     — machine-readable help dump for nested agents
  manpages.rs       — embedded `man/<cmd>.md` rendering for `spotifai help <cmd>`
  topic_docs.rs     — embedded `docs/<topic>.md` rendering for `spotifai help <topic>`
  zad_client.rs     — thin wrappers around zad: `precall_check`, `wait_mode*`, `SPOTIFAI_WAIT_ENV`
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

### Rate-limit coordination (zad 0.9.0)

Spotify and YouTube Music both enforce rolling-window rate limits per application, and the practical consequence of hammering them through a rate-limit response is a longer cooldown that affects every concurrent caller — including unrelated sibling processes that share the OAuth client. zad introduced a per-service deadline file at `~/.zad/state/<service>/rate_limit.json` in 0.8.0: on any 429 response the service client parses `Retry-After`, writes the absolute deadline, and returns `ZadError::RateLimited`. zad 0.9.0 extends the same gate to YouTube Music's de-facto 429: an `HTTP 403` with a Google quota body (`quotaExceeded`, `dailyLimitExceeded`, `rateLimitExceeded`, `userRateLimitExceeded`) is classified by `zad::google_quota`, persisted to the same deadline file, and surfaced as the same `ZadError::RateLimited` shape — so callers handle one error type regardless of provider. Daily-quota deadlines (reset at midnight Pacific Time, up to ~25 hours) are persisted faithfully; short-term per-user limits honor `Retry-After` when present or fall back to a 60-second window. Spotifai calls `zad::rate_limit::precall_check(service, wait)` before every zad operation (`src/api.rs`, `src/export.rs`, `src/import.rs`); with `wait = true` the helper sleeps until the deadline (capped at one hour per invocation — ymusic daily quotas that exceed the cap will sleep an hour and then surface `RateLimited` so the user can decide whether to keep waiting), with `wait = false` it returns the same error so the caller fails fast without burning a request.

The interactive surfaces (`spotifai ask`, `spotifai playlist`, `spotifai clean`) set `SPOTIFAI_WAIT=1` on their own process, so every sub-agent that fans out into `spotifai api …` inherits the same coordination automatically. The one-shot surfaces (`spotifai api`, `export`, `import`) default to fail-fast so a user-driven call surfaces rate-limit errors instead of stalling silently. The CLI exposes `--wait` / `--no-wait` global flags as the override.

## Provider abstraction

`src/providers.rs` owns the `Provider` enum and per-provider data:

- The CLI slug (`spotify`, `ymusic`) used by `--provider`, `SPOTIFAI_PROVIDER`, and the `<provider>/` directory under `~/.spotifai/permissions/`.
- The matching zad subcommand (`zad spotify …` / `zad ymusic …`).
- The zad service slug consumed by `zad service create <slug>`.
- A human-readable display name for prompts and CLI banners.
- A default permissions policy per profile (`Profile::Ask`, `Profile::Playlist`, `Profile::Clean`) — verbs differ between providers (Spotify exposes `library albums list`; YouTube Music exposes `library list` over rated videos).
- A prompt example block — provider-specific `spotifai api` invocations the LLM is expected to use.

Adding another provider (Tidal, Apple Music, …) is a single change in `providers.rs` plus a new `clap::ValueEnum` variant on `cli.rs::ProviderArg`. The rest of the codebase routes through `provider.*` accessors, so the new variant is picked up everywhere automatically.

## Request flow

1. User runs `spotifai ask --provider <slug> "..."`, `spotifai playlist --provider <slug> "..."`, or `spotifai clean --provider <slug> "..."`.
2. `main.rs` parses the query and dispatches via `cli::run` to the matching command module.
3. The command module picks a `(Provider, Profile)` pair, loads the matching `~/.spotifai/permissions/<provider>/<profile>.toml`, renders the versioned system prompt from `prompts/<name>/<version>.md` with the policy and the provider's example block injected, and sets `SPOTIFAI_PROVIDER` + `SPOTIFAI_PROFILE` (plus `SPOTIFAI_WAIT=1` for the interactive surfaces) so child `spotifai api` shells route to the same file and share the rate-limit cooldown (Spotify 429, or ymusic 429 / Google-quota 403).
4. zag reasons over the query and emits one or more shell tool calls of the form `spotifai api <verb>`.
5. The spawned `spotifai api` process reads `SPOTIFAI_PROVIDER` / `SPOTIFAI_PROFILE`, loads the policy file via zad's signed-trust check, runs `zad::rate_limit::precall_check` to honour any active cooldown, and dispatches the verb directly through the zad library's typed facades (`zad::service::spotify::Spotify`, `zad::service::ymusic::Ymusic`) — there is no shell-out to a `zad` binary.
6. zad executes the API call against the active provider (Spotify Web API, YouTube Data API v3, …) and returns the typed response; `spotifai api` serialises it to JSON and writes it to stdout. zag synthesises a natural-language response from the tool output.
7. `output.rs` renders the response in the requested format (`text` or `json`) and writes it to stdout.

## Prompts

System prompts live under `prompts/<name>/<major>_<minor>_<patch>.md` and are versioned independently of the binary. The version directory acts as a changelog: keep old versions in place and write a new one when the prompt's behaviour changes meaningfully. The current `ask`, `playlist`, and `clean` prompts are templated with three placeholders — `{{ provider_name }}`, `{{ provider_examples }}`, `{{ permissions_block }}` — substituted at runtime from the `(provider, profile)` pair selected by the caller.

## Cross-cutting concerns

| Concern | Owner |
|---------|-------|
| Provider credentials and token refresh | `zad` |
| LLM API key and retry | `zag` |
| Provider abstraction (Spotify vs. YouTube Music vs. future) | `src/providers.rs` |
| Output formatting | `src/output.rs` |
| Config loading | `src/cli.rs` (clap) + `src/permissions.rs` (per-(provider, profile) files) |
| Error messages | command modules surface `zag`/`zad` errors with user-facing context |
