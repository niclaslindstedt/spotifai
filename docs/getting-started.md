# Getting started with spotifai

> A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).

## Install

```sh
cargo install spotifai
```

Confirm the binary is available:

```sh
spotifai --version
```

## Three agent surfaces, two providers

spotifai exposes the agent through three commands, each with its own permissions profile:

- **`spotifai ask`** is read-only. Use it for questions about your library.
- **`spotifai playlist`** can additionally create a new playlist, add tracks/videos to it, and rename it. Use it when you want the agent to build something for you.
- **`spotifai clean`** is the destructive surface. It can delete playlists, remove tracks from playlists, and unsave items from your library — but cannot create or add anything, and cannot search the public catalogue. The agent must enumerate the candidate set and wait for your explicit confirmation before every destructive call.

All three commands run the agent through `spotifai api …`, which dispatches typed calls into the in-process zad library. Direct `spotifai api …` invocations from a shell are intentionally rejected — they require a parent surface (`spotifai ask` / `spotifai playlist` / `spotifai clean`) to have selected a permissions profile.

All three commands take `--provider <slug>` (default: `spotify`). Today the supported providers are:

| Slug | Display name | Notes |
|---|---|---|
| `spotify` (default) | Spotify       | OAuth 2.0 PKCE, one developer app per user. |
| `ymusic`            | YouTube Music | Google OAuth 2.0 Desktop-app credentials, talks to YouTube Data API v3. |

## Set up the local toolchain

`spotifai install` walks a three-step guided setup:

```sh
spotifai install
```

It will, in order:

1. Mint the per-machine Ed25519 signing key in your OS keychain (account `zad/signing:v1`) and create the trust store at `~/.zad/signing/trusted.toml`.
2. Scaffold the per-(provider, profile) permissions files at `~/.spotifai/permissions/<provider>/ask.toml` (read-only), `~/.spotifai/permissions/<provider>/playlist.toml` (read + create/add/rename), and `~/.spotifai/permissions/<provider>/clean.toml` (read + destructive), for every supported provider. Existing files are left alone.
3. Sign each profile file with the keychain key and upsert the resulting signature into the trust store, so the in-process zad library accepts the file on every later call.

Re-run `spotifai install` whenever you edit a profile file — the signing step runs unconditionally and resigns every file in place.

## Create a developer app

### Spotify

Spotify hands out **one developer app per user**, so you do this once:

1. Go to [developer.spotify.com/dashboard](https://developer.spotify.com/dashboard) and log in.
2. Click **Create app**, give it any name (e.g. "spotifai-local").
3. Under **Redirect URIs**, add `http://127.0.0.1` and save. zad's loopback listener picks a random port; Spotify accepts any port on `127.0.0.1` once the host is registered.
4. Copy the **Client ID** from the app settings. (The Client Secret is unused — zad uses an OAuth 2.0 PKCE *public-client* flow.)

### YouTube Music

YouTube Music has no dedicated public API; zad talks to YouTube Data API v3 with Google OAuth credentials.

1. Go to [console.cloud.google.com](https://console.cloud.google.com/), create or pick a project, and enable **YouTube Data API v3** under "APIs & Services".
2. Under **Credentials**, click **Create credentials → OAuth client ID** and pick **Desktop app**. Save the **Client ID** + **Client secret**.
3. While the OAuth consent screen is in **Testing**, add yourself as a test user.

## First run

Authenticate. For Spotify, this opens a browser for the consent screen:

```sh
spotifai auth
```

For YouTube Music:

```sh
spotifai auth --provider ymusic
```

After granting access, spotifai captures the redirect on a `127.0.0.1:<random-port>` loopback listener (Spotify uses HTTPS with a per-session self-signed cert; YouTube Music uses HTTP), exchanges the authorization code for a refresh token, and stores it in your OS keychain under the `zad` service. Spotifai also probes `/me` (Spotify) or `/userinfo` + `/channels?mine=true` (YouTube Music) to capture the authenticated user/channel id and writes it to `~/.spotifai/<provider>.toml` for `playlists create` to consume later.

If you'd rather skip the interactive prompt, pass the credentials up front:

```sh
spotifai auth --client-id <your-client-id>
spotifai auth --provider ymusic --client-id <id> --client-secret <secret>
```

`--no-browser` keeps the auth URL in stderr only (useful when the loopback listener is reachable from another machine over SSH port-forwarding).

## Your first query

Read-only questions go through `spotifai ask`:

```sh
# Default: Spotify
spotifai ask "What playlists do I have?"

# YouTube Music
spotifai ask --provider ymusic "What playlists do I have?"

# Get JSON output for scripting
spotifai ask "List my saved albums" --output json | jq '.[].name'
```

Building a new playlist goes through `spotifai playlist`:

```sh
# Default: Spotify
spotifai playlist "a 30-minute focus playlist with no vocals"

# YouTube Music
spotifai playlist --provider ymusic "a 30-minute focus playlist with no vocals"

# Or open the session empty and chat
spotifai playlist
```

The agent in `spotifai playlist` can search the catalogue, look at your existing playlists for inspiration, create one new playlist, add tracks/videos to it, and rename it before it commits. It cannot delete playlists or remove items — those verbs stay denied even in this profile.

## Cleaning up your library

Destructive cleanup goes through `spotifai clean`:

```sh
# Default: Spotify
spotifai clean "remove all baby songs — my child is 15 now"

# YouTube Music
spotifai clean --provider ymusic "delete my 'old phone' playlist"

# Open the session empty
spotifai clean
```

The agent in `spotifai clean` can delete whole playlists, remove tracks from playlists, unsave tracks/albums (Spotify), and unlike videos (YouTube Music). It will always enumerate the candidate set first, show it to you, and ask "Proceed with deleting these N items? (yes/no)" before issuing any destructive call. `search`, `playlists create`, `playlists add`, `playlists rename`, and library `save`/`like` stay denied even in this profile — `clean` works on what you already have, not the public catalogue.

## Next steps

- [Configuration reference](configuration.md)
- [Architecture overview](architecture.md)
- [Troubleshooting](troubleshooting.md)
