//! On-disk state for resumable `spotifai import` runs.
//!
//! When an import is interrupted — most often by a daily-quota
//! rate-limit on YouTube Music, where the next slot is hours away —
//! the user wants to re-run the same command later and pick up where
//! the previous run left off. We persist enough state to do that:
//!
//! - Which playlists already finished (or were skipped as duplicates,
//!   or failed unrecoverably at the create step).
//! - For an in-progress playlist: the target playlist id, the
//!   resolved track ids in order, and how many of those have already
//!   been added.
//!
//! State files live under `~/.spotifai/import-state/` and are keyed
//! by `(provider, envelope-fingerprint)`. The fingerprint is a
//! deterministic FNV-1a hash of the envelope's `source.service`,
//! `exported_at`, and the ordered list of playlist names. Re-running
//! the same envelope against the same target reuses the same state
//! file; re-running with a different envelope (or a different target
//! provider) starts fresh.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::export_schema::Envelope;
use crate::providers::Provider;

/// Per-playlist progress record. Tracks just enough to make a
/// resumed run idempotent: we don't re-resolve already-resolved
/// tracks and we don't re-add already-added ones.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistState {
    pub status: PlaylistStatus,
    /// Provider-specific id of the playlist created on the target
    /// (Spotify playlist id or YouTube playlist id). Set after a
    /// successful `create_playlist`. None for `SkippedDuplicate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    /// Resolved track / video ids in source order, with unresolved
    /// tracks skipped. Captured incrementally as each track is
    /// processed so a resumed run does not re-issue search calls
    /// (which both burn quota and may pick a different first hit on
    /// a different day). Length is always `tracks_added +
    /// tracks_failed` once `tracks_processed == playlist.tracks.len()`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_track_ids: Vec<String>,
    /// Cursor into the source playlist's track list. The next
    /// resumed run skips this many source tracks before resuming the
    /// resolve+insert loop. Distinct from `tracks_added` because not
    /// every processed source track resolves, and not every resolved
    /// track inserts successfully.
    #[serde(default)]
    pub tracks_processed: usize,
    /// Number of source tracks the resolver could not find on the
    /// target. Surfaced in the final summary; not retried on resume.
    #[serde(default)]
    pub unresolved_count: usize,
    /// Number of items from `resolved_track_ids` that have been
    /// successfully added to the target playlist.
    #[serde(default)]
    pub tracks_added: usize,
    /// Number of items from `resolved_track_ids` whose add call
    /// failed for non-rate-limit reasons. Counted in the summary;
    /// not retried.
    #[serde(default)]
    pub tracks_failed: usize,
}

/// Lifecycle of one playlist in a (possibly multi-run) import.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistStatus {
    /// A playlist with this name already existed on the target on
    /// the first run; never attempted.
    SkippedDuplicate,
    /// Resolution and/or addition is partially done. A resumed run
    /// picks up here.
    InProgress,
    /// Every resolved track was added (or counted as failed). No
    /// further work needed on resume.
    Completed,
    /// `create_playlist` itself failed; not retried on resume because
    /// the failure is usually a permission or scope problem, not
    /// transient. The user can clear state with `--no-resume` to try
    /// again from scratch.
    FailedCreate,
}

impl PlaylistState {
    pub fn new_skipped_duplicate() -> Self {
        Self {
            status: PlaylistStatus::SkippedDuplicate,
            target_id: None,
            resolved_track_ids: Vec::new(),
            tracks_processed: 0,
            unresolved_count: 0,
            tracks_added: 0,
            tracks_failed: 0,
        }
    }

    pub fn new_failed_create() -> Self {
        Self {
            status: PlaylistStatus::FailedCreate,
            target_id: None,
            resolved_track_ids: Vec::new(),
            tracks_processed: 0,
            unresolved_count: 0,
            tracks_added: 0,
            tracks_failed: 0,
        }
    }

    /// True when nothing more should be attempted for this playlist
    /// on a resumed run.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            PlaylistStatus::SkippedDuplicate
                | PlaylistStatus::Completed
                | PlaylistStatus::FailedCreate
        )
    }
}

/// Top-level state document persisted between runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportState {
    /// Stable hash of the source envelope so we don't accidentally
    /// resume against a different document.
    pub envelope_fingerprint: String,
    /// Target provider slug (`spotify` / `ymusic`).
    pub provider: String,
    /// RFC 3339 timestamp captured the first time the state file was
    /// written.
    pub started_at: String,
    /// RFC 3339 timestamp updated on every flush.
    pub last_updated_at: String,
    /// Per-playlist progress, keyed by the (trimmed) playlist name.
    /// A `BTreeMap` keeps the on-disk JSON stable and diff-friendly.
    #[serde(default)]
    pub playlists: BTreeMap<String, PlaylistState>,
}

impl ImportState {
    /// Build a fresh state for a never-seen-before envelope.
    pub fn new(envelope_fingerprint: String, provider: Provider) -> Self {
        let now = rfc3339_now();
        Self {
            envelope_fingerprint,
            provider: provider.as_str().to_string(),
            started_at: now.clone(),
            last_updated_at: now,
            playlists: BTreeMap::new(),
        }
    }

    /// Insert or replace one playlist's state and bump
    /// `last_updated_at`.
    pub fn upsert(&mut self, name: &str, state: PlaylistState) {
        self.playlists.insert(name.trim().to_string(), state);
        self.last_updated_at = rfc3339_now();
    }

    /// Read-only lookup keyed by trimmed playlist name.
    pub fn get(&self, name: &str) -> Option<&PlaylistState> {
        self.playlists.get(name.trim())
    }

    /// Counts derived from the playlist map. Handy for the resume
    /// banner ("3/88 playlists already completed").
    pub fn counts(&self) -> StateCounts {
        let mut c = StateCounts::default();
        for s in self.playlists.values() {
            match s.status {
                PlaylistStatus::Completed => c.completed += 1,
                PlaylistStatus::SkippedDuplicate => c.skipped_duplicate += 1,
                PlaylistStatus::InProgress => c.in_progress += 1,
                PlaylistStatus::FailedCreate => c.failed_create += 1,
            }
            c.tracks_added += s.tracks_added;
            c.tracks_unresolved += s.unresolved_count;
            c.tracks_failed += s.tracks_failed;
        }
        c
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StateCounts {
    pub completed: usize,
    pub skipped_duplicate: usize,
    pub in_progress: usize,
    pub failed_create: usize,
    pub tracks_added: usize,
    pub tracks_unresolved: usize,
    pub tracks_failed: usize,
}

/// Resolve `~/.spotifai/import-state/`, creating it on demand.
pub fn state_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    let dir = home.join(".spotifai").join("import-state");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating import-state directory {}", dir.display()))?;
    Ok(dir)
}

/// State file path for a given (provider, fingerprint) pair.
pub fn state_path(provider: Provider, fingerprint: &str) -> Result<PathBuf> {
    Ok(state_dir()?.join(format!("{}-{}.json", provider.as_str(), fingerprint)))
}

/// Load an existing state file; returns `Ok(None)` when no file
/// exists. A corrupt or schema-mismatched file is reported as an
/// error so the user can decide whether to delete it (`--no-resume`
/// also works as a workaround).
pub fn load(path: &std::path::Path) -> Result<Option<ImportState>> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str::<ImportState>(&s)
            .with_context(|| format!("parsing import-state file {}", path.display()))
            .map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

/// Persist state atomically: write to a sibling temp file and
/// rename. Crashes mid-write therefore leave the previous good copy
/// intact rather than producing a half-written JSON document.
pub fn save(state: &ImportState, path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(state).context("serializing import-state JSON")?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} to {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Best-effort delete (the file may not exist; that is not an
/// error). Used after a clean import finishes so the user's
/// `~/.spotifai/import-state/` doesn't accumulate stale runs.
pub fn clear(path: &std::path::Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::Error::new(e).context(format!("deleting {}", path.display()))),
    }
}

/// Deterministic, stable, dependency-free fingerprint of the parts
/// of the envelope that identify "this same import." We include
/// `source.service`, `exported_at`, the target provider, and the
/// ordered list of playlist names + track counts. This is *not* a
/// cryptographic hash; we just need a stable filename-friendly key.
pub fn fingerprint(envelope: &Envelope, provider: Provider) -> String {
    let mut hasher = Fnv1a64::new();
    hasher.update(envelope.source.service.as_bytes());
    hasher.update(b"|");
    hasher.update(envelope.exported_at.as_bytes());
    hasher.update(b"|");
    hasher.update(provider.as_str().as_bytes());
    for p in &envelope.playlists {
        hasher.update(b"|");
        hasher.update(p.name.trim().as_bytes());
        hasher.update(b":");
        hasher.update(p.tracks.len().to_string().as_bytes());
    }
    format!("{:016x}", hasher.finish())
}

/// Tiny FNV-1a 64-bit hasher. Stable across Rust versions and
/// platforms, unlike `std::collections::hash_map::DefaultHasher`.
struct Fnv1a64 {
    hash: u64,
}

impl Fnv1a64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self { hash: Self::OFFSET }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.hash ^= u64::from(b);
            self.hash = self.hash.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(self) -> u64 {
        self.hash
    }
}

fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, min, sec) = unix_to_civil(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Convert a UNIX epoch seconds value into a civil (Y, M, D, h, m, s)
/// tuple in UTC. Howard Hinnant's algorithm; no chrono dependency.
fn unix_to_civil(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = (secs % 86_400) as u32;
    let hour = rem / 3_600;
    let min = (rem % 3_600) / 60;
    let sec = rem % 60;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };

    (year, m, d, hour, min, sec)
}
