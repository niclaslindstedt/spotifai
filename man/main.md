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
| `install` | Walk the four-step setup that makes `spotifai api` usable: download the pinned zad binary into `~/.spotifai/bin/zad`, bootstrap the local Ed25519 signing key, scaffold `~/.spotifai/permissions.toml`, and sign it so zad's load-time trust check passes. Idempotent. |
| `auth`    | Forward to `zad service create spotify` (global scope) to register a Spotify Client ID and run the OAuth 2.0 PKCE flow. |
| `api`     | Forward to `zad spotify …` after verifying the pinned zad binary. The forwarded process gets `ZAD_PERMISSIONS_PATH=~/.spotifai/permissions.toml` so the same policy applies regardless of cwd. |
| `ask`     | Start an interactive zag session about the user's Spotify library, with `~/.spotifai/permissions.toml` injected into the system prompt. |
| `help`    | Show help text. |

### `spotifai install`

Walks a four-step guided setup. Each step prints a header so a first-time user can see what is happening.

1. **Install zad binary.** Ensures `~/.spotifai/bin/zad` matches the version pinned in `.zadrc` (baked in at build time). Spotifai forward-routes its `api …` subcommands to this exact path, so the binary on `$PATH` is intentionally never used. Re-runs are no-ops once the right version is present.
2. **Bootstrap signing key.** Runs `zad signing init`, which mints a fresh Ed25519 keypair in the OS keychain (account `signing:v1`) and writes a self-signed empty trust store at `~/.zad/signing/trusted.toml`. Idempotent — when a key already exists, the command prints its fingerprint and leaves the keychain untouched.
3. **Write default permissions.** Writes a read-only `~/.spotifai/permissions.toml` if no file exists yet. The default policy allows `search`, `playlists list/show`, and `library tracks/albums list`; every mutating verb is denied. Hand-edits to an existing file are preserved across re-runs.
4. **Sign permissions file.** Runs `zad spotify permissions sign --local` with `ZAD_PERMISSIONS_PATH` pinned at the spotifai-managed file. zad ≥ 0.4.0 fails closed at load time on permission files that are not in the per-machine trust store; signing here is what unblocks the first `spotifai api …` call. The step runs unconditionally on every `install` invocation, so re-running `spotifai install` after a hand-edit resigns the file.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--force` | bool | false | Re-download the zad binary even if the existing one already matches the pinned version. (Does not overwrite an existing permissions file; signing always re-runs regardless.) |

### `spotifai auth`

Runs `~/.spotifai/bin/zad service create spotify` to register Spotify OAuth credentials (Client ID + refresh token) at zad's global scope. Spotify only issues one developer app per user, so the credential intentionally lives at `~/.zad/services/spotify/config.toml` and applies to every directory `spotifai api …` is invoked from. See [`auth.md`](auth.md) for the full reference, including which zad flags pass through.

### `spotifai api`

Forward-routes everything after `api` to `~/.spotifai/bin/zad spotify …`, with `ZAD_PERMISSIONS_PATH` injected so the spotifai-managed permissions file at `~/.spotifai/permissions.toml` is consulted regardless of cwd. See [`api.md`](api.md) for the full reference.

### `spotifai ask`

Start an interactive zag session pre-loaded with a system prompt that explains how to use `spotifai api …` and injects `~/.spotifai/permissions.toml` so the agent self-restricts to the listed verbs.

| Argument | Type | Default | Description |
|---|---|---|---|
| `[query…]` | string | — | Optional opening question. Joined with spaces and used as the agent's first turn. With no argument the session opens empty and waits for the user to type. |

The agent talks to Spotify only through `spotifai api …` (no direct Spotify Web API calls), and is instructed in the system prompt never to widen the policy itself. To loosen the surface, edit `~/.spotifai/permissions.toml` directly — the file is re-read on every `spotifai ask` invocation. Run `spotifai install --force` to rewrite the binary; the permissions file is never overwritten without your edit.

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

- [`auth.md`](auth.md) — `spotifai auth` reference
- [`api.md`](api.md) — `spotifai api` reference
- [`ask.md`](ask.md) — `spotifai ask` reference
- `spotifai commands`
- `spotifai docs`