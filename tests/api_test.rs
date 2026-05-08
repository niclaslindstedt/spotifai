//! Pure-function tests for `spotifai::api` — no network, no zad spawn.

use std::path::PathBuf;

use spotifai::api::{
    SPOTIFAI_PROFILE_ENV, ZAD_PERMISSIONS_PATH_ENV, build_command, command_env, forward_args,
    resolve_permissions_path,
};
use spotifai::permissions::Profile;

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
fn profile_env_constant_is_spotifai_namespaced() {
    // The selector lives under spotifai's namespace because
    // `spotifai api` resolves the file before forwarding to zad —
    // zad never sees this variable.
    assert_eq!(SPOTIFAI_PROFILE_ENV, "SPOTIFAI_PROFILE");
}

#[test]
fn resolve_permissions_path_errors_when_file_missing() {
    // Neither profile file should exist under HOME during a fresh
    // test run; assert that resolve_permissions_path fails closed.
    // (If a developer happens to have spotifai installed on their
    // machine, both files will exist and the call will succeed —
    // which is also a valid outcome to assert against.)
    for &profile in Profile::ALL {
        match resolve_permissions_path(profile) {
            Ok(path) => {
                let s = path.to_string_lossy();
                assert!(
                    s.contains(&format!(".spotifai/permissions/{}.toml", profile.as_str()))
                        || s.contains(&format!(
                            ".spotifai\\permissions\\{}.toml",
                            profile.as_str()
                        )),
                    "resolved path should sit under .spotifai/permissions/, got {s}"
                );
            }
            Err(e) => {
                let msg = format!("{e:#}");
                assert!(
                    msg.contains("spotifai install"),
                    "missing-file error should point at `spotifai install`, got: {msg}"
                );
            }
        }
    }
}

#[test]
fn command_env_helper_reads_back_explicitly_set_env() {
    let zad = PathBuf::from("/tmp/zad");
    let mut cmd = build_command(&zad, &[]);
    cmd.env(ZAD_PERMISSIONS_PATH_ENV, "/tmp/permissions.toml");
    let v = command_env(&cmd, ZAD_PERMISSIONS_PATH_ENV).expect("env was set above");
    assert_eq!(v, std::ffi::OsStr::new("/tmp/permissions.toml"));
}
