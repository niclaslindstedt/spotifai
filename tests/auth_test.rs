//! Pure-function tests for `spotifai::auth` — no network, no zad spawn.

use std::path::PathBuf;

use spotifai::auth::{build_command, forward_args};

#[test]
fn forward_args_targets_zad_service_create_spotify() {
    let user: Vec<String> = vec![];
    assert_eq!(
        forward_args(&user),
        vec![
            "service".to_string(),
            "create".to_string(),
            "spotify".to_string(),
        ]
    );
}

#[test]
fn forward_args_does_not_inject_local_flag() {
    // Spotify hands out one developer app per user, so credentials
    // are stored globally. `auth` must not silently scope them to
    // a project — that defeats the point.
    let user: Vec<String> = vec![];
    let out = forward_args(&user);
    assert!(
        !out.iter().any(|s| s == "--local"),
        "auth must not inject --local: {out:?}"
    );
}

#[test]
fn forward_args_passes_user_args_through_verbatim() {
    let user = vec![
        "--client-id".to_string(),
        "abc123".to_string(),
        "--no-browser".to_string(),
    ];
    assert_eq!(
        forward_args(&user),
        vec![
            "service".to_string(),
            "create".to_string(),
            "spotify".to_string(),
            "--client-id".to_string(),
            "abc123".to_string(),
            "--no-browser".to_string(),
        ]
    );
}

#[test]
fn forward_args_lets_user_opt_into_local_explicitly() {
    // The default is global, but if a user really wants to scope the
    // credentials to the current project they can still pass --local
    // through. The shim does not strip flags.
    let user = vec!["--local".to_string()];
    let out = forward_args(&user);
    assert_eq!(
        out,
        vec![
            "service".to_string(),
            "create".to_string(),
            "spotify".to_string(),
            "--local".to_string(),
        ]
    );
}

#[test]
fn build_command_targets_the_managed_zad_binary() {
    let zad = PathBuf::from("/home/user/.spotifai/bin/zad");
    let cmd = build_command(&zad, &[]);
    assert_eq!(cmd.get_program(), zad.as_os_str());
    let argv: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
    assert_eq!(
        argv,
        vec![
            std::ffi::OsStr::new("service"),
            std::ffi::OsStr::new("create"),
            std::ffi::OsStr::new("spotify"),
        ]
    );
}
