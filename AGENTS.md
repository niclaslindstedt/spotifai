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

`spotifai` is a thin CLI shell around two workspace siblings: **zag** (the LLM agent runtime) and **zad** (the Spotify API client). `src/main.rs` parses arguments and dispatches to `src/lib.rs`, which wires the user's natural-language query through zag and routes any Spotify actions that emerge to zad. `src/output.rs` handles terminal formatting and JSON serialisation.

Dependency direction: `main.rs` → `lib.rs` → `zag` + `zad`. Neither zag nor zad imports spotifai. The CLI layer never reaches into Spotify or LLM internals directly — all cross-cutting concerns (auth tokens, retry, rate-limiting) live in zad and zag respectively.

Prompts live under `prompts/` and are versioned independently of the binary. When the agent's system prompt changes, bump the version directory (`<major>_<minor>_<patch>`) rather than editing in place.

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

## Parity / cross-cutting rules

- **CLI flags ↔ manpage**: every flag added to `src/main.rs` must appear in `man/main.md` with the same name, type, and default. Run `update-manpages` after any flag change.
- **Config keys ↔ docs**: every key in the config struct must appear in `docs/configuration.md`. Run `update-docs` after adding or removing config keys.
- **Prompts ↔ zag tool schema**: if you change the tool definitions that zag exposes to the LLM, bump the prompt version and run `update-prompts` so the prompt template stays aligned with the available tools.

## Maintenance skills

Per §21 of `OSS_SPEC.md`, this repo ships agent skills for keeping drift-prone artifacts in sync with their sources of truth. Skills live under `.agent/skills/<name>/` and are also accessible via the `.claude/skills` symlink.

| Skill | When to run |
|---|---|
| `maintenance`    | When several artifacts have likely drifted at once — umbrella skill that runs every `update-*` skill in the correct order. |
| `update-docs`    | After any change to the public API, configuration keys, or error messages. |
| `update-readme`  | After any change that alters user-visible behavior, commands, or install instructions. |
| `update-manpages` | After any change to CLI flags, subcommands, or their help text. |
| `update-prompts` | After any change to an LLM prompt's source of truth (embedded docs, rendering-context keys, JSON-schema enums, validation rules). |

Each skill has a `SKILL.md` (the playbook) and a `.last-updated` file (the baseline commit hash). Run a skill by loading its `SKILL.md` and following the discovery process and update checklist. The skill rewrites `.last-updated` at the end of a successful run, and improves itself in place when it discovers new mapping entries. The `maintenance` skill owns a **Registry** table listing every `update-*` skill — add a row whenever you create a new sync skill.