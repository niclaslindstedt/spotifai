//! Central output module for spotifai (§19.4).
//!
//! All user-facing messages route through the semantic helpers below so
//! that formatting and routing can change in one place. Each helper
//! writes a styled line to stderr **and** emits a `tracing` event so
//! the same message lands in the always-on `debug.log` (see
//! [`crate::logging`]). Raw `println!` / `eprintln!` must not appear
//! outside this module except for machine-readable contract output
//! (e.g. JSON on stdout from `spotifai api`, the `--version` banner).

use std::io::Write as _;

use crate::logging;

/// Success message — green checkmark on stderr, `info` in the log.
pub fn status(msg: &str) {
    let _ = writeln!(std::io::stderr(), "✓  {msg}");
    tracing::info!(target: "spotifai::output", "{msg}");
}

/// Recoverable issue the user should know about (§19.1 `warn`).
pub fn warn(msg: &str) {
    let _ = writeln!(std::io::stderr(), "!  {msg}");
    tracing::warn!(target: "spotifai::output", "{msg}");
}

/// Normal operational message (§19.1 `info`).
pub fn info(msg: &str) {
    let _ = writeln!(std::io::stderr(), "{msg}");
    tracing::info!(target: "spotifai::output", "{msg}");
}

/// Bold section header — also `info` in the log.
pub fn header(msg: &str) {
    let _ = writeln!(std::io::stderr(), "== {msg} ==");
    tracing::info!(target: "spotifai::output", "{msg}");
}

/// Unrecoverable failure (§19.1 `error`).
pub fn error(msg: &str) {
    let _ = writeln!(std::io::stderr(), "✗  {msg}");
    tracing::error!(target: "spotifai::output", "{msg}");
}

/// Verbose diagnostic (§19.1 `debug`). Always written to `debug.log`;
/// echoed to stderr only when the user passed `--debug`.
pub fn debug(msg: &str) {
    if logging::debug_to_stderr() {
        let _ = writeln!(std::io::stderr(), "[debug] {msg}");
    }
    tracing::debug!(target: "spotifai::output", "{msg}");
}

/// Plain stdout line for machine-readable contract output (e.g. the
/// version banner). Goes to stdout with no styling and is mirrored to
/// the log at `info` level.
pub fn plain(msg: &str) {
    let _ = writeln!(std::io::stdout(), "{msg}");
    tracing::info!(target: "spotifai::output", "{msg}");
}
