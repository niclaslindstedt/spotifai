# spotifai api

> Forward-route Spotify subcommands to the pinned zad binary.

## Synopsis

```
spotifai api [ARGS]...
```

## Description

`spotifai api` is a thin shim over `~/.spotifai/bin/zad spotify`. Everything after `api` is passed through verbatim, so `spotifai api playlists list` becomes `~/.spotifai/bin/zad spotify playlists list`.

Before exec'ing zad, spotifai performs the same install/version check as `spotifai install`:

1. If `~/.spotifai/bin/zad` is missing, the release tagged in `.zadrc` is downloaded into place.
2. If the binary exists, its `--version` is compared against the pinned tag (a leading `v` on either side is ignored). On a mismatch, the pinned release is downloaded and the wrong-version binary is replaced.
3. Once the managed path holds the pinned version, zad is invoked there. The binary on `$PATH` is intentionally never used, so a globally-installed zad with a different schema or permission policy cannot be picked up by accident.

Zad's stdout, stderr, and exit code are propagated verbatim — `spotifai api` returns whatever zad returned.

The forwarded zad process is launched with `ZAD_PERMISSIONS_PATH=~/.spotifai/permissions.toml`. zad ≥ 0.3.0 honours this variable as an explicit local-permissions override, so the spotifai-managed policy file applies regardless of which directory `spotifai api` is invoked from — there is no cwd-dependent project slug to worry about.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `[ARGS]...` | trailing | — | Arguments forwarded as-is to `zad spotify`. Hyphen-prefixed flags are accepted (zad does its own parsing); use `--` to defensively split spotifai's args from zad's. |

## Flags

`spotifai api` itself takes no flags. Anything that looks like a flag after the `api` keyword is forwarded to zad. To see zad's own flags:

```sh
spotifai api --help
```

## Environment variables

`spotifai api` does not read any environment variables of its own. The forwarded zad process inherits the current environment, so any variables zad consults (Spotify OAuth tokens, keychain hints, etc.) are honoured.

Spotifai sets one variable on the forwarded child:

| Variable | Set to | Why |
|---|---|---|
| `ZAD_PERMISSIONS_PATH` | `~/.spotifai/permissions.toml` | Pins zad's local-permissions lookup to the spotifai-managed file so the same policy applies in every cwd. zad ≥ 0.3.0 reads this variable as an explicit override that bypasses the cwd-derived project slug. If the user has already exported `ZAD_PERMISSIONS_PATH`, spotifai's value wins for the duration of the forwarded process. |

## Exit codes

| Code | Meaning |
|---|---|
| 0   | zad exited successfully. |
| 1   | Generic spotifai error (download/install failure, missing home directory, version mismatch after re-download). |
| 2   | Usage error parsing `spotifai api` itself. |
| *N* | Any other code is propagated verbatim from `zad spotify …`. |

## Examples

List your playlists:

```sh
spotifai api playlists list
```

Search the Spotify catalogue:

```sh
spotifai api tracks search "billie jean"
```

Pass JSON output flags through to zad:

```sh
spotifai api playlists list --json
```

Force-defer all parsing to zad with `--`:

```sh
spotifai api -- tracks --limit=10
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`auth.md`](auth.md) — register Spotify OAuth credentials before running `api …`
- [`spotifai install`](main.md#spotifai-install) — the same install/version check, run on its own
- [`.zadrc`](../.zadrc) — pinned zad release tag
