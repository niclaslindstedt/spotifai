//! Output projection for `spotifai api search`.
//!
//! The search dispatcher returns a JSON envelope shaped like
//! `{tracks: {items: [...]}, albums: {items: [...]}}` on Spotify and
//! `{items: [...]}` on YouTube Music. Agents that consume the result
//! pay for every token of context, so this module narrows each item
//! to the fields the caller asked for (`--fields title,artist,album`)
//! and optionally renders the result as line-based text instead of
//! pretty JSON (`--format text`).

use std::io::Write;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

/// Output shape for `spotifai api search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Pretty JSON, the existing default.
    #[default]
    Json,
    /// One item per line, fields tab-separated in the order requested
    /// via `--fields`.
    Text,
}

/// Parse a `--format <value>` argument.
pub fn parse_format(s: &str) -> Result<OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "json" => Ok(OutputFormat::Json),
        "text" | "txt" => Ok(OutputFormat::Text),
        other => bail!("--format must be `json` or `text`; got `{other}`"),
    }
}

/// Split a `--fields a,b,c` value (or a repeated `--fields` flag) into
/// individual field names. Empty entries are skipped.
pub fn append_fields(raw: &str, out: &mut Vec<String>) {
    for f in raw.split(',') {
        let f = f.trim();
        if !f.is_empty() {
            out.push(f.to_string());
        }
    }
}

/// Apply a field filter to a search-result envelope in place.
///
/// Spotify envelopes hold one or more `{ tracks|albums|artists|playlists:
/// { items: [...] } }` sections; YouTube Music envelopes hold a
/// top-level `items` array. We walk both shapes and replace each item
/// with a `{field: value}` projection where `field` is the name the
/// caller used (so `--fields title` returns `{"title": ...}` even
/// though the underlying key is `name`).
pub fn project_envelope(value: &mut Value, fields: &[String]) {
    if fields.is_empty() {
        return;
    }
    let Value::Object(map) = value else {
        return;
    };
    project_items_in(map, fields);
    for v in map.values_mut() {
        if let Value::Object(section) = v {
            project_items_in(section, fields);
        }
    }
}

fn project_items_in(map: &mut Map<String, Value>, fields: &[String]) {
    let Some(Value::Array(items)) = map.get_mut("items") else {
        return;
    };
    for item in items.iter_mut() {
        *item = project_item(item, fields);
    }
}

fn project_item(item: &Value, fields: &[String]) -> Value {
    let mut out = Map::new();
    for field in fields {
        if let Some(v) = lookup_field(item, field) {
            out.insert(field.clone(), v);
        }
    }
    Value::Object(out)
}

/// Resolve a user-supplied field name against a single search-result
/// item. Aliases collapse provider-specific shapes (Spotify's
/// `artists: [{name}]` vs YouTube Music's `snippet.channelTitle`) into
/// a single string the caller can consume.
///
/// For Spotify's `playlists show` response each item is wrapped in
/// `{item: <track>, added_at: ...}` (legacy `{track: <track>, ...}`);
/// the inner track is unwrapped transparently so the caller asks for
/// `title,artist,album,id` regardless of which endpoint the envelope
/// came from.
fn lookup_field(item: &Value, field: &str) -> Option<Value> {
    if let Some(inner) = item
        .get("item")
        .or_else(|| item.get("track"))
        .filter(|v| v.is_object())
        && let Some(v) = lookup_field_direct(inner, field)
    {
        return Some(v);
    }
    lookup_field_direct(item, field)
}

fn lookup_field_direct(item: &Value, field: &str) -> Option<Value> {
    match field.to_ascii_lowercase().as_str() {
        "title" | "name" => item
            .get("name")
            .cloned()
            .or_else(|| item.get("snippet").and_then(|s| s.get("title")).cloned()),
        "artist" | "artists" => artist_field(item),
        "album" => album_field(item),
        "duration" | "duration_ms" => item.get("duration_ms").cloned(),
        "release_date" => item.get("release_date").cloned().or_else(|| {
            item.get("album")
                .and_then(|a| a.get("release_date"))
                .cloned()
        }),
        "id" => id_field(item),
        "uri" => item.get("uri").cloned(),
        other => item.get(other).cloned(),
    }
}

fn artist_field(item: &Value) -> Option<Value> {
    if let Some(arr) = item.get("artists").and_then(|v| v.as_array()) {
        let names: Vec<String> = arr
            .iter()
            .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(str::to_string))
            .collect();
        if names.is_empty() {
            return None;
        }
        return Some(Value::String(names.join(", ")));
    }
    for key in ["videoOwnerChannelTitle", "channelTitle", "channel_title"] {
        if let Some(s) = item
            .get("snippet")
            .and_then(|s| s.get(key))
            .and_then(|v| v.as_str())
        {
            return Some(Value::String(s.to_string()));
        }
    }
    None
}

fn album_field(item: &Value) -> Option<Value> {
    let album = item.get("album")?;
    if let Some(name) = album.get("name").and_then(|v| v.as_str()) {
        return Some(Value::String(name.to_string()));
    }
    Some(album.clone())
}

fn id_field(item: &Value) -> Option<Value> {
    // YMusic playlist-items put the playlistItem record id at the top
    // level; the underlying video id lives at `contentDetails.videoId`
    // (or `snippet.resourceId.videoId` on older shapes). Callers
    // asking for `id` almost always want the video, not the
    // playlist-item record, so prefer those.
    if let Some(s) = item
        .get("contentDetails")
        .and_then(|c| c.get("videoId"))
        .and_then(|v| v.as_str())
    {
        return Some(Value::String(s.to_string()));
    }
    if let Some(s) = item
        .get("snippet")
        .and_then(|s| s.get("resourceId"))
        .and_then(|r| r.get("videoId"))
        .and_then(|v| v.as_str())
    {
        return Some(Value::String(s.to_string()));
    }
    match item.get("id")? {
        Value::String(s) => Some(Value::String(s.clone())),
        Value::Object(map) => {
            for key in ["videoId", "playlistId", "channelId"] {
                if let Some(Value::String(s)) = map.get(key) {
                    return Some(Value::String(s.clone()));
                }
            }
            None
        }
        other => Some(other.clone()),
    }
}

/// Serialize an envelope as pretty JSON to stdout, with a trailing
/// newline.
pub fn write_json(value: &Value) -> Result<()> {
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

/// Render the envelope as one item per line, tab-separated, in the
/// order of `fields`. The caller is responsible for having already
/// projected each item via [`project_envelope`].
pub fn write_text(value: &Value, fields: &[String]) -> Result<()> {
    if fields.is_empty() {
        bail!("--format text requires --fields to choose which columns to print");
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    visit_items(value, &mut |item| -> Result<()> {
        let row: Vec<String> = fields
            .iter()
            .map(|f| match item.get(f) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(v) => v.to_string(),
            })
            .collect();
        handle
            .write_all(row.join("\t").as_bytes())
            .context("writing text row")?;
        handle.write_all(b"\n").context("writing newline")?;
        Ok(())
    })
}

fn visit_items(value: &Value, visit: &mut dyn FnMut(&Value) -> Result<()>) -> Result<()> {
    let Value::Object(map) = value else {
        return Ok(());
    };
    if let Some(Value::Array(items)) = map.get("items") {
        for item in items {
            visit(item)?;
        }
    }
    for v in map.values() {
        if let Value::Object(section) = v {
            if let Some(Value::Array(items)) = section.get("items") {
                for item in items {
                    visit(item)?;
                }
            }
        }
    }
    Ok(())
}
