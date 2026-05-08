# Configuration

spotifai is configured through environment variables, an optional TOML file at `~/.config/spotifai/config.toml`, and per-`(provider, profile)` permission files. Environment variables always take precedence over the config file.

## Permissions files (`~/.spotifai/permissions/<provider>/`)

Each agent surface (`spotifai ask`, `spotifai playlist`) has its own per-profile permissions file, scoped under the active provider. The matching file is injected into the agent's system prompt so it self-restricts to the listed `spotifai api` verbs, and is also pointed at by `ZAD_PERMISSIONS_PATH` when the agent shells out, so zad's load-time gate sees the same policy.

| Provider × Profile | Path | Default surface |
|---|---|---|
| `spotify` × `ask`       | `~/.spotifai/permissions/spotify/ask.toml`      | Read-only — `search`, `playlists list/show`, `library tracks/albums list`. |
| `spotify` × `playlist`  | `~/.spotifai/permissions/spotify/playlist.toml` | Read + `playlists create`, `playlists add`, `playlists rename`. Destructive verbs (`delete`, `remove`) and library writes stay denied. |
| `ymusic` × `ask`        | `~/.spotifai/permissions/ymusic/ask.toml`       | Read-only — `search`, `playlists list/show`, `library list`. |
| `ymusic` × `playlist`   | `~/.spotifai/permissions/ymusic/playlist.toml`  | Read + `playlists create`, `playlists add`, `playlists rename`. Destructive verbs and `library like|unlike` stay denied. |

All files share the same TOML schema:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mode` | string | `read_only` (ask) / `playlist_curator` (playlist) | Free-form tag for the policy. Informational — the effective gate is the `allowed` / `denied` lists below. |
| `description` | string | per-profile blurb | Human-readable summary, embedded verbatim in the system prompt so the agent can quote the policy back to the user. |
| `allowed` | `[string]` | per-(provider, profile) list | `spotifai api` verbs the agent is allowed to invoke (e.g. `playlists list`, `playlists create`). Strings are the literal subcommand path after `spotifai api `. |
| `denied`  | `[string]` | per-(provider, profile) list | Verbs the agent must refuse to invoke. Deny always wins. |

All four files are created with their defaults the first time `spotifai install` runs; subsequent runs leave any hand edits in place. To widen or narrow any profile, edit `allowed` / `denied` directly, then re-run `spotifai install` so the file is resigned and zad's load-time trust check accepts it. spotifai re-reads the matching file on every `spotifai ask` / `spotifai playlist` / `spotifai export` invocation. To rewrite a file back to its default, delete it and re-run `spotifai install`.

These files are **advisory** — they constrain the agent via prompt injection but are not the authoritative gate. zad's runtime check at load time is what fails closed; spotifai's role is to keep the agent from proposing a forbidden verb in the first place.

### Signature

zad ≥ 0.4.0 fails closed on any permissions file referenced by `ZAD_PERMISSIONS_PATH` that is not in the per-machine trust store at `~/.zad/signing/trusted.toml`. `spotifai install` handles this for every (provider, profile) pair in two steps:

1. **Bootstrap** — runs `zad signing init` (idempotent), which mints an Ed25519 keypair into the OS keychain (account `signing:v1`) and writes a self-signed empty trust store.
2. **Sign** — runs `zad <provider> permissions sign --local` once per (provider, profile) pair with `ZAD_PERMISSIONS_PATH` pinned at the matching `~/.spotifai/permissions/<provider>/<profile>.toml`. Each call adds a `[signature]` block to that file and upserts the trust-store entry. Hand edits invalidate the signature, so re-run `spotifai install` after editing — every file is resigned on each install run.

## Provider credentials

Provider credentials are managed by zad, not spotifai. Run [`spotifai auth`](../man/auth.md) to register them; the command forwards to `zad service create <provider>` at zad's **global** scope (no `--local`). The resulting config lives at `~/.zad/services/<provider>/config.toml` and applies to every directory `spotifai api …` is invoked from.

- **Spotify** uses an OAuth 2.0 PKCE *public-client* flow, so there is no `client_secret` and no fixed redirect-URI port: the loopback listener picks a random port on `127.0.0.1` at runtime. Just register the host `http://127.0.0.1` in your Spotify dashboard once.
- **YouTube Music** (zad ≥ 0.6.0) uses OAuth 2.0 *Desktop-app* credentials issued by Google Cloud (`client_id` + `client_secret`), against the YouTube Data API v3.

## zad permissions path

`spotifai api` always sets `ZAD_PERMISSIONS_PATH` on the forwarded zad child:

| Variable | Value | Description |
|---|---|---|
| `ZAD_PERMISSIONS_PATH` | `~/.spotifai/permissions/<provider>/<profile>.toml` | Pins zad's local-permissions lookup to the file backing the active (provider, profile) pair. zad ≥ 0.3.0 reads this variable as an explicit override that bypasses the cwd-derived project slug. |

The active (provider, profile) pair is selected by the parent spotifai command:

| Variable | Description |
|---|---|
| `SPOTIFAI_PROVIDER` | Internal coupling between `spotifai ask` / `playlist` / `export` and `spotifai api`. The parent command sets this to `spotify`, `ymusic`, … before launching zag. Unset is treated as `spotify` for backwards compatibility; an unknown value fails with a usage error. |
| `SPOTIFAI_PROFILE`  | Internal coupling between `spotifai ask` / `playlist` and `spotifai api`. The parent command sets this to `ask` or `playlist` before launching zag, and `spotifai api` reads it to pick which file to point zad at. **Not a user knob** for the profile axis: direct `spotifai api …` invocations from a shell error out with a usage message. To call zad outside spotifai, run `~/.spotifai/bin/zad <provider> …` with `ZAD_PERMISSIONS_PATH` set yourself. |

## Agent (zag)

| Key (`config.toml`) | Environment variable | Type | Default | Description |
|---------------------|---------------------|------|---------|-------------|
| `model` | `SPOTIFAI_MODEL` | string | provider default | LLM model identifier forwarded to zag. |

## Output

| Key (`config.toml`) | Environment variable | Type | Default | Description |
|---------------------|---------------------|------|---------|-------------|
| `output` | `SPOTIFAI_OUTPUT` | `text` \| `json` | `text` | Default output format. Overridable per-invocation with `--output`. |

## Example `config.toml`

```toml
model         = "claude-sonnet-4-6"
output        = "text"
```

Provider credentials are not configured here — they live in the OS keychain and are written by `spotifai auth`.
