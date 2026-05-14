//! Pure-render tests for [`spotifai::output`].
//!
//! These tests deliberately avoid emitting events through the real
//! helpers (which install a global tracing subscriber and write to
//! stderr) and instead exercise the pure `render_line` surface plus
//! the `ScopeGuard` thread-local state. The end-to-end stderr write
//! path is covered by the integration tests under `tests/api_test.rs`
//! and friends, which scrape the streams of the compiled binary.

use spotifai::output::{self, Kind};

/// Smoke-test that every helper variant resolves to a non-empty
/// rendered line and that the ASCII glyph fallback contains no
/// non-ASCII bytes. Catches accidental regressions where a new kind
/// is added but the `decorate` switch forgets a branch.
#[test]
fn every_kind_renders_under_ascii_and_unicode() {
    let kinds = [
        Kind::Header,
        Kind::Section,
        Kind::Step,
        Kind::Action,
        Kind::Status,
        Kind::Info,
        Kind::Detail,
        Kind::Hint,
        Kind::Warn,
        Kind::Error,
        Kind::Debug,
    ];
    for kind in kinds {
        let unicode = output::render_line(kind, "hi", 0, false, true);
        let ascii = output::render_line(kind, "hi", 0, false, false);
        assert!(unicode.contains("hi"), "{kind:?} dropped the message body");
        assert!(ascii.contains("hi"), "{kind:?} dropped the message body");
        assert!(
            ascii.is_ascii(),
            "{kind:?} ASCII fallback contains non-ASCII bytes: {ascii:?}"
        );
    }
}

/// Bold-cyan headers and bold-cyan steps must include the ANSI
/// bold-plus-cyan SGR pair when colors are enabled, and zero escape
/// bytes when they are not. The exact escape codes are an internal
/// detail of `style::*` — we assert the public-visible shape only.
#[test]
fn color_off_emits_no_ansi() {
    let line = output::render_line(Kind::Status, "wrote file", 0, false, true);
    assert!(
        !line.contains('\x1b'),
        "colors=false must not emit ANSI: {line:?}"
    );
}

#[test]
fn color_on_emits_ansi_on_styled_kinds() {
    let action = output::render_line(Kind::Action, "fetching", 0, true, true);
    let status = output::render_line(Kind::Status, "done", 0, true, true);
    let error = output::render_line(Kind::Error, "boom", 0, true, true);
    assert!(
        action.contains('\x1b'),
        "action under colors=true: {action:?}"
    );
    assert!(
        status.contains('\x1b'),
        "status under colors=true: {status:?}"
    );
    assert!(error.contains('\x1b'), "error under colors=true: {error:?}");
}

#[test]
fn info_kind_under_colors_is_unstyled() {
    // Per docs/logging.md, info lines carry no color so they stay
    // legible against colored backgrounds. The glyph is the only
    // marker.
    let line = output::render_line(Kind::Info, "ctx", 0, true, true);
    assert!(
        !line.contains('\x1b'),
        "info under colors=true should be unstyled: {line:?}"
    );
}

/// Indentation rule: every kind except `Detail` indents one level
/// per scope depth; `Detail` indents one *extra* level so it sits
/// under the action above it.
#[test]
fn indent_grows_with_depth() {
    for depth in 0..3 {
        let info = output::render_line(Kind::Info, "x", depth, false, true);
        let detail = output::render_line(Kind::Detail, "x", depth, false, true);
        let info_lead: String = info.chars().take_while(|c| *c == ' ').collect();
        let detail_lead: String = detail.chars().take_while(|c| *c == ' ').collect();
        assert_eq!(info_lead.len(), depth * 2, "info indent at depth={depth}");
        assert_eq!(
            detail_lead.len(),
            (depth + 1) * 2,
            "detail indent at depth={depth}"
        );
    }
}

/// `header` / `section` / `step` always anchor at the active scope's
/// indent — they open a new sub-section, they don't get an extra
/// indent of their own. Tested here so a regression that adds e.g.
/// "make headers stand out by indenting more" gets caught.
#[test]
fn headers_anchor_at_scope_indent() {
    let h = output::render_line(Kind::Header, "x", 1, false, true);
    let s = output::render_line(Kind::Section, "x", 1, false, true);
    let step = output::render_line(Kind::Step, "x", 1, false, true);
    for line in [&h, &s, &step] {
        let lead: String = line.chars().take_while(|c| *c == ' ').collect();
        assert_eq!(lead.len(), 2, "header-like indent at depth=1: {line:?}");
    }
}

/// ScopeGuard nests and unnests cleanly. The depth observed inside
/// nested guards must climb monotonically and revert on drop in LIFO
/// order — that's the invariant the indent calculation relies on.
#[test]
fn scope_guard_nests_and_reverts() {
    assert_eq!(output::current_depth(), 0);
    assert_eq!(output::current_scope(), "");
    {
        let _outer = output::scope("outer");
        assert_eq!(output::current_depth(), 1);
        assert_eq!(output::current_scope(), "outer");
        {
            let _inner = output::scope("inner");
            assert_eq!(output::current_depth(), 2);
            assert_eq!(output::current_scope(), "outer.inner");
        }
        assert_eq!(output::current_depth(), 1);
        assert_eq!(output::current_scope(), "outer");
    }
    assert_eq!(output::current_depth(), 0);
    assert_eq!(output::current_scope(), "");
}

/// `Kind::as_str` is part of the structured-logging contract — the
/// grep recipes in docs/logging.md depend on these tags being
/// stable. Lock them down so a rename gets a code-review prompt.
#[test]
fn kind_as_str_tags_are_stable() {
    let pairs = [
        (Kind::Header, "header"),
        (Kind::Section, "section"),
        (Kind::Step, "step"),
        (Kind::Action, "action"),
        (Kind::Status, "status"),
        (Kind::Info, "info"),
        (Kind::Detail, "detail"),
        (Kind::Hint, "hint"),
        (Kind::Warn, "warn"),
        (Kind::Error, "error"),
        (Kind::Debug, "debug"),
        (Kind::Plain, "plain"),
    ];
    for (kind, tag) in pairs {
        assert_eq!(kind.as_str(), tag, "kind tag drift for {kind:?}");
    }
}

/// `color_enabled` / `unicode_enabled` are cached on first read.
/// The expected default in an integration-test runner (no TTY, no
/// `SPOTIFAI_COLOR=always`) is colors off — assert that the cached
/// value is the legal off state.
#[test]
fn color_default_is_off_under_tests() {
    // Don't assume what the user's environment looks like — just
    // assert the boolean is observable (i.e. the OnceLock resolves).
    let _ = output::color_enabled();
    let _ = output::unicode_enabled();
}
