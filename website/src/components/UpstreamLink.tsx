import type { ReactNode } from "react";

const REPOS: Record<string, string> = {
  zag: "https://github.com/niclaslindstedt/zag",
  zad: "https://github.com/niclaslindstedt/zad",
};

/**
 * External link to one of the upstream crates (zag or zad). Used so
 * every mention of those names across the site is consistently a
 * clickable reference to the upstream repository.
 */
export default function UpstreamLink({
  name,
  children,
}: {
  name: "zag" | "zad";
  children?: ReactNode;
}) {
  return (
    <a
      href={REPOS[name]}
      target="_blank"
      rel="noopener noreferrer"
      className="text-text-primary underline decoration-accent/40 underline-offset-2 hover:text-accent transition-colors"
    >
      {children ?? name}
    </a>
  );
}
