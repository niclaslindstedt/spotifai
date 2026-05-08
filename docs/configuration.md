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

This file is **advisory** — it constrains the agent via prompt injection but is not enforced by zad. zad's runtime gate continues to be the signed `~/.zad/services/spotify/permissions.toml` file. To widen the spotifai surface, edit `allowed` / `denied` directly; spotifai re-reads the file on every `spotifai ask` invocation. To rewrite the spotifai file back to the read-only default, delete it and re-run `spotifai install`.

## Spotify credentials

| Key (`config.toml`) | Environment variable | Type | Default | Description |
|---------------------|---------------------|------|---------|-------------|
| `client_id` | `SPOTIFY_CLIENT_ID` | string | — | Spotify app client ID. **Required.** |
| `client_secret` | `SPOTIFY_CLIENT_SECRET` | string | — | Spotify app client secret. **Required.** |
| `redirect_uri` | `SPOTIFY_REDIRECT_URI` | string | `http://localhost:8888/callback` | OAuth redirect URI — must match the value in your Spotify app dashboard. |

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
client_id     = "abc123"
client_secret = "def456"
redirect_uri  = "http://localhost:8888/callback"
model         = "claude-sonnet-4-6"
output        = "text"
```
