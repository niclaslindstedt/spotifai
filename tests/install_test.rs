//! End-to-end tests for `spotifai::install` against the
//! `ZAD_SECRETS_MEMORY` + `ZAD_HOME_OVERRIDE` test backends so the
//! signing flow exercises real `zad::permissions` code without
//! touching the OS keychain.
//!
//! These tests share process-wide env vars (`ZAD_HOME_OVERRIDE`) so
//! they must run serialized; the [`SERIAL`] mutex below enforces
//! that.

use std::sync::{Mutex, MutexGuard, OnceLock};

use tempfile::TempDir;

use spotifai::install::{bootstrap_signing_key, sign_permissions_file};
use spotifai::permissions::{self, Profile};
use spotifai::providers::Provider;

fn serial_lock() -> MutexGuard<'static, ()> {
    static SERIAL: OnceLock<Mutex<()>> = OnceLock::new();
    SERIAL
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// Configure the per-test environment so zad's signing layer
/// stores keys/trust data inside `tmp` instead of the host
/// keychain or `~/.zad/`. Also wipes the shared in-memory keychain
/// so each test starts from a clean slate even when the parent
/// process has already touched the static `secrets::memory_store`.
fn isolate(tmp: &TempDir) {
    // SAFETY: the SERIAL guard above ensures we are the only test
    // touching env vars right now.
    unsafe {
        std::env::set_var("ZAD_SECRETS_MEMORY", "1");
        std::env::set_var("ZAD_HOME_OVERRIDE", tmp.path());
    }
    let _ = zad::secrets::delete(zad::permissions::signing::SIGNING_ACCOUNT);
}

#[test]
fn bootstrap_signing_key_returns_a_fingerprint() {
    let _g = serial_lock();
    let tmp = TempDir::new().unwrap();
    isolate(&tmp);

    let fp = bootstrap_signing_key().expect("bootstrap should succeed under the in-memory backend");
    let fp = fp.expect("a fingerprint is produced");
    assert!(
        !fp.is_empty(),
        "fingerprint should be non-empty (8-char hex slice), got `{fp}`"
    );
    assert!(
        fp.chars().all(|c| c.is_ascii_hexdigit()),
        "fingerprint should be hex, got `{fp}`"
    );
}

#[test]
fn bootstrap_signing_key_is_idempotent() {
    let _g = serial_lock();
    let tmp = TempDir::new().unwrap();
    isolate(&tmp);

    let first = bootstrap_signing_key().unwrap().unwrap();
    let second = bootstrap_signing_key().unwrap().unwrap();
    assert_eq!(
        first, second,
        "subsequent runs should return the existing key, not a fresh one"
    );
}

#[test]
fn sign_permissions_file_writes_a_trust_store_entry() {
    let _g = serial_lock();
    let tmp = TempDir::new().unwrap();
    isolate(&tmp);
    bootstrap_signing_key().unwrap();

    // Scaffold a default permission file under the tempdir so
    // canonical_path_key works against a real path.
    let policy_dir = tmp.path().join(".spotifai/permissions/spotify");
    std::fs::create_dir_all(&policy_dir).unwrap();
    let policy_path = policy_dir.join("ask.toml");
    let policy = Profile::Ask.default_policy(Provider::Spotify);
    let body = permissions::to_toml_string(&policy).unwrap();
    std::fs::write(&policy_path, body).unwrap();

    sign_permissions_file(Provider::Spotify, &policy_path)
        .expect("signing should succeed after bootstrap");

    // The signed trust store sits under <home>/.zad/signing/trusted.toml
    // when ZAD_HOME_OVERRIDE points at the tempdir (ZAD_HOME_OVERRIDE
    // is treated as the home dir, not as ~/.zad itself).
    let trust_path = tmp.path().join(".zad/signing/trusted.toml");
    assert!(
        trust_path.exists(),
        "trust store should be written at {}",
        trust_path.display()
    );
    let trust_body = std::fs::read_to_string(&trust_path).unwrap();
    assert!(
        trust_body.contains("[[entry]]"),
        "trust store should carry at least one signed entry: {trust_body}"
    );
    assert!(
        trust_body.contains("[signature]"),
        "trust store should be self-signed: {trust_body}"
    );
}

#[test]
fn sign_permissions_file_is_idempotent() {
    let _g = serial_lock();
    let tmp = TempDir::new().unwrap();
    isolate(&tmp);
    bootstrap_signing_key().unwrap();

    let policy_dir = tmp.path().join(".spotifai/permissions/ymusic");
    std::fs::create_dir_all(&policy_dir).unwrap();
    let policy_path = policy_dir.join("playlist.toml");
    let policy = Profile::Playlist.default_policy(Provider::YouTubeMusic);
    let body = permissions::to_toml_string(&policy).unwrap();
    std::fs::write(&policy_path, body).unwrap();

    sign_permissions_file(Provider::YouTubeMusic, &policy_path).unwrap();
    sign_permissions_file(Provider::YouTubeMusic, &policy_path)
        .expect("re-signing the same file should overwrite the trust entry, not error");
}
