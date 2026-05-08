//! Pure-function tests for `spotifai::api` — no network, no zad spawn.

use std::path::PathBuf;

use spotifai::api::{build_command, forward_args};

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
