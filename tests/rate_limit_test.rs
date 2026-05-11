//! Tests for spotifai's rate-limit-coordination helpers
//! (`zad_client::wait_mode_with_default`, `precall_check`, and the
//! `SPOTIFAI_WAIT` env-var wiring).
//!
//! The disk-touching paths (the on-`ZAD_HOME_OVERRIDE` deadline
//! file) are covered by zad's own `rate_limit_test.rs`. What this
//! file exercises is the spotifai-side resolution from CLI flag +
//! env var + caller-supplied default into the single boolean every
//! call site consults.
//!
//! `std::env::set_var` is `unsafe` in the 2024 edition because POSIX
//! forbids env mutation while another thread is reading. `cargo
//! test` runs test functions concurrently, so the tests below
//! serialise on a single `Mutex` and clear the relevant variables
//! around each case.

use std::sync::Mutex;

use spotifai::zad_client::{self, SPOTIFAI_WAIT_ENV, wait_mode, wait_mode_with_default};

/// Global serialisation guard. Every test that pokes
/// `SPOTIFAI_WAIT` must hold this lock for the duration of its
/// `env::set_var` / `env::remove_var` calls.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_env<F: FnOnce()>(value: Option<&str>, f: F) {
    let _guard = ENV_LOCK.lock().unwrap();
    // SAFETY: serialised by ENV_LOCK; the rest of the test binary's
    // env-touching code goes through this helper.
    unsafe {
        match value {
            Some(v) => std::env::set_var(SPOTIFAI_WAIT_ENV, v),
            None => std::env::remove_var(SPOTIFAI_WAIT_ENV),
        }
    }
    f();
    unsafe {
        std::env::remove_var(SPOTIFAI_WAIT_ENV);
    }
}

#[test]
fn cli_wait_true_overrides_everything() {
    with_env(Some("0"), || {
        assert!(wait_mode_with_default(Some(true), false));
        assert!(wait_mode_with_default(Some(true), true));
    });
}

#[test]
fn cli_wait_false_overrides_everything() {
    with_env(Some("1"), || {
        assert!(!wait_mode_with_default(Some(false), false));
        assert!(!wait_mode_with_default(Some(false), true));
    });
}

#[test]
fn env_var_truthy_strings_wait() {
    for v in &["1", "true", "TRUE", "yes", "On"] {
        with_env(Some(v), || {
            assert!(
                wait_mode_with_default(None, false),
                "env value `{v}` should resolve to wait=true"
            );
        });
    }
}

#[test]
fn env_var_falsy_strings_fail_fast() {
    for v in &["0", "false", "no", "off", ""] {
        with_env(Some(v), || {
            assert!(
                !wait_mode_with_default(None, true),
                "env value `{v}` should resolve to wait=false"
            );
        });
    }
}

#[test]
fn env_var_garbage_falls_through_to_default() {
    with_env(Some("maybe"), || {
        assert!(wait_mode_with_default(None, true));
        assert!(!wait_mode_with_default(None, false));
    });
}

#[test]
fn missing_env_var_falls_through_to_default() {
    with_env(None, || {
        assert!(wait_mode_with_default(None, true));
        assert!(!wait_mode_with_default(None, false));
    });
}

#[test]
fn legacy_wait_mode_helper_defaults_to_fail_fast() {
    with_env(None, || {
        assert!(!wait_mode(None));
        assert!(wait_mode(Some(true)));
        assert!(!wait_mode(Some(false)));
    });
}

#[test]
fn rate_limit_service_slug_is_provider_specific() {
    assert_eq!(
        zad_client::rate_limit_service(spotifai::providers::Provider::Spotify),
        "spotify"
    );
    assert_eq!(
        zad_client::rate_limit_service(spotifai::providers::Provider::YouTubeMusic),
        "ymusic"
    );
}

#[test]
fn precall_check_without_recorded_deadline_is_noop() {
    // No deadline file → precall_check is a no-op regardless of
    // wait mode. Point ZAD_HOME_OVERRIDE at a fresh tempdir so we
    // don't accidentally consult the real one.
    let tmp = tempfile::tempdir().unwrap();
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("ZAD_HOME_OVERRIDE", tmp.path());
    }
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        zad_client::precall_check(spotifai::providers::Provider::Spotify, true)
            .await
            .expect("no recorded deadline → wait=true is a no-op");
        zad_client::precall_check(spotifai::providers::Provider::Spotify, false)
            .await
            .expect("no recorded deadline → wait=false is a no-op");
    });
    unsafe {
        std::env::remove_var("ZAD_HOME_OVERRIDE");
    }
}
