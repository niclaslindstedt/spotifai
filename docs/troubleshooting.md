# Troubleshooting

Common failure modes and how to fix them. Each entry should have:

- **Symptom** — what the user sees
- **Cause** — what's actually wrong
- **Fix** — how to resolve it
- **Prevention** — how to avoid it next time

## First step: read the debug log

Per OSS_SPEC §19, every `spotifai` invocation appends to a persistent
debug log so post-hoc triage doesn't require re-running with extra
flags. Find it before opening an issue:

| Platform | Path |
|---|---|
| Linux   | `~/.local/state/spotifai/debug.log` |
| macOS   | `~/Library/Application Support/spotifai/debug.log` |
| Windows | `%APPDATA%\spotifai\debug.log` |

To follow events live, add `--debug` to the command — it echoes the
same `debug` events to stderr that already land in the file:

```sh
spotifai --debug ask "list my playlists"
```

Adjust verbosity with `SPOTIFAI_LOG`, e.g.
`SPOTIFAI_LOG=spotifai=trace,zad=debug`.
