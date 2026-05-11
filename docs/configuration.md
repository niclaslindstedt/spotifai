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

| Knob | Type | Default | Description |
|------|------|---------|-------------|
| `--yolo` | flag | off | Forward `auto_approve(true)` to the underlying zag `AgentBuilder` so the interactive surfaces (`spotifai ask`, `spotifai playlist`) skip every per-tool approval prompt. The `(provider, profile)` policy file is still enforced by `spotifai api` at the zad layer, so `--yolo` cannot widen the allowed verb list — it only suppresses zag's tool-approval gating on top. No-op for the one-shot commands (`api`, `auth`, `export`, `import`, `install`) since they do not spawn zag. Global — works on every subcommand. |

## Rate-limit coordination

zad 0.8.0 records the deadline from any 429 response at
`~/.zad/state/<service>/rate_limit.json` and exposes a precall check so
every caller — inside the current process and any sibling `spotifai api`
shell — can gate its calls behind the shared deadline.

| Knob | Type | Default | Description |
|------|------|---------|-------------|
| `--wait` | flag | (see below) | When the active provider is in a 429 cooldown window, sleep until the deadline and continue instead of failing fast. No-op when no cooldown is recorded. Default: `true` for the interactive surfaces (`ask`, `playlist`) so multiple sub-agents coordinate cleanly; `false` for the one-shot commands (`api`, `export`, `import`) so a user-driven invocation surfaces 429s loudly. The `SPOTIFAI_WAIT` env var overrides the default; an explicit flag overrides both. Global — works on every subcommand. |
| `--no-wait` | flag | (see above) | Force fail-fast behaviour even when `SPOTIFAI_WAIT=1` is set. Mutually exclusive with `--wait`. |
| `SPOTIFAI_WAIT` | env | unset | Read by every `spotifai` invocation to decide whether to sleep through an active 429 cooldown (`1`/`true`/`yes`/`on` → wait; `0`/`false`/`no`/`off` → fail-fast). Set on the user's behalf by `spotifai ask` and `spotifai playlist` to `1` so child `spotifai api` shells coordinate. The CLI `--wait` / `--no-wait` flags override the env var. |

## Output

| Key (`config.toml`) | Environment variable | Type | Default | Description |
|---------------------|---------------------|------|---------|-------------|
| `output` | `SPOTIFAI_OUTPUT` | `text` \| `json` | `text` | Default output format. Overridable per-invocation with `--output`. |

## Logging

Per OSS_SPEC §19, `spotifai` always writes a debug-level log to a
persistent file. Verbosity on the terminal is controlled by `--debug`;
the file log captures everything regardless.

| Platform | Default path |
|---|---|
| Linux   | `~/.local/state/spotifai/debug.log` |
| macOS   | `~/Library/Application Support/spotifai/debug.log` |
| Windows | `%APPDATA%\spotifai\debug.log` |

| Knob | Type | Default | Description |
|------|------|---------|-------------|
| `--debug` | flag | off | Echo `debug`-level events to stderr in addition to the file log. |
| `SPOTIFAI_LOG` | env | `debug` | [`tracing_subscriber::EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html) directive controlling the file writer. Examples: `SPOTIFAI_LOG=info` quiets the log; `SPOTIFAI_LOG=spotifai=trace,zad=debug` enables fine-grained traces for spotifai while keeping zad at `debug`. |

The file is append-only — no rotation in v1. Truncate manually
(`: > ~/.local/state/spotifai/debug.log`) or pipe through `logrotate`
if you need to bound its size.

## Example `config.toml`

```toml
model         = "claude-sonnet-4-6"
output        = "text"
```

Provider credentials are not configured here — they live in the OS keychain and are written by `spotifai auth`.
