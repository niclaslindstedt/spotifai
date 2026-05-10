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
  url: "https://spotifai.niclaslindstedt.se/",
  // Path the site is served from on the host. The site lives at the
  // root of a custom domain (see `public/CNAME`), so the React bundle,
  // the router, and asset URLs are all root-relative.
  basePath: "/",
  author: "Niclas Lindstedt",
  authorUrl: "https://niclaslindstedt.se/",
  twitter: "@niclaslindstedt",
  language: "en",
  locale: "en_US",
  themeColor: "#1DB954",
  repository: "https://github.com/niclaslindstedt/spotifai",
  // Extra keywords beyond the obvious ones widen the SERP surface for
  // long-tail queries ("spotify cli", "manage playlists from terminal",
  // etc.) without diluting the page's core relevance.
  keywords: [
    "spotify",
    "spotify cli",
    "spotify command line",
    "spotify terminal",
    "spotify automation",
    "youtube music",
    "youtube music cli",
    "cli",
    "rust",
    "rust cli",
    "llm",
    "llm agent",
    "ai agent",
    "natural language interface",
    "music",
    "music library",
    "playlist",
    "playlist manager",
    "playlist automation",
    "natural language",
    "open source",
    "spotifai",
    "zag",
    "zad",
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
  // a per-route HTML splice target (see `scripts/splice-routes.mjs`),
  // and a JSON-LD block.
  routes: [
    {
      path: "/",
      title: "spotifai — Spotify, by way of natural language",
      description:
        "A Rust CLI for managing your music library and playlists via natural-language queries, powered by zag (agent) and zad (Spotify / YouTube Music integration).",
      schemaType: "SoftwareApplication",
    },
    {
      path: "/docs",
      title: "Documentation — spotifai",
      description:
        "Hosted reference for spotifai: getting started, configuration keys, architecture, the export/import schema, and troubleshooting.",
      schemaType: "TechArticle",
      breadcrumb: [
        { name: "Home", path: "/" },
        { name: "Documentation", path: "/docs" },
      ],
    },
    {
      path: "/manual",
      title: "Manual — spotifai",
      description:
        "Embedded manpages for every spotifai subcommand — install, auth, ask, playlist, api, export, import — rendered for the web.",
      schemaType: "TechArticle",
      breadcrumb: [
        { name: "Home", path: "/" },
        { name: "Manual", path: "/manual" },
      ],
    },
  ],
};

// Build an absolute URL by joining `siteConfig.url` with a path.
export function absoluteUrl(pathname) {
  const base = siteConfig.url.replace(/\/+$/, "");
  const rel = String(pathname || "/").replace(/^\/+/, "");
  return rel ? `${base}/${rel}` : `${base}/`;
}
