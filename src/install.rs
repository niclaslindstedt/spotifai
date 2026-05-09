//! Bootstrap helpers for the per-machine Ed25519 signing key in
//! the OS keychain and the spotifai-managed permissions files at
//! `~/.spotifai/permissions/<provider>/<profile>.toml`.
//!
//! Two functions:
//!
//! - [`bootstrap_signing_key`] mints (or loads) the signing key,
//!   materializes the trust store, and writes the public-key cache.
//! - [`sign_permissions_file`] signs a spotifai permission TOML and
//!   upserts the resulting signature into the per-machine trust
//!   store. Subsequent zad library calls that load the file pass
//!   the load-time trust check.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::providers::Provider;

/// Bootstrap the per-machine Ed25519 signing key in the OS keychain.
/// Idempotent — `load_or_create_from_keychain` is a no-op when a key
/// already exists, so this is safe to call on every `spotifai
/// install` invocation.
///
/// On success the keypair lives in the OS keychain (account
/// `signing:v1`) and a self-signed empty trust store is written to
/// `~/.zad/signing/trusted.toml`. Returns the short fingerprint of
/// the resulting key for the install banner.
pub fn bootstrap_signing_key() -> Result<Option<String>> {
    let key = zad::permissions::signing::load_or_create_from_keychain()
        .map_err(|e| anyhow!("bootstrapping zad signing key failed: {e}"))?;
    // Materialize the trust store: load the existing one (or default
    // empty), then re-save under the bootstrapped key. This way a
    // fresh install lands an empty `~/.zad/signing/trusted.toml`
    // signed by the new keychain key, and a subsequent
    // `sign_permissions_file` call has somewhere to upsert into.
    let store = zad::permissions::trust::TrustStore::load()
        .map_err(|e| anyhow!("loading zad trust store failed: {e}"))?;
    store
        .save(&key)
        .map_err(|e| anyhow!("saving zad trust store failed: {e}"))?;
    // Update the public-key cache so `~/.zad/signing/public_key.toml`
    // matches the live keychain key — `zad signing init` did this as
    // a side effect, and external tooling may rely on it.
    zad::permissions::signing::write_public_key_cache(&key)
        .map_err(|e| anyhow!("writing public-key cache failed: {e}"))?;
    Ok(Some(key.fingerprint()))
}

/// Sign the spotifai-managed permissions file at `policy_path` with
/// the keychain signing key, and upsert the resulting signature into
/// the per-machine trust store at `~/.zad/signing/trusted.toml`.
/// Subsequent zad library calls that load the file pass the
/// load-time trust check.
///
/// Requires [`bootstrap_signing_key`] to have populated the keychain
/// — the call fails with `SigningKeyMissing` otherwise. The
/// `_provider` argument is retained on the signature so call sites
/// can read the provider for diagnostic strings; the signed payload
/// is the canonical TOML serialization of the spotifai
/// [`crate::permissions::Permissions`] struct, so per-provider
/// routing is no longer needed.
pub fn sign_permissions_file(_provider: Provider, policy_path: &Path) -> Result<()> {
    use crate::permissions;

    let raw = std::fs::read_to_string(policy_path)
        .with_context(|| format!("reading {}", policy_path.display()))?;
    let parsed = permissions::from_toml_string(&raw).with_context(|| {
        format!(
            "parsing spotifai permission profile at {}",
            policy_path.display()
        )
    })?;

    let key = zad::permissions::signing::load_or_create_from_keychain()
        .map_err(|e| anyhow!("loading zad signing key failed: {e}"))?;
    let signature = zad::permissions::signing::sign_unsigned(&parsed, &key)
        .map_err(|e| anyhow!("signing permission file failed: {e}"))?;

    let canonical = zad::permissions::trust::canonical_path_key(policy_path)
        .map_err(|e| anyhow!("canonicalizing trust-store path failed: {e}"))?;
    let entry = zad::permissions::trust::TrustEntry {
        path: canonical,
        algorithm: signature.algorithm,
        public_key: signature.public_key,
        signed_at: signature.signed_at,
        value: signature.value,
    };
    let mut store = zad::permissions::trust::TrustStore::load()
        .map_err(|e| anyhow!("loading zad trust store failed: {e}"))?;
    store.upsert(entry);
    store
        .save(&key)
        .map_err(|e| anyhow!("saving zad trust store failed: {e}"))?;
    Ok(())
}
