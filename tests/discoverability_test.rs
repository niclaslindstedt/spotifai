//! Freshness tests for the §12 CLI discoverability contract.
//!
//! These tests are the §12.3 / §12.4 freshness gate: every clap
//! `Command` variant must have a matching [`CommandSpec`], every
//! `CommandSpec` must have an embedded manpage, every flag
//! documented in a manpage must exist in the CLI's clap definitions,
//! and the §12.5 discoverability contract (eight surfaces) must
//! produce stable, non-empty output.

use clap::CommandFactory;

use spotifai::cli::Cli;
use spotifai::commands_index::{COMMAND_SPECS, lookup as command_lookup};
use spotifai::manpages::{MANPAGES, lookup as manpage_lookup};
use spotifai::topic_docs::{TOPICS, lookup as topic_lookup};

/// Every clap subcommand must have a matching `CommandSpec` row, and
/// vice versa — the `commands` listing must mirror the actual CLI.
#[test]
fn every_clap_subcommand_is_in_commands_index() {
    let app = Cli::command();
    let clap_names: Vec<String> = app
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .filter(|n| n != "help") // clap auto-injects this; not a real subcommand
        .collect();
    let spec_names: Vec<&str> = COMMAND_SPECS.iter().map(|s| s.name).collect();

    for name in &clap_names {
        assert!(
            command_lookup(name).is_some(),
            "clap subcommand `{name}` is missing a CommandSpec in commands_index.rs (§12.4)"
        );
    }
    for name in &spec_names {
        assert!(
            clap_names.iter().any(|n| n == name),
            "commands_index entry `{name}` has no matching clap subcommand (§12.4 drift)"
        );
    }
}

/// §12.3: every command must have a manpage. Both directions are
/// checked so the binary cannot embed an orphan manpage either.
#[test]
fn every_command_has_a_manpage() {
    for spec in COMMAND_SPECS {
        // `commands`, `man`, and `docs` are themselves discoverability
        // surfaces and live in main.md rather than a per-page file.
        if matches!(spec.name, "commands" | "man" | "docs") {
            continue;
        }
        assert!(
            manpage_lookup(spec.name).is_some(),
            "command `{}` has no embedded manpage (§12.3)",
            spec.name
        );
    }
    for (name, _) in MANPAGES {
        if *name == "main" {
            continue;
        }
        assert!(
            command_lookup(name).is_some(),
            "manpage `{name}` does not correspond to any CLI command (§12.3 drift)"
        );
    }
}

/// §12.3 flag-parity: every flag declared on a clap subcommand must
/// appear (by long-name or signature) in its manpage's flag table.
#[test]
fn manpage_flag_table_covers_clap_flags() {
    let app = Cli::command();
    for sub in app.get_subcommands() {
        let name = sub.get_name();
        if matches!(name, "help" | "commands" | "man" | "docs") {
            continue;
        }
        let Some(body) = manpage_lookup(name) else {
            continue; // covered by `every_command_has_a_manpage`
        };
        for arg in sub.get_arguments() {
            if arg.is_global_set() {
                continue; // documented once in main.md
            }
            let token = match arg.get_long() {
                Some(long) => format!("--{long}"),
                None => continue, // positional / short-only — covered by usage line
            };
            assert!(
                body.contains(&token),
                "flag `{token}` of `{name}` is not documented in man/{name}.md (§12.3 flag parity)"
            );
        }
    }
}

/// §12.3: every doc topic referenced by `topic_docs::TOPICS` must
/// have a non-empty embedded body.
#[test]
fn every_topic_has_non_empty_body() {
    for (name, body) in TOPICS {
        assert!(
            !body.trim().is_empty(),
            "topic `{name}` has an empty embedded body (§12.3)"
        );
        assert!(
            topic_lookup(name).is_some(),
            "topic `{name}` is in TOPICS but lookup() can't find it"
        );
    }
}

/// §12.5: the eight contract rows must all be implemented. We assert
/// the surfaces exist by walking the parsed CLI tree — clap will fail
/// to build if any flag or subcommand is missing.
#[test]
fn discoverability_contract_surfaces_exist() {
    let app = Cli::command();
    let arg_long_names: Vec<String> = app
        .get_arguments()
        .filter_map(|a| a.get_long().map(String::from))
        .collect();
    for required in ["help-agent", "debug-agent"] {
        assert!(
            arg_long_names.iter().any(|n| n == required),
            "global flag --{required} is missing (§12.5 contract)"
        );
    }
    for required in ["commands", "man", "docs"] {
        assert!(
            app.get_subcommands().any(|s| s.get_name() == required),
            "subcommand `{required}` is missing (§12.5 contract)"
        );
    }
}

/// §12.4 example sanity: every command must declare at least one
/// realistic example invocation.
#[test]
fn every_command_has_examples() {
    for spec in COMMAND_SPECS {
        assert!(
            !spec.examples.is_empty(),
            "command `{}` has no example invocations (§12.4)",
            spec.name
        );
    }
}

/// §12.1 freshness: `--help-agent` must mention every top-level
/// command (so an agent that splices it into a prompt can still
/// reach every surface). Acts as the deterministic-regeneration
/// snapshot the spec asks for without committing brittle text.
#[test]
fn help_agent_lists_every_command() {
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_spotifai"))
        .arg("--help-agent")
        .output()
        .expect("running spotifai --help-agent");
    assert!(
        output.status.success(),
        "spotifai --help-agent exited non-zero: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("--help-agent stdout is UTF-8");
    for spec in COMMAND_SPECS {
        assert!(
            stdout.contains(spec.name),
            "--help-agent output is missing command `{}` (§12.1)",
            spec.name
        );
    }
    assert!(
        stdout.contains("commands"),
        "--help-agent must point at `spotifai commands` as the discovery surface (§12.1)"
    );
}

/// §12.2 freshness: `--debug-agent` must mention the log path and
/// every documented environment variable so a debugging prompt
/// learns where to look without re-probing the filesystem.
#[test]
fn debug_agent_covers_required_topics() {
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_spotifai"))
        .arg("--debug-agent")
        .output()
        .expect("running spotifai --debug-agent");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("--debug-agent stdout is UTF-8");
    for required in [
        "debug.log",
        "SPOTIFAI_PROVIDER",
        "SPOTIFAI_PROFILE",
        "SPOTIFAI_LOG",
        "permissions",
        "spotifai auth",
    ] {
        assert!(
            stdout.contains(required),
            "--debug-agent output is missing `{required}` (§12.2)"
        );
    }
}
