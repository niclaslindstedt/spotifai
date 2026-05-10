// Single source of truth for SEO copy and configuration.
//
// Imported by both the runtime client (the Vite HTML transform in
// `vite.config.mjs`, which injects this data into the served
// `<head>`) and every build-time generator under
// `website/scripts/` (sitemap, robots, OG image, verifier). Tweaking
// the site's pitch must be a one-file change. See OSS_SPEC §11.3.

export const siteConfig = {
  name: "spotifai",
  tagline: "Spotify, by way of natural language.",
  description:
    "A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).",
  // Canonical site URL. Used for sitemap `<loc>`, `<link rel=canonical>`,
  // Open Graph `og:url`, and JSON-LD `@id`. Trailing slash matters.
  url: "https://niclaslindstedt.github.io/spotifai/",
  author: "Niclas Lindstedt",
  twitter: "@niclaslindstedt",
  language: "en",
  keywords: [
    "spotify",
    "cli",
    "rust",
    "llm",
    "agent",
    "music",
    "playlist",
    "youtube music",
  ],
  ogImage: {
    path: "/og-default.png",
    width: 1200,
    height: 630,
    alt: "spotifai — Spotify, by way of natural language",
    // Background and accent colors used by the pure-Node PNG generator.
    bg: "#0F172A",
    accent: "#1DB954",
  },
  paths: {
    sitemap: "/sitemap.xml",
    robots: "/robots.txt",
  },
  // Public routes the site exposes. Each entry becomes a sitemap row,
  // an HTML transform target, and a JSON-LD block.
  routes: [
    {
      path: "/",
      title: "spotifai — Spotify, by way of natural language",
      description:
        "A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).",
      schemaType: "SoftwareApplication",
    },
  ],
};

// Build an absolute URL by joining `siteConfig.url` with a path.
export function absoluteUrl(pathname) {
  const base = siteConfig.url.replace(/\/+$/, "");
  const rel = String(pathname || "/").replace(/^\/+/, "");
  return rel ? `${base}/${rel}` : `${base}/`;
}
