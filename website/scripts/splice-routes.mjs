// Per-route SPA shells (OSS_SPEC §11.3).
//
// Generic crawlers don't execute JavaScript and won't wait for
// client-side `<head>` mutations, so the website needs a real HTML
// file per public route — same hydration root as `dist/index.html`,
// but with a route-specific `<head>` block. We do that by reading the
// just-built `dist/index.html`, re-running the same SEO transform
// vite uses at dev/build time, and writing the result to
// `dist/<route>/index.html` for every route in `siteConfig.routes`
// other than `/`. We also emit `dist/404.html` (a copy of `/`) so
// GitHub Pages' SPA fallback hands unknown URLs back to the React
// router with a sensible head.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { siteConfig, absoluteUrl } from "../src/seo/siteConfig.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(__dirname, "..");
const distDir = path.join(websiteRoot, "dist");
const indexPath = path.join(distDir, "index.html");

if (!fs.existsSync(indexPath)) {
  console.error(
    "splice-routes: dist/index.html does not exist — did `vite build` run?",
  );
  process.exit(1);
}

const baseShell = fs.readFileSync(indexPath, "utf8");

// Vite has already rewritten <!-- SEO_HEAD --> for the `/` route on
// the canonical index. To produce per-route variants we rip the
// existing `<head>` block back out and replace it with the route-
// specific equivalent. The `<title>` plus everything between it and
// the closing `</head>` is what vite's `seoHeadPlugin` injected, so
// dropping that range and inserting a fresh block keeps every other
// `<head>` element (charset, viewport, asset links injected by vite)
// intact.
const headInjectionRe =
  /(<title>[\s\S]*?<\/title>[\s\S]*?<script type="application\/ld\+json">[\s\S]*?<\/script>)/;

if (!headInjectionRe.test(baseShell)) {
  console.error(
    "splice-routes: could not locate the SEO head block in dist/index.html — extractor or vite plugin may be out of sync",
  );
  process.exit(1);
}

let routesWritten = 0;
for (const route of siteConfig.routes) {
  if (route.path === "/") continue;
  const html = baseShell.replace(headInjectionRe, renderHead(route));
  const outPath = routeHtmlPath(route.path);
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, html);
  routesWritten += 1;
  console.log(
    "wrote",
    path.relative(process.cwd(), outPath),
    `(route ${route.path})`,
  );
}

// SPA fallback for GH Pages — unknown URLs get the homepage shell so
// the React router can take over after hydration.
const fallbackPath = path.join(distDir, "404.html");
fs.writeFileSync(fallbackPath, baseShell);
console.log("wrote", path.relative(process.cwd(), fallbackPath), "(SPA fallback)");

console.log(`splice-routes: ${routesWritten} per-route HTML shell(s) written`);

// Mirrors the head block produced by `vite.config.mjs`'s
// `seoHeadPlugin`. Keep this in sync with that renderer — the SEO
// copy itself comes from `siteConfig`, so the rendering logic is the
// only thing that has to be duplicated, and the sole consumer is this
// build-time pass.
function renderHead(route) {
  const pageUrl = absoluteUrl(route.path);
  const ogImage = absoluteUrl(siteConfig.ogImage.path);
  const title = route.title || siteConfig.tagline;
  const description = route.description || siteConfig.description;
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": route.schemaType || "WebPage",
    "@id": pageUrl,
    name: siteConfig.name,
    description,
    url: pageUrl,
    image: ogImage,
    applicationCategory: "DeveloperApplication",
    operatingSystem: "Linux, macOS, Windows",
    author: { "@type": "Person", name: siteConfig.author },
    keywords: siteConfig.keywords.join(", "),
  };
  // Vite's HTML transform rewrites root-relative `href`s on the
  // canonical `dist/index.html` (so e.g. `/sitemap.xml` becomes
  // `/spotifai/sitemap.xml` under a configured base). Splicing
  // happens after that pass, so we have to apply the same prefix
  // ourselves for the per-route shells to point at the right URL.
  const sitemapHref =
    siteConfig.basePath.replace(/\/+$/, "") +
    "/" +
    siteConfig.paths.sitemap.replace(/^\/+/, "");
  const lines = [
    `<title>${escapeHtml(title)}</title>`,
    `<meta name="description" content="${escapeAttr(description)}" />`,
    `<meta name="keywords" content="${escapeAttr(siteConfig.keywords.join(", "))}" />`,
    `<meta name="robots" content="index,follow,max-image-preview:large" />`,
    `<link rel="canonical" href="${escapeAttr(pageUrl)}" />`,
    `<link rel="sitemap" type="application/xml" href="${escapeAttr(sitemapHref)}" />`,
    `<meta property="og:site_name" content="${escapeAttr(siteConfig.name)}" />`,
    `<meta property="og:type" content="website" />`,
    `<meta property="og:title" content="${escapeAttr(title)}" />`,
    `<meta property="og:description" content="${escapeAttr(description)}" />`,
    `<meta property="og:url" content="${escapeAttr(pageUrl)}" />`,
    `<meta property="og:image" content="${escapeAttr(ogImage)}" />`,
    `<meta property="og:image:width" content="${siteConfig.ogImage.width}" />`,
    `<meta property="og:image:height" content="${siteConfig.ogImage.height}" />`,
    `<meta property="og:image:alt" content="${escapeAttr(siteConfig.ogImage.alt)}" />`,
    `<meta name="twitter:card" content="summary_large_image" />`,
    `<meta name="twitter:title" content="${escapeAttr(title)}" />`,
    `<meta name="twitter:description" content="${escapeAttr(description)}" />`,
    `<meta name="twitter:image" content="${escapeAttr(ogImage)}" />`,
    `<script type="application/ld+json">${JSON.stringify(jsonLd)}</script>`,
  ];
  return lines.join("\n    ");
}

function routeHtmlPath(routePath) {
  if (routePath === "/") return path.join(distDir, "index.html");
  const trimmed = routePath.replace(/^\/+|\/+$/g, "");
  return path.join(distDir, trimmed, "index.html");
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeAttr(s) {
  return escapeHtml(s).replace(/"/g, "&quot;");
}
