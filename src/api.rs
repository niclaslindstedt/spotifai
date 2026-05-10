//! `spotifai api …` — typed dispatcher into the in-process zad
//! library.
//!
//! Parses the user-args grammar (`search "query"`, `playlists list`,
//! `playlists show <id>`, `playlists create --name "X"`,
//! `playlists add <id> <ids…>`, `library tracks list`,
//! `library albums list`, `library list` (ymusic)) into typed
//! `*Request` values and calls the matching method on
//! `zad::service::spotify::Spotify` / `zad::service::ymusic::Ymusic`.
//! Responses serialize to pretty JSON on stdout.
//!
//! `spotifai ask` and `spotifai playlist` spawn `spotifai api …`
//! through zag's shell tool; the command takes the active provider
//! from `SPOTIFAI_PROVIDER` and the active profile from
//! `SPOTIFAI_PROFILE`, both set by the parent surface.

use std::io::Write;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::permissions::Profile;
use crate::providers::Provider;
use crate::zad_client;

/// Env var read by `spotifai api` to pick which profile's policy
/// file backs the active call. Set by `spotifai ask` and
/// `spotifai playlist` before they spawn zag; an unset value is
/// treated as a usage error because there is no safe default.
pub const SPOTIFAI_PROFILE_ENV: &str = "SPOTIFAI_PROFILE";

/// Env var read by `spotifai api` to pick which provider to talk
/// to. Set by `spotifai ask`, `spotifai playlist`, and
/// `spotifai export`. Unset is tolerated — older shells written
/// against the Spotify-only spotifai still work — and is treated as
/// [`Provider::DEFAULT`].
pub const SPOTIFAI_PROVIDER_ENV: &str = "SPOTIFAI_PROVIDER";

/// Run the typed dispatcher against the active provider/profile.
pub fn forward(user_args: &[String]) -> Result<()> {
    let provider = active_provider()?;
    let profile = active_profile()?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    rt.block_on(dispatch(provider, profile, user_args))
}

/// Read [`SPOTIFAI_PROFILE_ENV`] and parse it into a [`Profile`].
pub fn active_profile() -> Result<Profile> {
    let raw = std::env::var(SPOTIFAI_PROFILE_ENV).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!(missing_profile_message());
    }
    Profile::parse(trimmed).ok_or_else(|| {
        anyhow!(
            "unknown {SPOTIFAI_PROFILE_ENV}=`{trimmed}`. {}",
            missing_profile_message(),
        )
    })
}

/// Read [`SPOTIFAI_PROVIDER_ENV`] and parse it into a [`Provider`].
/// Falls back to [`Provider::DEFAULT`] when unset/empty so existing
/// one-provider installs keep working.
pub fn active_provider() -> Result<Provider> {
    let raw = std::env::var(SPOTIFAI_PROVIDER_ENV).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Provider::DEFAULT);
    }
    Provider::parse(trimmed).ok_or_else(|| {
        anyhow!(
            "unknown {SPOTIFAI_PROVIDER_ENV}=`{trimmed}`; expected one of: {}",
            Provider::ALL
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
    })
}

fn missing_profile_message() -> String {
    "`spotifai api` must be invoked through `spotifai ask` or `spotifai playlist`; \
     no permission profile is selected."
        .to_string()
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

/// One typed dispatch a user has asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verb {
    /// `search "query" [--type T]* [--limit N]`
    Search {
        query: String,
        types: Vec<String>,
        limit: u32,
    },
    /// `playlists list [--limit N]`
    PlaylistsList { limit: u32 },
    /// `playlists show <id>`
    PlaylistsShow { id: String, limit: u32 },
    /// `playlists create --name|--title <name> [--description X] [--public|--private]`
    PlaylistsCreate {
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// `playlists add <playlist-id> <id1> [<id2>…]`
    PlaylistsAdd {
        playlist_id: String,
        ids: Vec<String>,
    },
    /// Spotify: `library tracks list [--limit N]`
    SpotifyLibraryTracksList { limit: u32 },
    /// Spotify: `library albums list [--limit N]`
    SpotifyLibraryAlbumsList { limit: u32 },
    /// YouTube Music: `library list [--limit N]` (rated videos).
    YmusicLibraryList { limit: u32 },
}

/// Default page size when `--limit` is omitted. Spotify and YouTube
/// Music both cap most list endpoints at 50.
pub const DEFAULT_LIMIT: u32 = 50;

/// Spotify's `/search` endpoint caps `limit` at 10 (tightened from
/// the historically-documented 50; values above 10 now return
/// `HTTP 400 "Invalid limit"`). Used as both the default and the
/// upper bound for `spotifai api search`.
pub const SEARCH_LIMIT: u32 = 10;

/// Parse the argv after `spotifai api` into a typed [`Verb`] for a
/// given provider. Errors with a human-readable message on bad
/// shapes; the agent's prompt is the source of truth for which
/// verbs each profile may use.
pub fn parse_verb(provider: Provider, args: &[String]) -> Result<Verb> {
    let mut iter = args.iter().peekable();
    let head = iter
        .next()
        .ok_or_else(|| anyhow!("missing verb after `spotifai api`"))?
        .as_str();
    match head {
        "search" => parse_search(&mut iter),
        "playlists" => parse_playlists(provider, &mut iter),
        "library" => parse_library(provider, &mut iter),
        other => bail!(
            "unknown spotifai api verb `{other}`. \
             Supported: search, playlists list/show/create/add, library …"
        ),
    }
}

fn parse_search<'a, I>(iter: &mut std::iter::Peekable<I>) -> Result<Verb>
where
    I: Iterator<Item = &'a String>,
{
    let mut query: Option<String> = None;
    let mut types: Vec<String> = Vec::new();
    let mut limit: u32 = SEARCH_LIMIT;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--type" | "-t" => {
                let v = iter.next().ok_or_else(|| anyhow!("--type needs a value"))?;
                types.push(v.clone());
            }
            s if s.starts_with("--type=") => types.push(s["--type=".len()..].to_string()),
            "--limit" | "-l" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--limit needs a value"))?;
                limit = parse_search_limit(v)?;
            }
            s if s.starts_with("--limit=") => limit = parse_search_limit(&s["--limit=".len()..])?,
            "--json" | "--pretty" => {
                // Output is always JSON; the flags are a no-op.
            }
            s if s.starts_with("--") => {
                bail!("unknown flag `{s}` for `search`");
            }
            other => {
                if query.is_some() {
                    bail!("`search` accepts only one query string; got an extra `{other}`");
                }
                query = Some(other.to_string());
            }
        }
    }
    let query = query.ok_or_else(|| anyhow!("`search` needs a query string"))?;
    if types.is_empty() {
        types.push("track".into());
    }
    Ok(Verb::Search {
        query,
        types,
        limit,
    })
}

fn parse_playlists<'a, I>(provider: Provider, iter: &mut std::iter::Peekable<I>) -> Result<Verb>
where
    I: Iterator<Item = &'a String>,
{
    let sub = iter
        .next()
        .ok_or_else(|| anyhow!("`playlists` needs a sub-verb (list, show, create, add)"))?
        .as_str();
    match sub {
        "list" => {
            let opts = parse_list_opts(iter)?;
            Ok(Verb::PlaylistsList { limit: opts.limit })
        }
        "show" => {
            let id = iter
                .next()
                .ok_or_else(|| anyhow!("`playlists show` needs a playlist id"))?
                .clone();
            let opts = parse_list_opts(iter)?;
            Ok(Verb::PlaylistsShow {
                id,
                limit: opts.limit,
            })
        }
        "create" => parse_playlists_create(provider, iter),
        "add" => {
            let playlist_id = iter
                .next()
                .ok_or_else(|| anyhow!("`playlists add` needs a playlist id"))?
                .clone();
            let mut ids: Vec<String> = Vec::new();
            for arg in iter {
                match arg.as_str() {
                    "--json" | "--pretty" => {}
                    s if s.starts_with("--") => {
                        bail!("unknown flag `{s}` for `playlists add`");
                    }
                    other => ids.push(other.to_string()),
                }
            }
            if ids.is_empty() {
                bail!("`playlists add <playlist-id> <id…>` needs at least one item id");
            }
            Ok(Verb::PlaylistsAdd { playlist_id, ids })
        }
        other => bail!("unknown `playlists` sub-verb `{other}`"),
    }
}

fn parse_playlists_create<'a, I>(
    provider: Provider,
    iter: &mut std::iter::Peekable<I>,
) -> Result<Verb>
where
    I: Iterator<Item = &'a String>,
{
    let name_flag = provider.playlist_name_flag(); // "--name" or "--title"
    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut public = false;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            s if s == name_flag => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("{name_flag} needs a value"))?;
                name = Some(v.clone());
            }
            s if s.starts_with(&format!("{name_flag}=")) => {
                name = Some(s[name_flag.len() + 1..].to_string());
            }
            "--description" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description needs a value"))?;
                description = Some(v.clone());
            }
            s if s.starts_with("--description=") => {
                description = Some(s["--description=".len()..].to_string());
            }
            "--public" => public = true,
            "--private" => public = false,
            "--json" | "--pretty" => {}
            other => bail!("unknown arg `{other}` for `playlists create`"),
        }
    }
    let name = name.ok_or_else(|| anyhow!("`playlists create` needs `{name_flag} <name>`"))?;
    Ok(Verb::PlaylistsCreate {
        name,
        description,
        public,
    })
}

fn parse_library<'a, I>(provider: Provider, iter: &mut std::iter::Peekable<I>) -> Result<Verb>
where
    I: Iterator<Item = &'a String>,
{
    match provider {
        Provider::Spotify => {
            let bucket = iter
                .next()
                .ok_or_else(|| anyhow!("`library` needs a bucket (tracks, albums)"))?
                .as_str();
            let next = iter
                .next()
                .ok_or_else(|| anyhow!("`library {bucket}` needs `list`"))?
                .as_str();
            if next != "list" {
                bail!("`library {bucket}` only supports `list`");
            }
            let opts = parse_list_opts(iter)?;
            match bucket {
                "tracks" => Ok(Verb::SpotifyLibraryTracksList { limit: opts.limit }),
                "albums" => Ok(Verb::SpotifyLibraryAlbumsList { limit: opts.limit }),
                other => {
                    bail!("unknown library bucket `{other}` for Spotify; expected tracks or albums")
                }
            }
        }
        Provider::YouTubeMusic => {
            let next = iter
                .next()
                .ok_or_else(|| anyhow!("`library` needs `list` for YouTube Music"))?
                .as_str();
            if next != "list" {
                bail!(
                    "YouTube Music's library surface is just `library list` (rated videos); \
                     got `library {next}`"
                );
            }
            let opts = parse_list_opts(iter)?;
            Ok(Verb::YmusicLibraryList { limit: opts.limit })
        }
    }
}

#[derive(Debug)]
struct ListOpts {
    limit: u32,
}

impl Default for ListOpts {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
        }
    }
}

fn parse_list_opts<'a, I>(iter: &mut std::iter::Peekable<I>) -> Result<ListOpts>
where
    I: Iterator<Item = &'a String>,
{
    let mut opts = ListOpts {
        limit: DEFAULT_LIMIT,
    };
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--limit" | "-l" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--limit needs a value"))?;
                opts.limit = parse_limit(v)?;
            }
            s if s.starts_with("--limit=") => {
                opts.limit = parse_limit(&s["--limit=".len()..])?;
            }
            "--json" | "--pretty" => {}
            other => bail!("unknown arg `{other}`"),
        }
    }
    Ok(opts)
}

fn parse_limit(s: &str) -> Result<u32> {
    let n: u32 = s
        .parse()
        .map_err(|_| anyhow!("--limit must be a positive integer; got `{s}`"))?;
    if !(1..=DEFAULT_LIMIT).contains(&n) {
        bail!("--limit must be between 1 and {DEFAULT_LIMIT}; got {n}");
    }
    Ok(n)
}

fn parse_search_limit(s: &str) -> Result<u32> {
    let n: u32 = s
        .parse()
        .map_err(|_| anyhow!("--limit must be a positive integer; got `{s}`"))?;
    if !(1..=SEARCH_LIMIT).contains(&n) {
        bail!("--limit must be between 1 and {SEARCH_LIMIT} for `search`; got {n}");
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

async fn dispatch(provider: Provider, _profile: Profile, user_args: &[String]) -> Result<()> {
    let verb = parse_verb(provider, user_args)?;
    let value = match provider {
        Provider::Spotify => dispatch_spotify(verb).await?,
        Provider::YouTubeMusic => dispatch_ymusic(verb).await?,
    };
    write_json(&value)
}

async fn dispatch_spotify(verb: Verb) -> Result<Value> {
    use zad::service::spotify::{
        CreatePlaylistRequest, PlaylistsRequest, SavedTracksRequest, SearchRequest,
    };
    match verb {
        Verb::Search {
            query,
            types,
            limit,
        } => {
            let client = zad_client::load_spotify_all()?;
            let req = SearchRequest::new(query, types, limit)
                .map_err(|e| anyhow!("invalid search request: {e}"))?;
            let res = client
                .search(req)
                .await
                .map_err(|e| anyhow!("spotify search failed: {e}"))?;
            Ok(spotify_search_to_value(&res))
        }
        Verb::PlaylistsList { limit } => {
            let client = zad_client::load_spotify_all()?;
            let req = PlaylistsRequest::new(limit)
                .map_err(|e| anyhow!("invalid playlists request: {e}"))?;
            let res = client
                .playlists(req)
                .await
                .map_err(|e| anyhow!("spotify playlists list failed: {e}"))?;
            Ok(json!({ "items": to_value(&res)? }))
        }
        Verb::PlaylistsShow { id, limit } => {
            let http = zad_client::load_spotify_http(default_spotify_scopes())?;
            let summary = http
                .get_playlist(&id)
                .await
                .map_err(|e| anyhow!("spotify get_playlist failed: {e}"))?;
            let tracks = http
                .get_playlist_tracks(&id, limit)
                .await
                .map_err(|e| anyhow!("spotify get_playlist_tracks failed: {e}"))?;
            let mut summary_value = to_value(&summary)?;
            if let Some(obj) = summary_value.as_object_mut() {
                obj.insert("tracks_items".into(), to_value(&tracks)?);
            }
            Ok(summary_value)
        }
        Verb::PlaylistsCreate {
            name,
            description,
            public,
        } => {
            let identity = zad_client::read_self_identity(Provider::Spotify)?;
            let user_id = identity.user_id.ok_or_else(|| {
                anyhow!(
                    "Spotify user id missing — re-run `spotifai auth --provider spotify` \
                     so the `/me` probe can capture it"
                )
            })?;
            let client = zad_client::load_spotify_all()?;
            let req = CreatePlaylistRequest::new(user_id, name, description, public)
                .map_err(|e| anyhow!("invalid create_playlist request: {e}"))?;
            let res = client
                .create_playlist(req)
                .await
                .map_err(|e| anyhow!("spotify create_playlist failed: {e}"))?;
            Ok(to_value(&res)?)
        }
        Verb::PlaylistsAdd { playlist_id, ids } => {
            let http = zad_client::load_spotify_http(default_spotify_scopes())?;
            // Spotify accepts a `spotify:track:<id>` URI or a bare
            // track id; the API expects URIs, so promote bare ids.
            let uris: Vec<String> = ids
                .into_iter()
                .map(|id| {
                    if id.starts_with("spotify:") {
                        id
                    } else {
                        format!("spotify:track:{id}")
                    }
                })
                .collect();
            http.add_playlist_tracks(&playlist_id, &uris)
                .await
                .map_err(|e| anyhow!("spotify add_playlist_tracks failed: {e}"))?;
            Ok(json!({
                "playlist_id": playlist_id,
                "added": uris.len(),
            }))
        }
        Verb::SpotifyLibraryTracksList { limit } => {
            let client = zad_client::load_spotify_all()?;
            let req = SavedTracksRequest::new(limit)
                .map_err(|e| anyhow!("invalid saved_tracks request: {e}"))?;
            let res = client
                .saved_tracks(req)
                .await
                .map_err(|e| anyhow!("spotify saved_tracks failed: {e}"))?;
            Ok(json!({ "items": to_value(&res)? }))
        }
        Verb::SpotifyLibraryAlbumsList { limit } => {
            let http = zad_client::load_spotify_http(default_spotify_scopes())?;
            let res = http
                .list_saved_albums(limit)
                .await
                .map_err(|e| anyhow!("spotify list_saved_albums failed: {e}"))?;
            Ok(json!({ "items": to_value(&res)? }))
        }
        Verb::YmusicLibraryList { .. } => bail!(
            "`library list` (rated videos) is a YouTube Music verb; \
             use `library tracks list` or `library albums list` on Spotify"
        ),
    }
}

async fn dispatch_ymusic(verb: Verb) -> Result<Value> {
    use zad::service::ymusic::{
        AddPlaylistItemRequest, CreatePlaylistRequest, LikedRequest, PlaylistsRequest,
        SearchRequest,
    };
    match verb {
        Verb::Search {
            query,
            types,
            limit,
        } => {
            let client = zad_client::load_ymusic_all()?;
            // YouTube Music's API takes `video`, `playlist`,
            // `channel`. Map the Spotify-style `track` we default to
            // when the agent omits `--type` to `video` so a
            // provider-agnostic `search "moon river"` works.
            let translated = translate_ymusic_types(&types);
            let req = SearchRequest::new(query, translated, limit)
                .map_err(|e| anyhow!("invalid search request: {e}"))?;
            let res = client
                .search(req)
                .await
                .map_err(|e| anyhow!("ymusic search failed: {e}"))?;
            Ok(json!({ "items": to_value(&res)? }))
        }
        Verb::PlaylistsList { limit } => {
            let client = zad_client::load_ymusic_all()?;
            let req = PlaylistsRequest::new(limit)
                .map_err(|e| anyhow!("invalid playlists request: {e}"))?;
            let res = client
                .playlists(req)
                .await
                .map_err(|e| anyhow!("ymusic playlists list failed: {e}"))?;
            Ok(json!({ "items": to_value(&res)? }))
        }
        Verb::PlaylistsShow { id, limit } => {
            let http = zad_client::load_ymusic_http(default_ymusic_scopes())?;
            let summary = http
                .get_playlist(&id)
                .await
                .map_err(|e| anyhow!("ymusic get_playlist failed: {e}"))?;
            let items = http
                .get_playlist_items(&id, limit)
                .await
                .map_err(|e| anyhow!("ymusic get_playlist_items failed: {e}"))?;
            let mut summary_value = to_value(&summary)?;
            if let Some(obj) = summary_value.as_object_mut() {
                obj.insert("items".into(), to_value(&items)?);
            }
            Ok(summary_value)
        }
        Verb::PlaylistsCreate {
            name,
            description,
            public,
        } => {
            let client = zad_client::load_ymusic_all()?;
            let privacy = if public {
                zad::service::ymusic::client::Privacy::Public
            } else {
                zad::service::ymusic::client::Privacy::Private
            };
            let req = CreatePlaylistRequest::new(name, description, privacy)
                .map_err(|e| anyhow!("invalid create_playlist request: {e}"))?;
            let res = client
                .create_playlist(req)
                .await
                .map_err(|e| anyhow!("ymusic create_playlist failed: {e}"))?;
            Ok(to_value(&res)?)
        }
        Verb::PlaylistsAdd { playlist_id, ids } => {
            let client = zad_client::load_ymusic_all()?;
            let mut added: Vec<String> = Vec::with_capacity(ids.len());
            for video_id in ids {
                let req = AddPlaylistItemRequest::new(playlist_id.clone(), video_id.clone())
                    .map_err(|e| anyhow!("invalid add_playlist_item request: {e}"))?;
                let item_id = client
                    .add_playlist_item(req)
                    .await
                    .map_err(|e| anyhow!("ymusic add_playlist_item failed: {e}"))?;
                added.push(item_id);
            }
            Ok(json!({
                "playlist_id": playlist_id,
                "items": added,
            }))
        }
        Verb::YmusicLibraryList { limit } => {
            let client = zad_client::load_ymusic_all()?;
            let req =
                LikedRequest::new(limit).map_err(|e| anyhow!("invalid liked request: {e}"))?;
            let res = client
                .liked(req)
                .await
                .map_err(|e| anyhow!("ymusic liked failed: {e}"))?;
            Ok(json!({ "items": to_value(&res)? }))
        }
        Verb::SpotifyLibraryTracksList { .. } | Verb::SpotifyLibraryAlbumsList { .. } => bail!(
            "`library tracks list` / `library albums list` are Spotify verbs; \
             on YouTube Music use `library list` for rated videos"
        ),
    }
}

/// Build a JSON envelope for Spotify's `SearchResults`. The zad
/// type only derives `Deserialize`, so we manually unpack each
/// section into JSON via the inner-summary types (which all derive
/// `Serialize`).
fn spotify_search_to_value(res: &zad::service::spotify::client::SearchResults) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(p) = res.tracks.as_ref() {
        out.insert(
            "tracks".into(),
            json!({ "items": to_value(&p.items).unwrap_or(Value::Null) }),
        );
    }
    if let Some(p) = res.albums.as_ref() {
        out.insert(
            "albums".into(),
            json!({ "items": to_value(&p.items).unwrap_or(Value::Null) }),
        );
    }
    if let Some(p) = res.artists.as_ref() {
        out.insert(
            "artists".into(),
            json!({ "items": to_value(&p.items).unwrap_or(Value::Null) }),
        );
    }
    if let Some(p) = res.playlists.as_ref() {
        out.insert(
            "playlists".into(),
            json!({ "items": to_value(&p.items).unwrap_or(Value::Null) }),
        );
    }
    Value::Object(out)
}

fn translate_ymusic_types(types: &[String]) -> Vec<String> {
    types
        .iter()
        .map(|t| match t.as_str() {
            "track" | "video" => "video".to_string(),
            "playlist" => "playlist".to_string(),
            "channel" | "artist" => "channel".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
}

fn default_spotify_scopes() -> std::collections::BTreeSet<String> {
    let mut s = std::collections::BTreeSet::new();
    s.insert("search".into());
    s.insert("playlists.read".into());
    s.insert("playlists.write".into());
    s.insert("library.read".into());
    s
}

fn default_ymusic_scopes() -> std::collections::BTreeSet<String> {
    let mut s = std::collections::BTreeSet::new();
    s.insert("search".into());
    s.insert("playlists.read".into());
    s.insert("playlists.write".into());
    s.insert("library.read".into());
    s
}

fn to_value<T: Serialize>(t: &T) -> Result<Value> {
    serde_json::to_value(t).context("serializing zad response to JSON")
}

fn write_json(value: &Value) -> Result<()> {
    let body = serde_json::to_string_pretty(value).context("serializing JSON output")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle
        .write_all(body.as_bytes())
        .context("writing JSON to stdout")?;
    handle
        .write_all(b"\n")
        .context("writing trailing newline")?;
    Ok(())
}
