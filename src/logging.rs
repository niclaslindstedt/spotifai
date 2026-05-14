//! Always-on debug-level file logging (§19.2) and `--debug` stderr
//! toggle (§19.3).
//!
//! `init` is called from [`crate::cli::run`] with the value of the
//! global `--debug` flag. Every event emitted by [`crate::output`] is
//! routed here through `tracing`, so the file log captures the same
//! user-facing messages the terminal sees plus any `debug` events
//! that the terminal suppresses by default. See
//! [`docs/logging.md`](../../docs/logging.md) for the full level /
//! glyph / color / scope contract.
//!
//! The log file lives at the platform's state directory (Linux:
//! `~/.local/state/spotifai/debug.log`, macOS: `~/Library/Application
//! Support/spotifai/debug.log`, Windows: `%APPDATA%\spotifai\debug.log`).
//! No rotation is performed — `truncate the file manually or set up
//! logrotate` is the documented v1 stance per §19.2.
//!
//! Each event arrives with two structured fields attached by the
//! [`crate::output`] helpers:
//!
//! - `kind` — the helper that produced the event (`"action"`,
//!   `"status"`, `"warn"`, …). Used by `grep 'kind="warn"'`-style
//!   log post-processing.
//! - `scope` — the joined active [`crate::output::scope`] stack at
//!   emit time (e.g. `"export.playlists"`). Empty for top-level
//!   events.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Holds the non-blocking-writer worker guard so it lives for the
/// entire process — dropping the guard would flush and close the
/// file handle prematurely.
static GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Set by [`init`] when the user passes `--debug`. Read by
/// [`crate::output::debug`] to decide whether to also print to stderr.
static DEBUG_TO_STDERR: AtomicBool = AtomicBool::new(false);

/// Cached log file path so [`path`] is cheap and stable across calls.
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Resolve the directory that holds `debug.log` per §19.2.
fn log_dir() -> Result<PathBuf> {
    if let Some(state) = dirs::state_dir() {
        return Ok(state.join("spotifai"));
    }
    if let Some(data) = dirs::data_dir() {
        return Ok(data.join("spotifai"));
    }
    Err(anyhow!(
        "could not determine a state/data directory for spotifai's debug log"
    ))
}

/// Best-effort log path. Returns `None` if neither
/// [`dirs::state_dir`] nor [`dirs::data_dir`] is available — used by
/// `--debug-agent`-style help text and the troubleshooting docs.
pub fn path() -> Option<PathBuf> {
    LOG_PATH
        .get()
        .cloned()
        .or_else(|| log_dir().ok().map(|d| d.join("debug.log")))
}

/// Whether the active session should also echo `debug` events to
/// stderr (set by `--debug`). The file log always captures `debug`,
/// regardless of this flag.
pub fn debug_to_stderr() -> bool {
    DEBUG_TO_STDERR.load(Ordering::Relaxed)
}

/// Install the global tracing subscriber that writes every event at
/// `debug` and above to `debug.log` and remembers the `--debug` flag
/// for the rest of the process. Idempotent — repeated calls return
/// `Ok(())` without re-installing.
pub fn init(debug: bool) -> Result<()> {
    DEBUG_TO_STDERR.store(debug, Ordering::Relaxed);
    if GUARD.get().is_some() {
        return Ok(());
    }

    let dir = log_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating log directory {}", dir.display()))?;
    let file_path = dir.join("debug.log");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .with_context(|| format!("opening {}", file_path.display()))?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    let _ = GUARD.set(guard);
    let _ = LOG_PATH.set(file_path);

    let filter =
        EnvFilter::try_from_env("SPOTIFAI_LOG").unwrap_or_else(|_| EnvFilter::new("debug"));
    let layer = fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .with_level(true);
    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init()
        .map_err(|e| anyhow!("failed to install tracing subscriber: {e}"))?;
    Ok(())
}
