//! Smoke tests for [`spotifai::logging`].
//!
//! `init` installs a global tracing subscriber, so it can only be
//! called once per test binary. These tests assert the cheap, reentrant
//! pieces (path resolution, the `--debug` toggle) and leave the heavy
//! subscriber wiring to the integration smoke in `cli` callers.

use spotifai::logging;

#[test]
fn debug_to_stderr_default_is_false() {
    // `init` may or may not have been called by another test in this
    // binary; either way the only legal pre-flag value is `false`,
    // because the default is `false` and no test in this file sets it.
    assert!(!logging::debug_to_stderr());
}

#[test]
fn path_resolves_to_a_filename() {
    let path = logging::path().expect("expected dirs::state_dir or dirs::data_dir to resolve");
    assert!(
        path.file_name().is_some_and(|n| n == "debug.log"),
        "expected log path to end in debug.log, got {}",
        path.display()
    );
    let parent = path.parent().expect("log path must have a parent dir");
    assert!(
        parent.ends_with("spotifai"),
        "expected log dir to live under .../spotifai/, got {}",
        parent.display()
    );
}
