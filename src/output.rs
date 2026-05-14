//! Central output module for spotifai (§19.4 / `docs/logging.md`).
//!
//! Every user-facing message routes through one of the semantic
//! helpers below so formatting, color, and stream choice live in one
//! place. Each helper writes a styled line to stderr **and** emits a
//! `tracing` event with structured `kind` and `scope` fields, so
//! `debug.log` captures everything the terminal sees plus the
//! `debug`-level events the terminal suppresses by default.
//!
//! Raw `println!` / `eprintln!` are confined to the contract surfaces
//! that need ANSI-free plain-text on stdout (see `help_agent`,
//! `commands_index`, `manpages`, `topic_docs`). Everywhere else, use
//! the helpers below — and re-read [`docs/logging.md`] before adding
//! a new one.

use std::cell::{Cell, RefCell};
use std::io::{IsTerminal as _, Write as _};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};

use crate::logging;

/// Semantic kind of one output event. Mirrors the helper that
/// produced the line and is attached to the file-log event as a
/// structured field so `grep 'kind="warn"' debug.log` works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Header,
    Section,
    Step,
    Action,
    Status,
    Info,
    Detail,
    Hint,
    Warn,
    Error,
    Debug,
    Plain,
}

impl Kind {
    /// Stable lowercase tag used in the `kind=` structured field.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Header => "header",
            Kind::Section => "section",
            Kind::Step => "step",
            Kind::Action => "action",
            Kind::Status => "status",
            Kind::Info => "info",
            Kind::Detail => "detail",
            Kind::Hint => "hint",
            Kind::Warn => "warn",
            Kind::Error => "error",
            Kind::Debug => "debug",
            Kind::Plain => "plain",
        }
    }
}

// ---------------------------------------------------------------------------
// Public helpers — see docs/logging.md for the decision tree.
// ---------------------------------------------------------------------------

/// Bold cyan header naming the operation a command is about to
/// perform. Printed once at the top of a command. Use [`section`]
/// instead when the lines that follow should be indented under it.
pub fn header(msg: &str) {
    write_event(Kind::Header, msg);
}

/// Combine [`header`] with a fresh [`scope`] so every helper invoked
/// during the returned guard's lifetime is indented one level deeper.
/// The `scope_label` is recorded on every nested tracing event as the
/// `scope=` structured field.
#[must_use = "the returned guard must be held for the duration of the section"]
pub fn section(msg: &str, scope_label: &'static str) -> ScopeGuard {
    write_event(Kind::Section, msg);
    scope(scope_label)
}

/// `[n/total] label` step header for multi-stage procedures (install,
/// migration). Pair with a [`scope`] to indent the body.
pub fn step(n: usize, total: usize, label: &str) {
    let msg = format!("[{n}/{total}] {label}");
    write_event(Kind::Step, &msg);
}

/// "Work is in progress" — phrase as a present-participle verb
/// ("fetching playlists"). Use when a long-running call follows so the
/// reader knows the program has not hung.
pub fn action(msg: &str) {
    write_event(Kind::Action, msg);
}

/// "Done" — a discrete user-visible win the user asked for. Past
/// tense ("exported 312 items").
pub fn status(msg: &str) {
    write_event(Kind::Status, msg);
}

/// Neutral context (paths, modes, totals). Resist the urge — most
/// info lines are really [`detail`] under an [`action`].
pub fn info(msg: &str) {
    write_event(Kind::Info, msg);
}

/// Sub-bullet under the previous [`action`] or [`status`]. Adds one
/// extra level of indent on top of the active scope.
pub fn detail(msg: &str) {
    write_event(Kind::Detail, msg);
}

/// Actionable suggestion the user can act on or ignore. Never used
/// for required follow-ups — those are [`status`] or [`warn`].
pub fn hint(msg: &str) {
    write_event(Kind::Hint, msg);
}

/// Recoverable issue: partial success now, or a future error the
/// user can prevent by acting now.
pub fn warn(msg: &str) {
    write_event(Kind::Warn, msg);
}

/// Unrecoverable for this command. The next thing the caller should
/// do is exit non-zero.
pub fn error(msg: &str) {
    write_event(Kind::Error, msg);
}

/// Verbose diagnostic written to `debug.log` unconditionally and
/// mirrored to stderr only when the user passed `--debug` (§19.3).
pub fn debug(msg: &str) {
    write_event(Kind::Debug, msg);
}

/// Machine-readable contract output: plain text on **stdout** with no
/// ANSI escapes (JSON, version banners, `--help-agent` payloads).
/// Mirrored to the file log at info-level for diagnosability.
pub fn plain(msg: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{msg}");
    emit_trace(Kind::Plain, msg);
}

/// Write a blank line to stderr to separate sections. Always flushed
/// to the file log too so the spacing survives in `debug.log`.
pub fn newline() {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err);
    emit_trace(Kind::Info, "");
}

// ---------------------------------------------------------------------------
// Input helpers — see docs/logging.md "Input"
// ---------------------------------------------------------------------------

/// Read one trimmed line from stdin after writing `label: ` on stderr
/// with the same color gate as the output helpers. Errors on EOF or
/// on an empty trimmed line.
pub fn prompt(label: &str) -> Result<String> {
    use std::io::{BufRead as _, Write as _};
    let style = active_style();
    {
        let mut err = std::io::stderr().lock();
        if style.colors {
            let _ = write!(err, "{}{label}{}: ", style::CYAN, style::RESET);
        } else {
            let _ = write!(err, "{label}: ");
        }
        err.flush().context("flushing prompt")?;
    }
    emit_trace(Kind::Info, &format!("prompt: {label}"));
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("reading prompt input")?;
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        bail!("empty value entered for `{label}`");
    }
    Ok(trimmed)
}

/// Read a y/n answer from stdin. Empty input returns `default`.
/// Accepts `y`/`yes`/`n`/`no` case-insensitively; rejects anything
/// else with an error so a typo cannot silently default to "no".
pub fn confirm(question: &str, default: bool) -> Result<bool> {
    use std::io::{BufRead as _, Write as _};
    let style = active_style();
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    {
        let mut err = std::io::stderr().lock();
        if style.colors {
            let _ = write!(err, "{}{question}{} {suffix}: ", style::CYAN, style::RESET);
        } else {
            let _ = write!(err, "{question} {suffix}: ");
        }
        err.flush().context("flushing confirm")?;
    }
    emit_trace(Kind::Info, &format!("confirm: {question}"));
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("reading confirm input")?;
    let trimmed = line.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Ok(default);
    }
    match trimmed.as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        other => bail!("expected y/yes/n/no for `{question}`; got `{other}`"),
    }
}

// ---------------------------------------------------------------------------
// Scope guards — see docs/logging.md "Indentation and scopes"
// ---------------------------------------------------------------------------

thread_local! {
    static SCOPE_DEPTH: Cell<usize> = const { Cell::new(0) };
    static SCOPE_STACK: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

/// RAII guard: the scope is active until this value is dropped, at
/// which point depth and label stack revert. Hold it in a `let _` to
/// keep it alive for the scope body.
#[must_use = "scope is released on drop; bind it to a local to keep it active"]
pub struct ScopeGuard {
    _priv: (),
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        SCOPE_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        SCOPE_STACK.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

/// Enter a new nested scope. Every helper invoked while the returned
/// guard is alive gets one extra level of indent and the joined scope
/// stack as its `scope=` structured field.
#[must_use = "the returned guard must be held for the duration of the scope"]
pub fn scope(label: &'static str) -> ScopeGuard {
    SCOPE_DEPTH.with(|d| d.set(d.get() + 1));
    SCOPE_STACK.with(|s| s.borrow_mut().push(label));
    ScopeGuard { _priv: () }
}

/// Active indent depth — exposed for tests; production code reaches
/// it through the helpers.
pub fn current_depth() -> usize {
    SCOPE_DEPTH.with(|d| d.get())
}

/// Dot-joined scope stack — exposed for tests and used internally as
/// the value of the `scope=` field on tracing events.
pub fn current_scope() -> String {
    SCOPE_STACK.with(|s| s.borrow().join("."))
}

// ---------------------------------------------------------------------------
// Style policy — see docs/logging.md "Colors" and "Glyphs"
// ---------------------------------------------------------------------------

/// Resolved style decisions for one process. Cached once on first
/// use; both env vars and TTY status are read only at that point so
/// repeated calls are cheap and stable.
#[derive(Debug, Clone, Copy)]
struct ActiveStyle {
    colors: bool,
    unicode: bool,
}

static ACTIVE_STYLE: OnceLock<ActiveStyle> = OnceLock::new();

fn active_style() -> ActiveStyle {
    *ACTIVE_STYLE.get_or_init(resolve_style)
}

fn resolve_style() -> ActiveStyle {
    ActiveStyle {
        colors: resolve_color_choice(),
        unicode: resolve_glyph_choice(),
    }
}

fn resolve_color_choice() -> bool {
    // Order matches the precedence documented in docs/logging.md
    // "Detection".
    match std::env::var("SPOTIFAI_COLOR")
        .ok()
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
    {
        Some(ref s) if s == "always" || s == "1" || s == "true" || s == "on" => return true,
        Some(ref s) if s == "never" || s == "0" || s == "false" || s == "off" => return false,
        _ => {}
    }
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return false;
    }
    std::io::stderr().is_terminal()
}

fn resolve_glyph_choice() -> bool {
    match std::env::var("SPOTIFAI_GLYPHS")
        .ok()
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
    {
        Some(ref s) if s == "ascii" => return false,
        Some(ref s) if s == "unicode" => return true,
        _ => {}
    }
    // Locale heuristic — `LC_ALL=C` or `LANG=C` users have asked for
    // pure ASCII output; everyone else gets unicode.
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    let upper = locale.to_ascii_uppercase();
    !(upper == "C" || upper == "POSIX")
}

#[allow(dead_code)]
mod style {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const ITALIC: &str = "\x1b[3m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const BOLD_RED: &str = "\x1b[1;31m";
    pub const BOLD_GREEN: &str = "\x1b[1;32m";
    pub const BOLD_YELLOW: &str = "\x1b[1;33m";
    pub const BOLD_CYAN: &str = "\x1b[1;36m";
}

// ---------------------------------------------------------------------------
// Test surfaces — exposed for `tests/output_test.rs` only.
// ---------------------------------------------------------------------------

/// Public read-only view of the resolved color decision. Used by
/// integration tests and by callers that want to skip an expensive
/// rendering step when the terminal would not show it.
pub fn color_enabled() -> bool {
    active_style().colors
}

/// Public read-only view of the resolved glyph decision.
pub fn unicode_enabled() -> bool {
    active_style().unicode
}

/// Render one helper line *without* emitting it. Used by tests to
/// assert the exact bytes a helper writes for a given (kind, scope
/// depth, style) tuple.
pub fn render_line(kind: Kind, msg: &str, depth: usize, colors: bool, unicode: bool) -> String {
    let style = ActiveStyle { colors, unicode };
    format_line(kind, msg, depth, style)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn write_event(kind: Kind, msg: &str) {
    let style = active_style();
    let depth = current_depth();
    let line = format_line(kind, msg, depth, style);

    // Debug events are gated on `--debug` for the terminal. Everything
    // else is unconditional. The file log gets every event regardless
    // — that's the §19.2 contract.
    let to_terminal = match kind {
        Kind::Debug => logging::debug_to_stderr(),
        _ => true,
    };
    if to_terminal {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{line}");
    }

    emit_trace(kind, msg);
}

fn format_line(kind: Kind, msg: &str, depth: usize, style: ActiveStyle) -> String {
    let indent = match kind {
        // detail is one level deeper than the active scope
        Kind::Detail => "  ".repeat(depth + 1),
        // header lines sit at column 0 regardless of scope — they
        // *open* a new section. Step lines do too.
        Kind::Header | Kind::Section | Kind::Step => "  ".repeat(depth.saturating_sub(0)),
        _ => "  ".repeat(depth),
    };
    let (glyph, color_on, color_off) = decorate(kind, style);
    // Separator between the glyph column and the message. Skipped
    // entirely when the kind has no glyph (Step, Plain) so an
    // indent-only render does not leak a stray leading space.
    let sep = if glyph.is_empty() { "" } else { " " };
    // For headers / sections the entire phrase is bold; for everything
    // else only the glyph carries the color so the message stays
    // legible against colored backgrounds.
    match kind {
        Kind::Header | Kind::Section | Kind::Step => {
            if style.colors {
                format!("{indent}{color_on}{glyph}{sep}{msg}{color_off}")
            } else {
                format!("{indent}{glyph}{sep}{msg}")
            }
        }
        _ => {
            if style.colors {
                format!("{indent}{color_on}{glyph}{color_off}{sep}{msg}")
            } else {
                format!("{indent}{glyph}{sep}{msg}")
            }
        }
    }
}

fn decorate(kind: Kind, style: ActiveStyle) -> (&'static str, &'static str, &'static str) {
    let glyph = match (kind, style.unicode) {
        (Kind::Header, true) => "══",
        (Kind::Header, false) => "==",
        (Kind::Section, true) => "▌",
        (Kind::Section, false) => ">>",
        (Kind::Step, _) => "",
        (Kind::Action, true) => "→",
        (Kind::Action, false) => "->",
        (Kind::Status, true) => "✓",
        (Kind::Status, false) => "[ok]",
        (Kind::Info, true) => "·",
        (Kind::Info, false) => "*",
        (Kind::Detail, true) => "·",
        (Kind::Detail, false) => "-",
        (Kind::Hint, true) => "i",
        (Kind::Hint, false) => "i",
        (Kind::Warn, true) => "⚠",
        (Kind::Warn, false) => "[!]",
        (Kind::Error, true) => "✗",
        (Kind::Error, false) => "[x]",
        (Kind::Debug, true) => "…",
        (Kind::Debug, false) => "[debug]",
        (Kind::Plain, _) => "",
    };
    let (on, off) = match (kind, style.colors) {
        (Kind::Header | Kind::Section | Kind::Step, true) => (style::BOLD_CYAN, style::RESET),
        (Kind::Action, true) => (style::CYAN, style::RESET),
        (Kind::Status, true) => (style::BOLD_GREEN, style::RESET),
        (Kind::Detail, true) => (style::DIM, style::RESET),
        (Kind::Hint, true) => (style::CYAN, style::RESET),
        (Kind::Warn, true) => (style::BOLD_YELLOW, style::RESET),
        (Kind::Error, true) => (style::BOLD_RED, style::RESET),
        (Kind::Debug, true) => (style::DIM, style::RESET),
        _ => ("", ""),
    };
    (glyph, on, off)
}

fn emit_trace(kind: Kind, msg: &str) {
    let scope = current_scope();
    match kind {
        Kind::Error => {
            tracing::error!(
                target: "spotifai::output",
                kind = kind.as_str(),
                scope = scope.as_str(),
                "{msg}"
            );
        }
        Kind::Warn => {
            tracing::warn!(
                target: "spotifai::output",
                kind = kind.as_str(),
                scope = scope.as_str(),
                "{msg}"
            );
        }
        Kind::Debug => {
            tracing::debug!(
                target: "spotifai::output",
                kind = kind.as_str(),
                scope = scope.as_str(),
                "{msg}"
            );
        }
        _ => {
            tracing::info!(
                target: "spotifai::output",
                kind = kind.as_str(),
                scope = scope.as_str(),
                "{msg}"
            );
        }
    }
}
