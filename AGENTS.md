# Agent guidance for spotifai

This file is the canonical source of truth for AI coding agents working in this
repo. `CLAUDE.md`, `.cursorrules`, `.windsurfrules`, `GEMINI.md`,
`.aider.conf.md`, and `.github/copilot-instructions.md` are symlinks to this
file.

## OSS Spec conformance

This repository adheres to [`OSS_SPEC.md`](OSS_SPEC.md), a prescriptive
specification for open source project layout, documentation, automation, and
governance. A copy of the spec lives at the repository root so contributors and
AI agents can consult it without leaving the repo; its version is recorded in
the YAML front matter at the top of the file.

Run `oss-spec validate .` to verify conformance. When in doubt about a layout,
naming, or workflow decision, consult the relevant section of `OSS_SPEC.md` —
it is the source of truth for the conventions this repo follows.

## Build and test commands

```sh
make build         # developer build
make test          # full test suite
make lint          # zero-warning linter
make fmt           # format in place
make fmt-check     # verify formatting (CI)
```

## Commit and PR conventions

- All commits follow [Conventional Commits](https://www.conventionalcommits.org/).
- PRs are squash-merged; the **PR title** becomes the single commit on `main`,
  so it must follow conventional-commit format.
- Breaking changes use `<type>!:` or a `BREAKING CHANGE:` footer.

## Architecture summary

`spotifai` is a thin CLI shell around two upstream tools: **zag** (the LLM agent runtime) and **zad** (the Spotify API client). `src/main.rs` parses arguments and dispatches to `src/lib.rs`, which wires the user's natural-language query through zag and routes any Spotify actions that emerge to zad. `src/output.rs` handles terminal formatting and JSON serialisation.

Dependency direction: `main.rs` → `lib.rs` → `zag` + `zad`. Neither zag nor zad imports spotifai. The CLI layer never reaches into Spotify or LLM internals directly — all cross-cutting concerns (auth tokens, retry, rate-limiting) live in zad and zag respectively.

Prompts live under `prompts/` and are versioned independently of the binary. When the agent's system prompt changes, bump the version directory (`<major>_<minor>_<patch>`) rather than editing in place.

### Upstream dependencies: zag and zad

Both upstream tools are consumed as **Rust libraries** via crates.io:

- **zag** ([niclaslindstedt/zag](https://github.com/niclaslindstedt/zag)) —
  the [`zag`](https://crates.io/crates/zag) crate. Re-exports `zag-agent`
  (core), `zag::orch::*` (orchestration), and `zag::serve::*` (HTTP/WS
  server). Use the in-process API directly — do not shell out.

- **zad** ([niclaslindstedt/zad](https://github.com/niclaslindstedt/zad)) —
  the [`zad`](https://crates.io/crates/zad) crate (≥ 0.8.0). Spotifai uses
  `zad::service::spotify::Spotify` and `zad::service::ymusic::Ymusic`
  (typed facades), `zad::service::spotify::SpotifyHttp` /
  `zad::service::ymusic::YmusicHttp` (raw HTTP for verbs the facade does
  not yet expose), `zad::oauth::run_loopback_flow` (in-process OAuth),
  `zad::secrets::{store, load, account, Scope}` (OS keychain),
  `zad::permissions::{signing, trust}` (Ed25519 trust store), and
  `zad::rate_limit` (cross-process 429 coordination — `precall_check`
  is consulted before every zad call so sibling processes do not burn
  quota during an active cooldown window). All spotifai-side helpers
  are bundled in [`src/zad_client.rs`](src/zad_client.rs).
  Bump zad's version in `Cargo.toml` like any other Rust dep.

When bumping zad, run `cargo test` and exercise `spotifai auth`,
`spotifai api`, and `spotifai export|import` against a real account
locally — zad's typed surface is still evolving and request/response
shapes shift between minor releases.

### Rate-limit coordination

Spotify and YouTube Music enforce rolling-window rate limits per
application. zad 0.8.0 records the deadline from any 429 response at
`~/.zad/state/<service>/rate_limit.json` and exposes
`zad::rate_limit::precall_check(service, wait)` so every caller —
inside the current process and any sibling `spotifai api` shell — can
gate its calls behind the shared deadline. `spotifai ask` and
`spotifai playlist` set `SPOTIFAI_WAIT=1` so the sub-agent fan-out
sleeps through cooldowns instead of retrying into a longer ban; the
one-shot commands default to fail-fast. The plumbing lives in
`src/zad_client.rs` (`precall_check`, `wait_mode_with_default`,
`SPOTIFAI_WAIT_ENV`) and is invoked from `api.rs`, `export.rs`, and
`import.rs` before every zad call. The system prompts under
`prompts/ask/` and `prompts/playlist/` instruct sub-agents to respect
429s and never pass `--no-wait`.

### Permissions files (`~/.spotifai/permissions/<provider>/<profile>.toml`)

Two profiles per provider:

- `ask.toml` — read-only verbs for `spotifai ask`.
- `playlist.toml` — adds `playlists create / add / rename` for
  `spotifai playlist`.

`spotifai install` scaffolds them with safe defaults and signs each one
with the per-machine Ed25519 key in the OS keychain (account
`zad/signing:v1`); the resulting signature is upserted into
`~/.zad/signing/trusted.toml`, the per-machine trust store. Subsequent
zad library calls that load these files pass the trust check at load
time.

The `allowed` / `denied` lists are also injected into the agent's system
prompt so the agent self-restricts to the verbs the user has allowed.
After hand-editing, re-run `spotifai install` to resign.

See [`src/permissions.rs`](src/permissions.rs) for the schema,
[`docs/configuration.md`](docs/configuration.md) for the user-facing
reference, and [`docs/export_schema.md`](docs/export_schema.md) for the
provider-agnostic export/import envelope.

## Where new code goes

| Change type | Goes in |
|---|---|
| New feature | `src/...` |
| Tests       | `tests/...` |
| Docs update | `docs/...` |
| Examples    | `examples/...` |
| LLM prompt  | `prompts/<name>/<major>_<minor>_<patch>.md` (see `prompts/README.md`) |

## Test conventions

- **All tests live in separate files** — never inline in source files (no `#[cfg(test)]` blocks, no `if __name__ == "__main__"` test harnesses). This keeps source files free of test scaffolding and lets agents, hooks, and linters treat source and test code differently.
- Test files are named with a `_test` or `_tests` suffix (e.g. `check_test.rs`, `utils_test.py`). The stem must match the pattern `_?[Tt]ests?$` per §20 of `OSS_SPEC.md`.
- Tests live in `tests/`. Use `tempfile` or equivalent for any test that writes to the filesystem.

## Source file size

- Non-test source files must stay under **1000 physical lines** (§20.5 of `OSS_SPEC.md`). When a file grows past the limit, prefer splitting by concern (extracting submodules, helpers, or sibling files) over relaxing the cap.
- A file may opt out by placing `oss-spec:allow-large-file: <reason>` in any comment within its first 20 lines. The reason must be non-empty and motivate why the file genuinely cannot be split (generated code, cohesive state machine, third-party snapshot, inherently dense rule catalogue).

## Documentation sync points

When you change… | Update…
--- | ---
public API | `docs/`, `README.md` Quick start
CLI flags  | `man/<cmd>.md`, `README.md`
config keys| `docs/configuration.md`

## Website staleness policy

Per §11.2 of `OSS_SPEC.md`, the marketing website under `website/` must be
regenerated whenever source-derived content changes — README copy, feature
descriptions, version strings, integration matrices, configuration keys,
and anything else extracted at build time from `README.md`, `docs/`, or
`OSS_SPEC.md`. Do not hand-edit generated content; update the source of
truth and let the build pipeline (and the `update-website` skill, see
below) refresh the site. The `pages.yml` workflow rebuilds and redeploys
on every push to `main`.

## Parity / cross-cutting rules

- **CLI flags ↔ manpage**: every flag added to `src/main.rs` must appear in `man/main.md` with the same name, type, and default. Run `update-manpages` after any flag change.
- **Config keys ↔ docs**: every key in the config struct must appear in `docs/configuration.md`. Run `update-docs` after adding or removing config keys.
- **Prompts ↔ zag tool schema**: if you change the tool definitions that zag exposes to the LLM, bump the prompt version and run `update-prompts` so the prompt template stays aligned with the available tools.

## Maintenance skills

Per §21 of `OSS_SPEC.md`, this repo ships agent skills for keeping drift-prone artifacts in sync with their sources of truth. Skills live under `.agent/skills/<name>/` and are also accessible via the `.claude/skills` symlink.

| Skill | When to run |
|---|---|
| `maintenance`     | When several artifacts have likely drifted at once — umbrella skill that runs every `update-*` skill in the correct order. |
| `sync-oss-spec`   | When the repo may have drifted from `OSS_SPEC.md`. Fetches the latest spec from GitHub, walks its mandates, and fixes each violation. Run as the final step of a drift sweep to catch residual gaps the per-artifact skills did not touch. |
| `update-docs`     | After any change to the public API, configuration keys, or error messages. |
| `update-readme`   | After any change that alters user-visible behavior, commands, or install instructions. |
| `update-manpages` | After any change to CLI flags, subcommands, or their help text. |
| `update-prompts`  | After any change to an LLM prompt's source of truth (embedded docs, rendering-context keys, JSON-schema enums, validation rules). |
| `update-website`  | After any change to README copy, `docs/`, or `OSS_SPEC.md` so the marketing site under `website/` regenerates from the new source data. |
| `commit`          | At the end of a feature or fix to verify quality gates, commit, push, and open or update a PR with a conventional-commit-formatted title. |

Each skill has a `SKILL.md` (the playbook) and a `.last-updated` file (the baseline commit hash). Run a skill by loading its `SKILL.md` and following the discovery process and update checklist. The skill rewrites `.last-updated` at the end of a successful run, and improves itself in place when it discovers new mapping entries. The `maintenance` skill owns a **Registry** table listing every `update-*` skill — add a row whenever you create a new sync skill.