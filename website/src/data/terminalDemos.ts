// Adapter that converts the README-extracted ExampleGroup[] from
// `generated/sourceData.ts` into the TerminalTab[] shape that the
// zag-derived terminal animation engine consumes.
//
// Keeping this thin means the underlying terminal subsystem
// (TerminalShell + useTerminalAnimation + LOG_STYLES) stays a verbatim
// copy of niclaslindstedt/zag's, and only this file knows how to
// translate spotifai's "Quick start" prose into a typing-animation
// sequence with sensible colors.

import type { ExampleGroup } from "../generated/sourceData";
import type {
  LogStyleName,
  OutputLine,
  TerminalLine,
  TerminalTab,
} from "./logStyles";

/**
 * Produce a one-or-two-word tab label.
 *
 * Labels are derived from the first command's shape so they remain
 * stable even if the prose comments above the command change. Extend
 * the table below when the README grows a new Quick start example.
 */
export function tabLabel(group: ExampleGroup, index: number): string {
  const command = group.steps[0]?.command ?? "";
  if (
    /spotifai\s+export\b/.test(command) &&
    /spotifai\s+import\b/.test(command)
  ) {
    return "Migrate";
  }
  const match = /spotifai\s+([a-z-]+)/.exec(command);
  if (match) {
    return match[1].charAt(0).toUpperCase() + match[1].slice(1);
  }
  return `Demo ${index + 1}`;
}

/**
 * Pick a LOG_STYLES name for one line of program output. The
 * conventions mirror those in the README's Quick start block.
 */
function styleForOutput(line: string): LogStyleName {
  const trimmed = line.trim();
  if (!trimmed) return "dim";
  if (/^==\s.*\s==$/.test(trimmed)) return "header";
  if (/^[✓✔]/.test(trimmed)) return "success";
  if (/^[✗✘]/.test(trimmed)) return "failure";
  if (/^\d+\.\s/.test(trimmed)) return "primary";
  if (/^Created\s+`/.test(trimmed)) return "success";
  if (/^Your\s.+:$/.test(trimmed)) return "primary";
  return "dim";
}

function annotate(lines: string[]): OutputLine[] {
  return lines.map((text) => ({ text, style: styleForOutput(text) }));
}

/**
 * Translate one ExampleGroup into a TerminalLine[] sequence the
 * animation hook understands. Each step contributes:
 *   - the typed command
 *   - a short pause
 *   - its output lines (slightly delayed so the typing finishes first)
 *   - a beat before the next step
 */
function groupToSequence(group: ExampleGroup): TerminalLine[] {
  const seq: TerminalLine[] = [];
  for (let i = 0; i < group.steps.length; i++) {
    const step = group.steps[i];
    seq.push({ type: "command", text: step.command });
    seq.push({ type: "pause", duration: 280 });
    if (step.output.length) {
      seq.push({ type: "output", delay: 180, lines: annotate(step.output) });
    }
    if (i < group.steps.length - 1) {
      seq.push({ type: "pause", duration: 480 });
    }
  }
  seq.push({ type: "pause", duration: 2200 });
  return seq;
}

export function exampleGroupsToTabs(groups: ExampleGroup[]): TerminalTab[] {
  return groups.map((group, i) => ({
    label: tabLabel(group, i),
    sequence: groupToSequence(group),
  }));
}
