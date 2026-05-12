import type { ReactNode } from "react";

/**
 * Inline code-style snippet for commands and filenames embedded in
 * body copy. Renders as a <code> with the shared `.inline-code` class
 * (defined in App.css) so all commands and filenames across the site
 * look the same regardless of the surrounding section background.
 */
export default function InlineCode({ children }: { children: ReactNode }) {
  return <code className="inline-code">{children}</code>;
}
