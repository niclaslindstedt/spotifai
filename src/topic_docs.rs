//! `spotifai docs <topic>` — embedded conceptual docs (§12.3).
//!
//! Docs live in `docs/<topic>.md` in the source tree; this module
//! includes them at compile time via `include_str!` so the binary
//! works offline. The `TOPICS` array is the single source of truth
//! shared with the §12.3 freshness tests.

use std::io::Write as _;

use anyhow::{Result, bail};

/// `(topic-name, embedded-markdown)` pairs for every doc topic shipped
/// in the binary. Order is the order shown by `spotifai docs` with no
/// argument.
pub static TOPICS: &[(&str, &str)] = &[
    (
        "getting-started",
        include_str!("../docs/getting-started.md"),
    ),
    ("configuration", include_str!("../docs/configuration.md")),
    ("architecture", include_str!("../docs/architecture.md")),
    ("export-schema", include_str!("../docs/export_schema.md")),
    (
        "troubleshooting",
        include_str!("../docs/troubleshooting.md"),
    ),
];

/// Look up a topic doc by name.
pub fn lookup(name: &str) -> Option<&'static str> {
    TOPICS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
}

/// `spotifai docs [topic]`. With no argument, prints the list of
/// available topics. With a topic, prints the embedded markdown.
pub fn run(name: Option<&str>) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match name {
        None => {
            let _ = writeln!(out, "Available topics:");
            for (n, _) in TOPICS {
                let _ = writeln!(out, "  spotifai docs {n}");
            }
        }
        Some(name) => {
            let Some(body) = lookup(name) else {
                bail!(
                    "unknown topic: {name}. Run `spotifai docs` for the list of available topics."
                );
            };
            let _ = out.write_all(body.as_bytes());
            if !body.ends_with('\n') {
                let _ = writeln!(out);
            }
        }
    }
    Ok(())
}
