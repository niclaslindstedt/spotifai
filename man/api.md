# spotifai api

> Forward-route provider subcommands to the pinned zad binary.

## Synopsis

```
spotifai api [ARGS]...
```

## Description

`spotifai api` is a thin shim over `~/.spotifai/bin/zad <provider>`, where `<provider>` is the active provider (`spotify`, `ymusic`, …) selected by the parent spotifai command. Everything after `api` is passed through verbatim, so `spotifai api playlists list` becomes `~/.spotifai/bin/zad <provider> playlists list`.

Before exec'ing zad, spotifai performs the same install/version check as `spotifai install`:

1. If `~/.spotifai/bin/zad` is missing, the release tagged in `.zadrc` is downloaded into place.
2. If the binary exists, its `--version` is compared against the pinned tag (a leading `v` on either side is ignored). On a mismatch, the pinned release is downloaded and the wrong-version binary is replaced.
3. Once the managed path holds the pinned version, zad is invoked there. The binary on `$PATH` is intentionally never used, so a globally-installed zad with a different schema or permission policy cannot be picked up by accident.

Zad's stdout, stderr, and exit code are propagated verbatim — `spotifai api` returns whatever zad returned.

`spotifai api` requires a parent spotifai command to have selected a permission profile. `spotifai ask`, `spotifai playlist`, and `spotifai export` set `SPOTIFAI_PROFILE` to `ask` or `playlist` and `SPOTIFAI_PROVIDER` to the active provider slug before launching zag; child shells then inherit both variables when the agent runs `spotifai api …`. Direct invocations from a user shell exit with a usage error (code `2`) if `SPOTIFAI_PROFILE` is unset — there is intentionally no implicit default for the profile axis. `SPOTIFAI_PROVIDER` falls back to `spotify` when unset, for backwards compatibility with shells written against the original Spotify-only `spotifai`.

To call zad outside spotifai, run `~/.spotifai/bin/zad <provider> …` and set `ZAD_PERMISSIONS_PATH` yourself.

The forwarded zad process is always launched with `ZAD_PERMISSIONS_PATH` pinned at the matching `~/.spotifai/permissions/<provider>/<profile>.toml`. spotifai overrides any inherited `ZAD_PERMISSIONS_PATH` so an agent cannot escalate by setting the zad variable itself before invoking the shim. zad ≥ 0.3.0 honours this variable as an explicit local-permissions override, so the spotifai-managed policy file applies regardless of which directory `spotifai api` is invoked from.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `[ARGS]...` | trailing | — | Arguments forwarded as-is to `zad <provider>`. Hyphen-prefixed flags are accepted (zad does its own parsing); use `--` to defensively split spotifai's args from zad's. |

## Flags

`spotifai api` itself takes no flags. Anything that looks like a flag after the `api` keyword is forwarded to zad. To see zad's own flags:

```sh
spotifai api --help
```

There is intentionally **no** `--provider` flag on `api`: clap's trailing-var-arg parsing would swallow it. Use `SPOTIFAI_PROVIDER` (or, more typically, just the parent `--provider` flag on `ask` / `playlist` / `export`).

## Environment variables

`spotifai api` reads two variables of its own and sets one on the forwarded child:

| Variable | Read / set | Description |
|---|---|---|
| `SPOTIFAI_PROVIDER` | read | Selects which zad subcommand (`spotify` / `ymusic` / …) and which `~/.spotifai/permissions/<provider>/` directory to point zad at. Set by `spotifai ask` / `playlist` / `export` on the user's behalf. Unset is treated as `spotify` for backwards compatibility; an unknown value fails with a usage error. |
| `SPOTIFAI_PROFILE` | read | Selects which `<profile>.toml` to point zad at. Set by `spotifai ask` (`ask`) and `spotifai playlist` (`playlist`) on the user's behalf. Treated as an internal coupling, not a user knob: missing or unknown values fail with a usage error. |
| `ZAD_PERMISSIONS_PATH` | set | Forwarded to the zad child as `~/.spotifai/permissions/<provider>/<profile>.toml`. Always overrides any inherited value so an agent cannot escalate by setting the zad variable itself before invoking the shim. zad ≥ 0.3.0 reads this variable as an explicit override that bypasses the cwd-derived project slug. |

The forwarded zad process otherwise inherits the current environment, so any variables zad itself consults (OAuth tokens, keychain hints, etc.) are honoured.

## Exit codes

| Code | Meaning |
|---|---|
| 0   | zad exited successfully. |
| 1   | Generic spotifai error (download/install failure, missing home directory, version mismatch after re-download, missing per-profile permissions file). |
| 2   | Usage error parsing `spotifai api` itself, or `SPOTIFAI_PROFILE` is unset / unknown. |
| *N* | Any other code is propagated verbatim from `zad <provider> …`. |

## Examples

List your playlists (active provider; defaults to Spotify):

```sh
spotifai api playlists list
```

Search the catalogue:

```sh
spotifai api search "billie jean"
```

Pass JSON output flags through to zad:

```sh
spotifai api playlists list --json
```

Force-defer all parsing to zad with `--`:

```sh
spotifai api -- tracks --limit=10
```

Drive a YouTube Music call directly (rare; usually go through `spotifai ask --provider ymusic`):

```sh
SPOTIFAI_PROVIDER=ymusic SPOTIFAI_PROFILE=ask spotifai api playlists list --json
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`auth.md`](auth.md) — register OAuth credentials before running `api …`
- [`ask.md`](ask.md) — read-only agent surface that drives `spotifai api` for you
- [`playlist.md`](playlist.md) — playlist-builder agent surface
- [`spotifai install`](main.md#spotifai-install) — the same install/version check, run on its own
- [`.zadrc`](../.zadrc) — pinned zad release tag
