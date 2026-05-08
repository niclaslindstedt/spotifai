# Configuration

spotifai is configured through environment variables and an optional TOML file at `~/.config/spotifai/config.toml`. Environment variables always take precedence over the file.

## Permissions file (`~/.spotifai/permissions.toml`)

`spotifai ask` reads this TOML file and injects it into the agent's system prompt so the agent self-restricts to the listed `spotifai api` verbs. It is created with a read-only default the first time `spotifai install` runs; subsequent runs leave any hand edits in place.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mode` | string | `read_only` | Free-form tag for the policy. Informational — the effective gate is the `allowed` / `denied` lists below. |
| `description` | string | "Read-only access…" | Human-readable summary, embedded verbatim in the system prompt so the agent can quote the policy back to the user. |
| `allowed` | `[string]` | read-only verbs | `spotifai api` verbs the agent is allowed to invoke (e.g. `playlists list`, `library tracks list`). Strings are the literal subcommand path after `spotifai api `. |
| `denied`  | `[string]` | every write verb | Verbs the agent must refuse to invoke. Deny always wins. |

The default policy allows `search`, `playlists list`, `playlists show`, `library tracks list`, `library albums list`, and denies every mutating verb (`playlists create|rename|delete|add|remove`, `library tracks save|unsave`, `library albums save|unsave`).

This file is **advisory** — it constrains the agent via prompt injection but is not enforced by zad. zad's runtime gate continues to be the signed `~/.zad/services/spotify/permissions.toml` file. To widen the spotifai surface, edit `allowed` / `denied` directly, then re-run `spotifai install` so the file is resigned and zad's load-time trust check accepts it. spotifai re-reads the file on every `spotifai ask` invocation. To rewrite the spotifai file back to the read-only default, delete it and re-run `spotifai install`.

### Signature

zad ≥ 0.4.0 fails closed on any permissions file referenced by `ZAD_PERMISSIONS_PATH` that is not in the per-machine trust store at `~/.zad/signing/trusted.toml`. `spotifai install` handles this for you in two steps:

1. **Bootstrap** — runs `zad signing init` (idempotent), which mints an Ed25519 keypair into the OS keychain (account `signing:v1`) and writes a self-signed empty trust store.
2. **Sign** — runs `zad spotify permissions sign --local` with `ZAD_PERMISSIONS_PATH` pinned at `~/.spotifai/permissions.toml`, which adds a `[signature]` block to the file and upserts the trust-store entry. Hand edits invalidate the signature, so re-run `spotifai install` after editing.

## Spotify credentials

Spotify credentials are managed by zad, not spotifai. Run [`spotifai auth`](../man/auth.md) to register them; the command forwards to `zad service create spotify` at zad's **global** scope (no `--local`) and stores the Client ID and OAuth refresh token in your OS keychain. The resulting config lives at `~/.zad/services/spotify/config.toml` and applies to every directory `spotifai api …` is invoked from.

zad uses an OAuth 2.0 PKCE *public-client* flow, so there is no `client_secret` and no fixed redirect-URI port: the loopback listener picks a random port on `127.0.0.1` at runtime. Just register the host `http://127.0.0.1` in your Spotify dashboard once.

## zad permissions path

`spotifai api` sets one environment variable on the forwarded zad child:

| Variable | Value | Description |
|---|---|---|
| `ZAD_PERMISSIONS_PATH` | `~/.spotifai/permissions.toml` | Pins zad's local-permissions lookup to the spotifai-managed file so the same policy applies regardless of cwd. zad ≥ 0.3.0 reads this variable as an explicit override that bypasses the cwd-derived project slug. |

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

Spotify credentials are not configured here — they live in the OS keychain and are written by `spotifai auth`.
