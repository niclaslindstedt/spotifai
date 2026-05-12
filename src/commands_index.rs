//! `commands` subcommand and the single source of truth for the
//! discoverability surfaces (§12.4).
//!
//! Each [`CommandSpec`] declares one top-level command's name, usage
//! signature, summary, full flag table, exit codes, and a handful of
//! realistic example invocations. The same data structure powers
//! `commands`, `commands <name>`, `commands --examples`, the
//! `--help-agent` overview, and the freshness tests that cross-check
//! the manpages — keeping every surface in sync without hand-edited
//! parallel tables.

use std::io::Write;

use anyhow::{Result, bail};

/// One row of the §12.4 command index, with enough detail to render
/// every discoverability surface without reaching back into clap.
pub struct CommandSpec {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub flags: &'static [FlagSpec],
    pub exit_codes: &'static [(&'static str, &'static str)],
    pub examples: &'static [&'static str],
}

/// One row of a command's flag/argument table, mirroring the
/// `man/<cmd>.md` layout.
pub struct FlagSpec {
    pub name: &'static str,
    pub ty: &'static str,
    pub default: &'static str,
    pub description: &'static str,
}

const COMMON_EXIT_CODES: &[(&str, &str)] = &[
    ("0", "Success."),
    ("1", "Generic error."),
    ("2", "Usage error."),
];

/// Every top-level spotifai command in the order they should appear.
pub static COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        name: "install",
        usage: "spotifai install",
        summary: "Bootstrap the Ed25519 signing key, scaffold every <provider>/<profile>.toml, and sign each one. Idempotent.",
        flags: &[],
        exit_codes: COMMON_EXIT_CODES,
        examples: &["spotifai install"],
    },
    CommandSpec {
        name: "auth",
        usage: "spotifai auth [--provider <slug>] [--client-id <id>] [--client-secret <secret>] [--no-browser]",
        summary: "Run the in-process OAuth loopback flow for a provider and persist the tokens in the OS keychain.",
        flags: &[
            FlagSpec {
                name: "--provider <slug>",
                ty: "enum",
                default: "spotify",
                description: "Provider to register credentials for (spotify, ymusic).",
            },
            FlagSpec {
                name: "--client-id <id>",
                ty: "string",
                default: "—",
                description: "Skip the interactive prompt for the OAuth client id.",
            },
            FlagSpec {
                name: "--client-secret <secret>",
                ty: "string",
                default: "—",
                description: "Required for ymusic; rejected for Spotify (PKCE has no secret).",
            },
            FlagSpec {
                name: "--no-browser",
                ty: "bool",
                default: "false",
                description: "Print the authorize URL to stderr instead of opening a browser.",
            },
        ],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai auth",
            "spotifai auth --provider ymusic --client-id <id> --client-secret <secret>",
            "spotifai auth --no-browser",
        ],
    },
    CommandSpec {
        name: "api",
        usage: "spotifai api <verb> [args...]",
        summary: "Dispatch a typed call into the in-process zad library and print the JSON response. Requires SPOTIFAI_PROFILE.",
        flags: &[FlagSpec {
            name: "<verb> [args...]",
            ty: "trailing-var-arg",
            default: "—",
            description: "Verb plus arguments parsed by `crate::api::parse_verb` (e.g. `search \"query\"`, `playlists list`, `library tracks list`).",
        }],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "SPOTIFAI_PROVIDER=spotify SPOTIFAI_PROFILE=ask spotifai api playlists list",
            "SPOTIFAI_PROVIDER=ymusic SPOTIFAI_PROFILE=ask spotifai api library list",
            "SPOTIFAI_PROVIDER=spotify SPOTIFAI_PROFILE=ask spotifai api search \"daft punk\"",
        ],
    },
    CommandSpec {
        name: "ask",
        usage: "spotifai ask [--provider <slug>] [query...]",
        summary: "Open an interactive zag session with the read-only ask permission profile injected into the system prompt.",
        flags: &[
            FlagSpec {
                name: "--provider <slug>",
                ty: "enum",
                default: "spotify",
                description: "Backing provider (spotify, ymusic).",
            },
            FlagSpec {
                name: "[query...]",
                ty: "string",
                default: "—",
                description: "Optional opening question. Joined with spaces.",
            },
        ],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai ask",
            "spotifai ask \"What are my most-played albums?\"",
            "spotifai ask --provider ymusic \"how many liked videos do I have?\"",
        ],
    },
    CommandSpec {
        name: "playlist",
        usage: "spotifai playlist [--provider <slug>] [query...]",
        summary: "Open a zag session with the playlist profile injected so the agent can build one new playlist on the active provider.",
        flags: &[
            FlagSpec {
                name: "--provider <slug>",
                ty: "enum",
                default: "spotify",
                description: "Backing provider for the new playlist (spotify, ymusic).",
            },
            FlagSpec {
                name: "[query...]",
                ty: "string",
                default: "—",
                description: "Optional brief. Joined with spaces and used as the agent's first turn.",
            },
        ],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai playlist \"a 30-min focus playlist with no vocals\"",
            "spotifai playlist --provider ymusic \"upbeat 45-minute commute mix\"",
            "spotifai --yolo playlist \"a 200-song running playlist\"",
        ],
    },
    CommandSpec {
        name: "clean",
        usage: "spotifai clean [--provider <slug>] [query...]",
        summary: "Open a zag session with the clean profile injected so the agent can delete playlists, remove tracks from playlists, and unsave items from the user's library on the active provider.",
        flags: &[
            FlagSpec {
                name: "--provider <slug>",
                ty: "enum",
                default: "spotify",
                description: "Backing provider whose library to clean up (spotify, ymusic).",
            },
            FlagSpec {
                name: "[query...]",
                ty: "string",
                default: "—",
                description: "Optional cleanup brief. Joined with spaces and used as the agent's first turn.",
            },
        ],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai clean \"remove all baby songs — my child is 15 now\"",
            "spotifai clean --provider ymusic \"delete my 'old phone' playlist\"",
            "spotifai clean \"unsave every saved album from before 2010\"",
        ],
    },
    CommandSpec {
        name: "export",
        usage: "spotifai export [--provider <slug>] [--output <path>] [--pretty]",
        summary: "Dump the user's library on the active provider into one structured JSON document on stdout.",
        flags: &[
            FlagSpec {
                name: "--provider <slug>",
                ty: "enum",
                default: "spotify",
                description: "Provider whose library to export (spotify, ymusic).",
            },
            FlagSpec {
                name: "--output <path>, -o <path>",
                ty: "path",
                default: "—",
                description: "Write the JSON document to this path instead of stdout.",
            },
            FlagSpec {
                name: "--pretty",
                ty: "bool",
                default: "false",
                description: "Pretty-print with two-space indent instead of one dense line.",
            },
        ],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai export > library.json",
            "spotifai export --provider ymusic --pretty -o ~/backups/ymusic.json",
            "spotifai export --provider spotify | spotifai import --provider ymusic --dry-run",
        ],
    },
    CommandSpec {
        name: "import",
        usage: "spotifai import [--provider <slug>] [--input <path>] [--dry-run]",
        summary: "Recreate playlists from a `spotifai export` envelope on the active provider.",
        flags: &[
            FlagSpec {
                name: "--provider <slug>",
                ty: "enum",
                default: "spotify",
                description: "Provider to import the playlists onto (spotify, ymusic).",
            },
            FlagSpec {
                name: "--input <path>, -i <path>",
                ty: "path",
                default: "stdin",
                description: "Read the envelope from this file instead of stdin.",
            },
            FlagSpec {
                name: "--dry-run",
                ty: "bool",
                default: "false",
                description: "Preview without making any zad write calls.",
            },
        ],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai import --input library.json",
            "spotifai export --provider spotify | spotifai import --provider ymusic",
            "spotifai import --provider ymusic --input library.json --dry-run",
        ],
    },
    CommandSpec {
        name: "commands",
        usage: "spotifai commands [<name>] [--examples]",
        summary: "Machine-readable command index (§12.4). Lists commands or prints the full usage spec / examples for one.",
        flags: &[
            FlagSpec {
                name: "<name>",
                ty: "string",
                default: "—",
                description: "Restrict the output to a single command.",
            },
            FlagSpec {
                name: "--examples",
                ty: "bool",
                default: "false",
                description: "Print realistic example invocations instead of the usage spec.",
            },
        ],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai commands",
            "spotifai commands ask",
            "spotifai commands --examples",
            "spotifai commands export --examples",
        ],
    },
    CommandSpec {
        name: "man",
        usage: "spotifai man [<command>]",
        summary: "Print the embedded reference manpage for a command. With no argument, lists every command that has a manpage.",
        flags: &[FlagSpec {
            name: "<command>",
            ty: "string",
            default: "—",
            description: "Command whose manpage to print (e.g. `ask`, `export`).",
        }],
        exit_codes: COMMON_EXIT_CODES,
        examples: &["spotifai man", "spotifai man ask", "spotifai man export"],
    },
    CommandSpec {
        name: "docs",
        usage: "spotifai docs [<topic>]",
        summary: "Print an embedded conceptual doc. With no argument, lists every topic that has a doc.",
        flags: &[FlagSpec {
            name: "<topic>",
            ty: "string",
            default: "—",
            description: "Topic whose doc to print (e.g. `getting-started`, `configuration`, `architecture`).",
        }],
        exit_codes: COMMON_EXIT_CODES,
        examples: &[
            "spotifai docs",
            "spotifai docs getting-started",
            "spotifai docs configuration",
        ],
    },
];

/// Resolve a [`CommandSpec`] by name.
pub fn lookup(name: &str) -> Option<&'static CommandSpec> {
    COMMAND_SPECS.iter().find(|c| c.name == name)
}

/// Render the §12.4 command index. With `name = None`, lists every
/// command. With `name = Some(...)`, prints the full usage spec for
/// one. With `examples = true`, prints example invocations instead.
pub fn run(name: Option<&str>, examples: bool) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match (name, examples) {
        (None, false) => {
            for spec in COMMAND_SPECS {
                let _ = writeln!(out, "{:<9}  {}", spec.name, spec.usage);
            }
        }
        (None, true) => {
            for spec in COMMAND_SPECS {
                let _ = writeln!(out, "## {}", spec.name);
                for ex in spec.examples {
                    let _ = writeln!(out, "  {ex}");
                }
                let _ = writeln!(out);
            }
        }
        (Some(name), false) => {
            let Some(spec) = lookup(name) else {
                bail!("unknown command: {name}. Run `spotifai commands` for the list.");
            };
            print_spec(&mut out, spec);
        }
        (Some(name), true) => {
            let Some(spec) = lookup(name) else {
                bail!("unknown command: {name}. Run `spotifai commands` for the list.");
            };
            for ex in spec.examples {
                let _ = writeln!(out, "{ex}");
            }
        }
    }
    Ok(())
}

fn print_spec<W: Write>(out: &mut W, spec: &CommandSpec) {
    let _ = writeln!(out, "Name:    {}", spec.name);
    let _ = writeln!(out, "Usage:   {}", spec.usage);
    let _ = writeln!(out, "Summary: {}", spec.summary);
    if !spec.flags.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "Flags / arguments:");
        for f in spec.flags {
            let _ = writeln!(out, "  {}  ({}; default: {})", f.name, f.ty, f.default);
            let _ = writeln!(out, "      {}", f.description);
        }
    }
    if !spec.exit_codes.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "Exit codes:");
        for (code, desc) in spec.exit_codes {
            let _ = writeln!(out, "  {code}  {desc}");
        }
    }
}
