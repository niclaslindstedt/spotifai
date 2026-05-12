/**
 * Log line styles for the simulated terminal.
 *
 * Adapted verbatim from niclaslindstedt/zag's website/src/data/logStyles.ts
 * — same shape, same animation contract, just remapped to spotifai's
 * Spotify-green palette so the colors match the rest of the site.
 *
 *   \u{2713} → tool result success (green)
 *   \u{2717} → tool result failure (red)
 *   \u{25cf} → session started/ended
 *   \u{276f} → user message
 *   \u{23fa} → assistant message
 *   \u{26a1} → tool call
 *   \u{2026} → reasoning / thinking
 */

export interface LogStyle {
  className: string;
}

/** Named styles that output lines can reference. */
export const LOG_STYLES = {
  /** ✓  success / completion (Spotify-green) */
  success: { className: "text-accent" },
  /** ✗  failure / error (red) */
  failure: { className: "text-[#f87171]" },
  /** ==  section header (Spotify-green, slightly heavier) */
  header: { className: "text-accent font-medium" },
  /** primary output — the "answer" line(s) the user actually cares about */
  primary: { className: "text-text-primary" },
  /** ⏱  assistant / tool-call activity (light Spotify-green) */
  assistant: { className: "text-accent-light" },
  /** ←  tool result arrow (Spotify-green) */
  toolResult: { className: "text-accent" },
  /** diff-stat / counts (secondary text) */
  diffStat: { className: "text-text-secondary" },
  /** default dim output */
  dim: { className: "text-text-dim" },
} as const;

export type LogStyleName = keyof typeof LOG_STYLES;

// ---------------------------------------------------------------------------
// Terminal line types (shared between demo data and animation hook)
// ---------------------------------------------------------------------------

/** A single output line: plain string (defaults to "dim") or annotated. */
export type OutputLine = string | { text: string; style: LogStyleName };

export type TerminalLine =
  | { type: "command"; text: string; typingSpeed?: number }
  | { type: "output"; lines: OutputLine[]; delay?: number }
  | { type: "comment"; text: string }
  | { type: "pause"; duration: number };

export type TerminalTab = {
  label: string;
  sequence: TerminalLine[];
};

/** Produced by useTerminalAnimation, consumed by TerminalLine renderer. */
export type RenderedLine = {
  text: string;
  type: "command" | "output" | "comment";
  style?: LogStyleName;
  isActive: boolean;
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Resolve an OutputLine to its text and style name. */
export function resolveOutputLine(line: OutputLine): {
  text: string;
  style: LogStyleName;
} {
  if (typeof line === "string") {
    return { text: line, style: "dim" };
  }
  return line;
}
