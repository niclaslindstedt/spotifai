//! `spotifai man <command>` — embedded reference manpages (§12.3).
//!
//! Manpages live in `man/<command>.md` in the source tree; this module
//! includes them at compile time via `include_str!` so the binary
//! works offline. The `MANPAGES` array is the single source of truth
//! shared with the §12.3 freshness tests that cross-check it against
//! the [`crate::commands_index`] command list.

use std::io::Write as _;

use anyhow::{Result, bail};

/// `(command-name, embedded-markdown)` pairs for every manpage shipped
/// in the binary. Keep this list in `commands_index.rs` declaration
/// order so the listing surface is predictable.
pub static MANPAGES: &[(&str, &str)] = &[
    ("main", include_str!("../man/main.md")),
    ("install", include_str!("../man/install.md")),
    ("auth", include_str!("../man/auth.md")),
    ("api", include_str!("../man/api.md")),
    ("ask", include_str!("../man/ask.md")),
    ("playlist", include_str!("../man/playlist.md")),
    ("clean", include_str!("../man/clean.md")),
    ("export", include_str!("../man/export.md")),
    ("import", include_str!("../man/import.md")),
];

/// Look up a manpage by command name.
pub fn lookup(name: &str) -> Option<&'static str> {
    MANPAGES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
}

/// `spotifai man [name]`. With no argument, prints the list of
/// available manpages. With a name, prints the embedded markdown.
pub fn run(name: Option<&str>) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match name {
        None => {
            let _ = writeln!(out, "Available manpages:");
            for (n, _) in MANPAGES {
                let _ = writeln!(out, "  spotifai man {n}");
            }
        }
        Some(name) => {
            let Some(body) = lookup(name) else {
                bail!(
                    "unknown manpage: {name}. Run `spotifai man` for the list of available pages."
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
