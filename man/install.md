# spotifai install

> Bootstrap the per-machine signing key, scaffold every per-(provider, profile) permissions file, and sign each one.

## Synopsis

```
spotifai install
```

## Description

`spotifai install` walks a three-step guided setup that makes the agent surfaces (`ask`, `playlist`, `export`, `import`) usable on this machine. Each step prints a header so a first-time user can see what is happening. The command is fully **idempotent** — re-running it on a machine that is already set up only resigns existing permissions files and is otherwise a no-op.

1. **Bootstrap signing key.** Mints a fresh Ed25519 keypair in the OS keychain (account `zad/signing:v1`) via `zad::permissions::signing::load_or_create_from_keychain`, materializes the per-machine trust store at `~/.zad/signing/trusted.toml`, and writes the public-key cache at `~/.zad/signing/public_key.toml`. When a key already exists, the call returns its fingerprint and leaves the keychain untouched.

2. **Write default permission profiles.** Scaffolds every `<provider>/<profile>.toml` file under `~/.spotifai/permissions/`. For each supported provider (`spotify`, `ymusic`) and each profile (`ask`, `playlist`):

   - `ask.toml` ships read-only — allows `search`, `playlists list`, `playlists show`, plus the read-side library verbs (`library tracks/albums list` on Spotify, `library list` on YouTube Music).
   - `playlist.toml` adds `playlists create`, `playlists add`, and `playlists rename` for the agent-driven `spotifai playlist` surface. Destructive verbs (`playlists delete`, `playlists remove`) and library writes stay denied.

   Verb names differ between providers — Spotify exposes `library tracks list` and `library albums list`; YouTube Music exposes a single `library list` over rated videos. Hand-edits to existing files are preserved across re-runs.

3. **Sign permission profiles.** Calls `zad::permissions::signing::sign_unsigned` once per `(provider, profile)` pair, then upserts the resulting signature into the per-machine trust store at `~/.zad/signing/trusted.toml`. zad ≥ 0.4.0 fails closed at load time on permission files whose signatures are not registered in the trust store, so this step is what unblocks the first call from any agent surface. The step runs **unconditionally** on every `install` invocation — re-run `spotifai install` after any hand-edit to resign every file.

## Arguments

`spotifai install` takes no positional arguments.

## Flags

`spotifai install` takes no flags.

## Environment variables

`spotifai install` reads no environment variables of its own. The OS keychain backend (`secret-service` on Linux, Keychain on macOS, Credential Manager on Windows) is selected by the runtime in the usual platform-default way.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | All three steps succeeded. |
| 1 | Generic error: keychain unavailable, trust-store write failure, missing home directory, or signing failure. |
| 2 | Usage error parsing `spotifai install` itself (none today — the command takes no flags). |

## Files written

| Path | Contents |
|---|---|
| OS keychain (account `zad/signing:v1`) | Ed25519 keypair used to sign permissions files. |
| `~/.zad/signing/trusted.toml`     | Per-machine trust store. zad's load-time gate rejects any permissions file whose signature is not listed here. |
| `~/.zad/signing/public_key.toml`  | Cached public key for external tooling. |
| `~/.spotifai/permissions/spotify/ask.toml`     | Spotify read-only profile. |
| `~/.spotifai/permissions/spotify/playlist.toml`| Spotify playlist-curator profile. |
| `~/.spotifai/permissions/ymusic/ask.toml`      | YouTube Music read-only profile. |
| `~/.spotifai/permissions/ymusic/playlist.toml` | YouTube Music playlist-curator profile. |

## Examples

First-time setup on a fresh machine:

```sh
spotifai install
```

Re-run after editing one of the permissions files (resigns every profile so zad accepts them again):

```sh
$EDITOR ~/.spotifai/permissions/spotify/ask.toml
spotifai install
```

## See also

- [`main.md`](main.md) — top-level `spotifai` reference
- [`auth.md`](auth.md) — register provider OAuth credentials after install
- [`ask.md`](ask.md) / [`playlist.md`](playlist.md) — the agent surfaces unblocked by install
- [`../docs/configuration.md`](../docs/configuration.md) — the per-(provider, profile) permissions schema
