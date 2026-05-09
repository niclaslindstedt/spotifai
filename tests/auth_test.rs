//! Pure-function tests for `spotifai::auth` — no network, no keychain.
//!
//! `auth::run` itself runs an OAuth loopback flow that's not unit
//! testable in-process (it needs a browser callback). What we can
//! lock down here is the argument parser that drives it: which
//! flags are accepted, which combinations are rejected.

use spotifai::auth::parse_args;

#[test]
fn parse_args_defaults_to_open_browser_no_creds() {
    let opts = parse_args(&[]).expect("empty args parse");
    assert!(opts.client_id.is_none());
    assert!(opts.client_secret.is_none());
    assert!(opts.open_browser);
}

#[test]
fn parse_args_accepts_separated_and_equal_forms_for_client_id() {
    let separated = parse_args(&["--client-id".into(), "abc123".into()]).unwrap();
    assert_eq!(separated.client_id.as_deref(), Some("abc123"));
    let equal = parse_args(&["--client-id=abc123".into()]).unwrap();
    assert_eq!(equal.client_id.as_deref(), Some("abc123"));
}

#[test]
fn parse_args_accepts_separated_and_equal_forms_for_client_secret() {
    let separated = parse_args(&["--client-secret".into(), "shh".into()]).unwrap();
    assert_eq!(separated.client_secret.as_deref(), Some("shh"));
    let equal = parse_args(&["--client-secret=shh".into()]).unwrap();
    assert_eq!(equal.client_secret.as_deref(), Some("shh"));
}

#[test]
fn parse_args_no_browser_flips_open_browser_off() {
    let opts = parse_args(&["--no-browser".into()]).unwrap();
    assert!(!opts.open_browser);
}

#[test]
fn parse_args_rejects_unknown_flag() {
    let err = parse_args(&["--bogus".into()]).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("--bogus"),
        "error should mention the offending flag, got: {msg}"
    );
}

#[test]
fn parse_args_errors_when_value_missing_after_flag() {
    assert!(parse_args(&["--client-id".into()]).is_err());
    assert!(parse_args(&["--client-secret".into()]).is_err());
}
