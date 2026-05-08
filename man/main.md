# spotifai

> A Rust CLI for managing your Spotify library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify integration).

## Synopsis

```
spotifai [OPTIONS] [COMMAND]
```

## Description

_What this command does and when to reach for it._

## Subcommands

| Command | Description |
|---|---|
| `install` | Download the pinned zad binary into `~/.spotifai/bin/zad`. Idempotent. |
| `help` | Show help text. |

### `spotifai install`

Ensures the zad binary at `~/.spotifai/bin/zad` matches the version pinned in `.zadrc` (baked in at build time). Spotifai forward-routes its `api …` subcommands to this exact path, so the binary on `$PATH` is intentionally never used. Re-runs are no-ops once the right version is present.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--force` | bool | false | Re-download even if the existing binary already matches the pinned version. |

## Flags

| Flag | Type | Default | Description |
|---|---|---|---|
| `--version` | bool | false | Print version and exit. |
| `--help`    | bool | false | Print help and exit. |

## Environment variables

| Variable | Description |
|---|---|

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Generic error |
| 2 | Usage error |

## Examples

```sh
spotifai --help
```

## See also

- `spotifai commands`
- `spotifai docs`