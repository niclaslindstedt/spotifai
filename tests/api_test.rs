//! Pure-function tests for `spotifai::api` — no network, no zad spawn.

use std::path::PathBuf;

use spotifai::api::{
    ZAD_PERMISSIONS_PATH_ENV, build_command, command_env, forward_args, permissions_env_value,
};

#[test]
fn forward_args_prefixes_spotify_subcommand() {
    let user: Vec<String> = vec![];
    assert_eq!(forward_args(&user), vec!["spotify".to_string()]);
}

#[test]
fn forward_args_passes_user_args_through_verbatim() {
    let user = vec![
        "playlists".to_string(),
        "list".to_string(),
        "--json".to_string(),
    ];
    assert_eq!(
        forward_args(&user),
        vec![
            "spotify".to_string(),
            "playlists".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ]
    );
}

#[test]
fn forward_args_preserves_hyphen_values() {
    // Anything after `api` is forwarded as-is, including bare `--` and
    // flag values that look like options. zad does its own parsing.
    let user = vec![
        "tracks".to_string(),
        "--".to_string(),
        "--limit=10".to_string(),
    ];
    assert_eq!(
        forward_args(&user),
        vec![
            "spotify".to_string(),
            "tracks".to_string(),
            "--".to_string(),
            "--limit=10".to_string(),
        ]
    );
}

#[test]
fn build_command_targets_the_managed_zad_binary() {
    let zad = PathBuf::from("/home/user/.spotifai/bin/zad");
    let cmd = build_command(&zad, &["playlists".to_string(), "list".to_string()]);
    assert_eq!(cmd.get_program(), zad.as_os_str());
    let argv: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
    assert_eq!(
        argv,
        vec![
            std::ffi::OsStr::new("spotify"),
            std::ffi::OsStr::new("playlists"),
            std::ffi::OsStr::new("list"),
        ]
    );
}

#[test]
fn permissions_env_var_constant_matches_zad() {
    // zad ≥ 0.3.0 reads exactly this name; if upstream renames it,
    // this test will at least flag the assumption.
    assert_eq!(ZAD_PERMISSIONS_PATH_ENV, "ZAD_PERMISSIONS_PATH");
}

#[test]
fn permissions_env_value_points_at_spotifai_home() {
    let p = permissions_env_value().expect("resolving permissions path");
    let s = p.to_string_lossy();
    assert!(
        s.ends_with("/.spotifai/permissions.toml") || s.ends_with("\\.spotifai\\permissions.toml"),
        "unexpected permissions path: {s}"
    );
}

#[test]
fn command_env_helper_reads_back_explicitly_set_env() {
    let zad = PathBuf::from("/tmp/zad");
    let mut cmd = build_command(&zad, &[]);
    cmd.env(ZAD_PERMISSIONS_PATH_ENV, "/tmp/permissions.toml");
    let v = command_env(&cmd, ZAD_PERMISSIONS_PATH_ENV).expect("env was set above");
    assert_eq!(v, std::ffi::OsStr::new("/tmp/permissions.toml"));
}
