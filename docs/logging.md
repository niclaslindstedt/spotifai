# Logging and diagnostic output

This document is the source of truth for spotifai's logging system. It
specifies the levels, the semantic helpers, the glyphs, the colors, the
indentation model, the streams, the input helpers, and the structured
fields. It is also designed to be lifted wholesale into other CLI
projects — the rules are deliberately framework-agnostic and the call
sites in spotifai are a worked example of the discipline.

`OSS_SPEC.md` §19 sets the floor (four levels, central output module,
always-on file log, `--debug` flag). This document refines the floor
into something an engineer can apply consistently from line one without
arguing in code review.

## Mission

Every diagnostic message a CLI emits is a contract with whoever reads
the terminal. A message that does not change a reader's behavior — they
neither act on it, learn from it, nor file it for later — is noise that
hides the messages that do. The system below exists so that:

1. **Every level has a job, and only one job.** A warning is a future
   error a user can prevent. An info is a checkpoint the user wants to
   see. A debug is a breadcrumb for post-hoc triage. Picking the level
   should be mechanical, not aesthetic.
2. **Form follows function.** Glyphs, colors, and indentation are not
   decoration — each one encodes one bit of information that the prose
   would otherwise have to spell out. A green check means "this thing
   you asked for is done." A dim bullet means "more detail about the
   line above it." A yellow `⚠` means "look at this when the run
   finishes."
3. **Streams stay clean.** stdout is a contract with downstream tools
   (jq, the next pipe stage, the test harness). Anything human-facing
   goes to stderr. ANSI escapes never go to stdout.
4. **Input is as deliberate as output.** The same module that prints
   diagnostics also reads them — `prompt` and `confirm` are styled,
   stream-aware, and idempotent under non-interactive runs.

## Levels and helpers

spotifai exposes ten semantic helpers. They map onto the four spec
levels but are chosen by what the message *does* for the reader, not by
its severity:

| Helper           | Level   | Glyph (UTF) | ASCII | Color           | Use it when…                                                                    |
|------------------|---------|-------------|-------|-----------------|----------------------------------------------------------------------------------|
| `header(msg)`    | `info`  | `══`        | `==`  | bold cyan       | Naming the operation the command is about to perform — printed once at the top. |
| `step(n,t,msg)`  | `info`  | `[n/t]`     | `[n/t]` | bold cyan     | A multi-stage procedure (install, migration). Always paired with `section`.     |
| `section(msg)`   | `info`  | `▌` + msg   | `>>`  | bold cyan       | Same as `header`, but returns a `ScopeGuard` so subsequent lines indent under it.|
| `action(msg)`    | `info`  | `→`         | `->`  | cyan            | About to do work the user is waiting on. Phrase as a present-participle verb.   |
| `status(msg)`    | `info`  | `✓`         | `[ok]`| green           | A discrete user-visible win — the thing the user asked for actually happened.   |
| `info(msg)`      | `info`  | `·`         | `*`   | (none)          | Neutral context the user should see (paths, totals, mode). Use sparingly.       |
| `detail(msg)`    | `info`  | `  ·`       | `  -` | dim             | A sub-bullet under the previous `action` or `status`. Always one indent deeper. |
| `hint(msg)`      | `info`  | `i`         | `i`   | dim cyan        | Actionable suggestion the user can ignore. Never used for required follow-ups.  |
| `warn(msg)`      | `warn`  | `⚠`         | `[!]` | bold yellow     | Recoverable; either partial success now, or a future error the user can prevent. |
| `error(msg)`     | `error` | `✗`         | `[x]` | bold red        | Unrecoverable for this command. Almost always followed by a non-zero exit.      |
| `debug(msg)`     | `debug` | `…`         | `[debug]` | dim         | Verbose triage data only useful when investigating a bug.                       |
| `plain(msg)`     | (info)  | (none)      |       | (none)          | Machine-readable contract output on **stdout** (JSON, version banner).          |

### How to pick a level: the decision tree

Apply these in order. The first one that matches wins.

1. **Is this output a contract with a downstream tool?** (JSON the next
   pipe stage parses, the version banner a script greps for, the
   `--help-agent` body a sibling LLM splices into a prompt.) → `plain`.
2. **Is the operation the user asked for impossible to complete?**
   → `error`. The next thing this command does should be exit non-zero.
3. **Did the operation succeed but in a way the user wouldn't expect, or
   will a future operation fail unless the user acts?** → `warn`.
   Examples: "playlist X already exists — skipping", "Google did not
   return the YouTube scope — `import` will fail until you re-auth".
4. **Did one discrete unit of user-visible work just complete
   successfully?** → `status`. Examples: "credentials written to OS
   keychain", "exported 312 liked items".
5. **Is the program about to start a long-running unit of work the user
   is waiting on?** → `action`. Always present-participle ("fetching
   playlists", "signing permission profiles"). The matching `status`
   line at the end uses past-tense ("fetched 8 playlists").
6. **Is this a sub-bullet under the line immediately above?** →
   `detail`. Indent one extra level. Detail lines should never appear
   without an `action` or `status` directly above them.
7. **Is this a one-line context the user must see (a path, a mode flag,
   a total)?** → `info`. Resist the urge — most "info" lines are
   actually `detail` under an action.
8. **Is this an optional next step or a useful pointer the user can
   safely ignore?** → `hint`.
9. **Is this purely diagnostic — only useful when something is broken
   and the user is reading `debug.log`?** → `debug`.
10. **Is this the top of a command, or a numbered sub-stage of one?**
    → `header` for the command name; `section` if the body that follows
    should be visually nested; `step(n, total, label)` for the n-th of
    a multi-stage procedure.

If none of the above fits, the line probably should not be emitted.
Silence is consequential too.

### Anti-patterns

- **Don't use `info` as a default.** Most calls that look like `info`
  are really `action`, `detail`, or `debug`. If the message doesn't
  give the user a checkpoint they care about, it doesn't belong on the
  terminal.
- **Don't pair `action` with `status` for trivial work.** Single-call
  operations get one `status` line at the end. The `action`/`status`
  pair is for work where the gap between them is long enough that a
  user might wonder if the program hung.
- **Don't emit `warn` for things that aren't actionable.** "Something
  unusual happened" is not a warning. A warning answers "what should
  the user do about this when the run finishes?"
- **Don't emit `error` and then continue.** If the program can recover
  and the user's task can still complete, the right level is `warn`.
- **Don't use `debug` for sensitive data.** The file log is captured by
  bug reports and `--debug-agent` recipes. Treat it as quotable.

## Glyphs

Glyphs are single-column markers padded to two cells of visual width
(glyph + space). They mean exactly one thing each, and the meaning is
the same whether you see them at the top of the screen or in the middle
of a wall of text:

| Glyph | Means                                            |
|-------|--------------------------------------------------|
| `══`  | This is a command boundary — a new operation is starting. |
| `▌`   | Same as `══`, plus everything below is part of this section. |
| `→`   | Work is in progress; the program has not hung.   |
| `✓`   | A unit of work completed successfully.           |
| `·`   | Neutral fact; sub-bullet when indented further.  |
| `i`   | Pointer the reader can act on if they want.      |
| `⚠`   | Look at this when the command finishes.          |
| `✗`   | The thing you asked for did not happen.          |
| `…`   | Diagnostic breadcrumb — file log only by default.|

Glyphs are dropped on `SPOTIFAI_GLYPHS=ascii` and replaced with the
ASCII fallback in the table above. The fallback is deliberately ugly:
the goal is grep-friendliness for log post-processing, not aesthetics.

## Colors

Colors carry the same one-bit meaning glyphs do, doubled up so a
color-blind or scrolled reader picks up the signal from either channel:

- **bold cyan** — structural (header, section, step). Tells the eye
  where one operation ends and the next begins.
- **cyan** — in-progress (action, hint).
- **green** — success (status).
- **yellow** — recoverable issue (warn).
- **red** — failure (error).
- **dim** — secondary information (detail, debug, hint body).

### Detection

ANSI escapes are emitted on stderr only when *every* gate below passes:

1. The active call destination is stderr — `plain` (stdout) is **never**
   styled, even on a TTY.
2. `NO_COLOR` is unset or empty (https://no-color.org).
3. `SPOTIFAI_COLOR` is unset, `auto`, `1`, or `always`. `0`, `never`,
   or any other value disables color.
4. When `SPOTIFAI_COLOR` is `auto` (or unset), `stderr` is a terminal
   per `std::io::IsTerminal`.

Precedence: `SPOTIFAI_COLOR=always` overrides `NO_COLOR`; `NO_COLOR`
overrides the TTY check; `SPOTIFAI_COLOR=never` overrides everything.

The same gate governs unicode glyphs unless `SPOTIFAI_GLYPHS` is set
explicitly.

## Indentation and scopes

A `ScopeGuard` raises the indent level for every helper that runs
during its lifetime, then restores it on drop. The intended pattern:

```rust
output::header("spotifai export (Spotify)");
let _scope = output::scope("export");
output::action("fetching liked tracks");
output::detail("312 liked tracks");
output::action("fetching playlists");
output::detail("8 playlists");
output::status("exported 312 liked items, 47 albums, 8 playlists");
```

Renders as:

```
══ spotifai export (Spotify)
  → fetching liked tracks
    · 312 liked tracks
  → fetching playlists
    · 8 playlists
  ✓ exported 312 liked items, 47 albums, 8 playlists
```

Indentation rules:

- Top-level lines start at column 0; each scope adds two spaces.
- `detail` always adds one *extra* level of indent on top of the active
  scope, so a detail under a top-level action sits at column 2 and a
  detail inside a scope sits at column 4.
- Scopes nest. Three levels deep is fine; five levels is a smell. If a
  command needs more than three nested scopes, the operation is
  probably under-decomposed.
- Scopes are thread-local. Output emitted from a tokio task on a
  different worker thread will see depth `0`. If a command needs to
  emit from a worker thread, it should drive the print from the awaited
  result on the main task instead. (spotifai already does this — every
  scope brackets a `block_on(...)` call, not the futures inside it.)
- A `section(msg)` call is sugar for `header(msg)` + `scope(msg)`. Use
  it when every line that follows belongs under the section title.

## Streams

| Stream | What goes there                                                                          |
|--------|------------------------------------------------------------------------------------------|
| stdout | Machine-readable contract output only — JSON from `spotifai api` / `spotifai export`, version banner, `--help-agent` / `--debug-agent` payloads, the `commands` index, embedded manpages and docs. **No ANSI escapes.** |
| stderr | Every human-facing message — header, status, info, action, detail, hint, warn, error, debug-on-`--debug`. Styled when the gate above passes. |
| File   | Every level (including `debug`), every helper, always on. No styling. |

Why this matters: `spotifai export | jq …` and `spotifai api … > out.json`
must work without spotifai poisoning stdout. Tests in `tests/api_test.rs`
and `tests/export_test.rs` parse stdout as JSON; one stray `info` line
on stdout breaks them. Treat stdout as if a parser is reading it,
because one is.

## Input

The output module also owns input. Two helpers:

- `prompt(label) -> Result<String>` — write `label: ` to stderr (with
  styling), flush, read one line from stdin, trim it, return it.
  Errors when stdin is closed or empty. Used by `spotifai auth` for
  client-id / client-secret entry.
- `confirm(question, default) -> Result<bool>` — write `question
  [y/N]: ` (or `[Y/n]:` if `default == true`) to stderr, read one line,
  parse `y`/`yes` (case-insensitive) as true, `n`/`no` as false; an
  empty line returns `default`.

Both helpers honor the same color gate as the rest of the module — the
prompt is styled when stderr is a TTY and plain otherwise. Both write
to stderr (not stdout) so prompt text never poisons a piped JSON
contract.

If stdin is not a terminal, callers should not call `prompt` /
`confirm` at all — the right shape is to require the value via flag or
env var instead. spotifai's `auth` flow exemplifies this: every input
prompt has a corresponding `--client-id` / `--client-secret` flag for
non-interactive use.

## Structured fields in the file log

`tracing-subscriber` writes the file log with the default `fmt` layer
plus `with_target(true)` and `with_level(true)`. Every helper attaches
two structured fields for grep-ability:

- `kind` — the helper that produced the line (`"header"`, `"action"`,
  `"status"`, `"info"`, `"detail"`, `"hint"`, `"warn"`, `"error"`,
  `"debug"`, `"step"`).
- `scope` — the joined active scope stack at emit time
  (`"export"`, `"export.spotify"`, `""` for top-level).

A typical line:

```
2026-05-14T17:42:00.123Z  INFO spotifai::output: kind="action" scope="export" fetching liked tracks
```

Filter the live log with `tail -F debug.log | grep 'kind="warn"'` or
`grep 'scope="export'`. Do not change the field names without bumping
the helper API.

Modules that need to record events that *aren't* user-visible — e.g.
"refresh-token rotation succeeded", "rate-limit precall_check found a
deadline 12s in the future" — should call `tracing::debug!` directly
with their module path as the `target` and structured fields for the
interesting values. Those events show up in `debug.log` only and never
on the terminal.

## File log

Per `OSS_SPEC.md` §19.2, every run appends to a persistent log:

| Platform | Path                                                       |
|----------|------------------------------------------------------------|
| Linux    | `~/.local/state/spotifai/debug.log`                        |
| macOS    | `~/Library/Application Support/spotifai/debug.log`         |
| Windows  | `%APPDATA%\spotifai\debug.log`                             |

The file is never rotated by spotifai itself. Truncate manually
(`: > <path>`) or wire up `logrotate`. The `--debug` flag mirrors
debug-level events to stderr; the file always captures them regardless.

`SPOTIFAI_LOG` accepts a `tracing_subscriber::EnvFilter` directive
(e.g. `SPOTIFAI_LOG=spotifai=trace,zad=debug`) for surgical filtering.
Default: `debug`.

## Environment variables

| Variable          | Default | Description                                                                            |
|-------------------|---------|----------------------------------------------------------------------------------------|
| `NO_COLOR`        | unset   | Any non-empty value disables ANSI on stderr (https://no-color.org).                    |
| `SPOTIFAI_COLOR`  | `auto`  | `auto` (TTY-detect), `always` (force on), `never` (force off). Overrides `NO_COLOR`.   |
| `SPOTIFAI_GLYPHS` | `auto`  | `auto` (unicode unless ASCII-only locale), `unicode`, `ascii`.                         |
| `SPOTIFAI_LOG`    | `debug` | `tracing_subscriber::EnvFilter` directive controlling the file-log writer's verbosity. |

## Cheat sheet

```rust
use spotifai::output;

output::header("spotifai export (Spotify)");
let _scope = output::scope("export");

output::info(&format!("permissions: {}", path.display()));

output::action("fetching liked tracks");
let tracks = client.saved_tracks(...).await?;
output::detail(&format!("{} liked tracks", tracks.len()));

if !something_optional {
    output::hint("pass --pretty to format the JSON for human reading");
}

if duplicate {
    output::warn(&format!("playlist `{name}` already exists — skipping"));
}

output::status(&format!("exported {n} items"));
```

For input:

```rust
let id = output::prompt("Spotify client_id")?;
if output::confirm("overwrite existing tokens?", false)? {
    write_tokens(...)?;
}
```

## Reusing this template in another project

Copy `src/output.rs`, `src/logging.rs`, `tests/output_test.rs`,
`tests/logging_test.rs`, and this document into the target project.
Then:

1. Replace every `spotifai`-shaped string (env-var prefix, log
   directory, tracing target) with the new project's name. The grep
   list: `SPOTIFAI_LOG`, `SPOTIFAI_COLOR`, `SPOTIFAI_GLYPHS`,
   `spotifai::output`, the `dirs::state_dir().join("spotifai")` paths.
2. Wire `logging::init(debug)` into the CLI entry point before any
   command runs. (spotifai does this in `cli::run`.)
3. Audit existing call sites against the decision tree above. Most
   `println!` / `eprintln!` calls map onto `info` or `debug`; very few
   are genuinely `status`. Be ruthless — most existing log lines do
   not earn their place.
4. Add the module to your CI: `grep -rE 'println!|eprintln!' src/`
   should return zero hits outside of the output module itself and any
   contract surface that intentionally writes plain stdout (the
   `--help-agent` / `--debug-agent` printers in spotifai are the
   precedent).

The discipline scales down (a single-binary CLI) and up (a workspace
with several binaries that share the same output module). The
file-log location, color-detection precedence, and stream policy do
not change between project sizes — only the name does.
