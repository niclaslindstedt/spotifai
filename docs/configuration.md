# Configuration

spotifai is configured through environment variables and an optional TOML file at `~/.config/spotifai/config.toml`. Environment variables always take precedence over the file.

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
