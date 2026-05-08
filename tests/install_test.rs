//! Pure-function tests for `spotifai::install` — no network, no filesystem.

use std::path::PathBuf;

use spotifai::install::{
    PINNED_VERSION_RAW, ZAD_PERMISSIONS_PATH_ENV, asset_name_for, asset_url,
    build_permissions_sign_command, build_signing_init_command, command_env, parse_fingerprint,
    parse_version, pinned_version, version_matches,
};

#[test]
fn pinned_version_matches_zadrc_trimmed() {
    let raw = PINNED_VERSION_RAW;
    assert_eq!(pinned_version(), raw.trim());
    assert!(
        pinned_version().starts_with('v'),
        ".zadrc should pin a `vX.Y.Z` git tag, got `{}`",
        pinned_version()
    );
}

#[test]
fn parse_version_picks_trailing_token() {
    assert_eq!(parse_version("zad 0.2.0\n").as_deref(), Some("0.2.0"));
    assert_eq!(parse_version("zad v0.2.0").as_deref(), Some("v0.2.0"));
    assert_eq!(parse_version("0.2.0\nignored\n").as_deref(), Some("0.2.0"));
    assert_eq!(parse_version(""), None);
    assert_eq!(parse_version("\n"), None);
}

#[test]
fn version_matches_strips_v_prefix_either_side() {
    assert!(version_matches("0.2.0", "v0.2.0"));
    assert!(version_matches("v0.2.0", "0.2.0"));
    assert!(version_matches("v0.2.0", "v0.2.0"));
    assert!(!version_matches("0.2.0", "v0.3.0"));
    assert!(!version_matches("0.2.0", "v0.2.1"));
}

#[test]
fn asset_name_covers_supported_targets() {
    assert_eq!(
        asset_name_for("linux", "x86_64").unwrap(),
        "zad-x86_64-unknown-linux-gnu"
    );
    assert_eq!(
        asset_name_for("linux", "aarch64").unwrap(),
        "zad-aarch64-unknown-linux-gnu"
    );
    assert_eq!(
        asset_name_for("macos", "x86_64").unwrap(),
        "zad-x86_64-apple-darwin"
    );
    assert_eq!(
        asset_name_for("macos", "aarch64").unwrap(),
        "zad-aarch64-apple-darwin"
    );
    assert_eq!(
        asset_name_for("windows", "x86_64").unwrap(),
        "zad-x86_64-pc-windows-msvc.exe"
    );
}

#[test]
fn asset_name_rejects_unsupported_targets() {
    assert!(asset_name_for("freebsd", "x86_64").is_err());
    assert!(asset_name_for("linux", "riscv64").is_err());
}

#[test]
fn parse_fingerprint_extracts_value_from_signing_init_json() {
    let json = r#"{
  "command": "signing.init",
  "fingerprint": "8ce74dee",
  "rotated": false
}"#;
    assert_eq!(parse_fingerprint(json).as_deref(), Some("8ce74dee"));
}

#[test]
fn parse_fingerprint_returns_none_when_absent_or_malformed() {
    assert!(parse_fingerprint("{}").is_none());
    assert!(parse_fingerprint("not even json").is_none());
    assert!(parse_fingerprint(r#"{"fingerprint": }"#).is_none());
}

#[test]
fn signing_init_command_is_zad_signing_init_with_json_flag() {
    let zad = PathBuf::from("/tmp/zad");
    let cmd = build_signing_init_command(&zad);
    let argv: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
    assert_eq!(argv, vec!["signing", "init", "--json"]);
    // Program path is the zad binary we passed in.
    assert_eq!(cmd.get_program(), zad.as_os_str());
}

#[test]
fn permissions_sign_command_uses_local_and_pins_env_var() {
    let zad = PathBuf::from("/tmp/zad");
    let policy = PathBuf::from("/tmp/permissions.toml");
    let cmd = build_permissions_sign_command(&zad, &policy);

    let argv: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
    assert_eq!(
        argv,
        vec!["spotify", "permissions", "sign", "--local"],
        "the install flow signs the local file pinned via {ZAD_PERMISSIONS_PATH_ENV}"
    );

    let env = command_env(&cmd, ZAD_PERMISSIONS_PATH_ENV)
        .expect("ZAD_PERMISSIONS_PATH must be set so zad signs the spotifai-managed file");
    assert_eq!(env, policy.as_os_str());
}

#[test]
fn asset_url_uses_releases_download_path() {
    // Avoid coupling to the host arch by going through asset_name_for.
    let asset = asset_name_for("linux", "x86_64").unwrap();
    let url = asset_url("v0.2.0").unwrap();
    // We can't assert full equality (depends on host), but it must point
    // at the niclaslindstedt/zad release-download path with the tag we
    // passed in.
    assert!(
        url.starts_with("https://github.com/niclaslindstedt/zad/releases/download/v0.2.0/zad-"),
        "url = {url}"
    );
    // Sanity: the asset name we computed for linux-x86_64 matches the
    // shape spotifai uses too.
    assert!(asset.starts_with("zad-"));
}
