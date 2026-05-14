//! `--help-agent` and `--debug-agent` content (§12.1, §12.2).
//!
//! Both surfaces are prompt-injectable: plain text on stdout, no ANSI
//! escapes, generated from the same source of truth that drives
//! `--help`, `commands`, and the manpages so the surfaces cannot drift.
//!
//! `print_help_agent` answers "what is this tool and how do I use it?".
//! `print_debug_agent` answers "why is it broken and how do I debug
//! it?". The contents stay short (~50–200 lines) so they don't dominate
//! the surrounding prompt when spliced via command substitution.

use std::io::Write as _;

use crate::commands_index::COMMAND_SPECS;
use crate::logging;

/// `--help-agent`: compact, prompt-injectable description (§12.1).
pub fn print_help_agent() {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "spotifai {}", crate::version());
    let _ = writeln!(
        out,
        "A Rust CLI for managing your music library and playlists via natural-language"
    );
    let _ = writeln!(
        out,
        "queries. Wraps two upstream tools as in-process libraries: zag (the LLM agent"
    );
    let _ = writeln!(
        out,
        "runtime) and zad (the music-service API client for Spotify and YouTube Music)."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Top-level commands:");
    for spec in COMMAND_SPECS {
        let _ = writeln!(out, "  {:<9}  {}", spec.name, spec.summary);
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "Important flags and environment variables:");
    let _ = writeln!(
        out,
        "  --debug                 Echo debug-level diagnostics to stderr (file log captures debug regardless)."
    );
    let _ = writeln!(
        out,
        "  --provider <slug>       Backing music provider for ask|playlist|export|import|auth (default: spotify; also: ymusic)."
    );
    let _ = writeln!(
        out,
        "  --wait | --no-wait      Sleep through (or fail fast on) an active rate-limit cooldown window recorded by zad at ~/.zad/state/<service>/rate_limit.json (Spotify HTTP 429, or ymusic HTTP 429 / Google-quota HTTP 403). Default: wait for ask|playlist, fail-fast for one-shot commands. SPOTIFAI_WAIT overrides the default."
    );
    let _ = writeln!(
        out,
        "  --yolo                  Run the underlying zag agent with maximum permissions — skip every per-tool approval prompt. Only meaningful for ask|playlist. The spotifai (provider, profile) policy file is still enforced at the zad layer, so --yolo cannot widen the allowed verb list."
    );
    let _ = writeln!(
        out,
        "  SPOTIFAI_PROVIDER       Active provider read by `spotifai api` (set on your behalf by parent commands)."
    );
    let _ = writeln!(
        out,
        "  SPOTIFAI_PROFILE        Active permission profile read by `spotifai api` (`ask` or `playlist`)."
    );
    let _ = writeln!(
        out,
        "  SPOTIFAI_WAIT           Set to 1 by `spotifai ask` and `spotifai playlist` so child `spotifai api` shells sleep through 429 cooldowns instead of erroring out."
    );
    let _ = writeln!(
        out,
        "  SPOTIFAI_LOG            tracing_subscriber EnvFilter directive for the always-on debug.log."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Discovery (recommended for agents):");
    let _ = writeln!(
        out,
        "  spotifai commands                       List every command, one per line, grep-friendly."
    );
    let _ = writeln!(
        out,
        "  spotifai commands <name>                Full usage spec for one command (flags, types, defaults, exit codes)."
    );
    let _ = writeln!(
        out,
        "  spotifai commands --examples            Realistic example invocations for every command."
    );
    let _ = writeln!(
        out,
        "  spotifai commands <name> --examples     Realistic examples for one command only."
    );
    let _ = writeln!(
        out,
        "  spotifai man <name>                     Embedded reference manpage for <name>."
    );
    let _ = writeln!(
        out,
        "  spotifai docs <topic>                   Embedded conceptual doc for <topic>."
    );
    let _ = writeln!(
        out,
        "  spotifai --debug-agent                  Troubleshooting context for failure investigation."
    );
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "Binary: spotifai (version {} — see `spotifai --version`).",
        crate::version()
    );
}

/// `--debug-agent`: troubleshooting context (§12.2).
pub fn print_debug_agent() {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "spotifai --debug-agent (version {})", crate::version());
    let _ = writeln!(
        out,
        "Compact troubleshooting context for an LLM investigating a failure."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Log file (always on, debug level captured):");
    let _ = writeln!(out, "  Linux:   ~/.local/state/spotifai/debug.log");
    let _ = writeln!(
        out,
        "  macOS:   ~/Library/Application Support/spotifai/debug.log"
    );
    let _ = writeln!(out, "  Windows: %APPDATA%\\spotifai\\debug.log");
    if let Some(p) = logging::path() {
        let _ = writeln!(out, "  Resolved on this machine: {}", p.display());
    }
    let _ = writeln!(
        out,
        "  Format: tracing fmt layer (no ANSI). Append-only — no rotation."
    );
    let _ = writeln!(out, "  Inspect: tail -200 <path>; truncate: : > <path>.");
    let _ = writeln!(out);

    let _ = writeln!(out, "Config and state paths (read in this order):");
    let _ = writeln!(
        out,
        "  ~/.spotifai/permissions/<provider>/<profile>.toml   Per-(provider, profile) permission policy (signed)."
    );
    let _ = writeln!(
        out,
        "  ~/.spotifai/<provider>.toml                          Per-provider self-id cache (set by `spotifai auth`)."
    );
    let _ = writeln!(
        out,
        "  ~/.zad/signing/trusted.toml                          Per-machine zad trust store (Ed25519 signatures)."
    );
    let _ = writeln!(
        out,
        "  OS keychain (account `zad/signing:v1`)               Per-machine signing key minted by `spotifai install`."
    );
    let _ = writeln!(
        out,
        "  OS keychain (service `zad`)                          OAuth tokens written by `spotifai auth`."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Environment variables:");
    let _ = writeln!(
        out,
        "  SPOTIFAI_PROVIDER       Active provider read by `spotifai api`. Set by `ask|playlist|export|import` parents."
    );
    let _ = writeln!(
        out,
        "  SPOTIFAI_PROFILE        Active profile read by `spotifai api` (`ask` or `playlist`). Required for direct `api` calls."
    );
    let _ = writeln!(
        out,
        "  SPOTIFAI_LOG            tracing_subscriber EnvFilter directive (default: debug)."
    );
    let _ = writeln!(
        out,
        "  ZAD_PERMISSIONS_PATH    Pinned by spotifai before each zad library call; manual override discouraged."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Common failure modes — diagnostic commands:");
    let _ = writeln!(
        out,
        "  Missing/expired OAuth tokens         spotifai auth [--provider <slug>]"
    );
    let _ = writeln!(
        out,
        "  Permissions file fails trust check   spotifai install   # re-signs every <provider>/<profile>.toml"
    );
    let _ = writeln!(
        out,
        "  Agent self-restricts unexpectedly    cat ~/.spotifai/permissions/<provider>/<profile>.toml"
    );
    let _ = writeln!(
        out,
        "  Stale/zombie debug.log               truncate the file (path above) — no rotation in v1"
    );
    let _ = writeln!(
        out,
        "  Provider slug invalid                spotifai api -- (run with no args for usage; valid slugs: spotify, ymusic)"
    );
    let _ = writeln!(
        out,
        "  zad/zag library mismatch             cargo tree -p spotifai | grep -E '^[├└] (zad|zag)'"
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Verbosity controls:");
    let _ = writeln!(
        out,
        "  --debug                 Lifts debug-level events onto stderr in addition to debug.log."
    );
    let _ = writeln!(
        out,
        "  SPOTIFAI_LOG=trace      Fine-grained verbosity for the file writer (per-target filters supported)."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Reproducer / bug-report bundle:");
    let _ = writeln!(out, "  1. Re-run with --debug and SPOTIFAI_LOG=trace.");
    let _ = writeln!(
        out,
        "  2. Capture the resulting debug.log tail (last ~200 lines) and the exact CLI invocation."
    );
    let _ = writeln!(
        out,
        "  3. Include `spotifai --version` output and the resolved permissions file path."
    );
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "Build metadata: spotifai {}; zad library {}.",
        crate::version(),
        zad::version(),
    );
}
