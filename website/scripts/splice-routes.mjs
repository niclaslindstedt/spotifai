// Per-route SPA shells (OSS_SPEC §11.3).
//
// Generic crawlers don't execute JavaScript and won't wait for
// client-side `<head>` mutations, so the website needs a real HTML
// file per public route — same hydration root as `dist/index.html`,
// but with a route-specific `<head>` block. We do that by reading the
// just-built `dist/index.html`, re-running the same `renderHead`
// helper that vite uses for the canonical entry, and writing the
// result to `dist/<route>/index.html` for every route enumerated by
// `scripts/lib/routes.mjs` other than `/`. That set includes both the
// hand-curated `siteConfig.routes` (`/docs`, `/manual`) and every
// dynamic doc/manpage page (`/docs/<slug>`, `/manual/<slug>`) so
// Googlebot indexes each piece of content as its own URL with its own
// title / description / breadcrumb.
//
// We also emit `dist/404.html` (a copy of `/`) so GitHub Pages' SPA
// fallback hands unknown URLs back to the React router with a sensible
// head.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { siteConfig } from "../src/seo/siteConfig.mjs";
import { enumerateRoutes } from "./lib/routes.mjs";
import { renderHead } from "./lib/render-head.mjs";

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
// existing SEO block back out and replace it with the route-specific
// equivalent. The block runs from the `<title>` vite injected through
// the *last* `application/ld+json` script. Capturing the whole span
// in one greedy match keeps every other `<head>` element (charset,
// viewport, asset links injected by vite) intact.
const headInjectionRe =
  /<title>[\s\S]*?<\/title>[\s\S]*?<script type="application\/ld\+json">[\s\S]*?<\/script>(?:\s*<script type="application\/ld\+json">[\s\S]*?<\/script>)*/;

if (!headInjectionRe.test(baseShell)) {
  console.error(
    "splice-routes: could not locate the SEO head block in dist/index.html — extractor or vite plugin may be out of sync",
  );
  process.exit(1);
}

// Vite's HTML transform rewrites root-relative `href`s on the
// canonical `dist/index.html` according to `siteConfig.basePath`.
// Splicing happens after that pass, so we apply the same prefix
// ourselves for the per-route shells to point at the right URL
// (a no-op when the site is served from a domain root).
const sitemapHref =
  siteConfig.basePath.replace(/\/+$/, "") +
  "/" +
  siteConfig.paths.sitemap.replace(/^\/+/, "");

const routes = enumerateRoutes();
let routesWritten = 0;
for (const route of routes) {
  if (route.path === "/") continue;
  const html = baseShell.replace(headInjectionRe, renderHead(route, { sitemapHref }));
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

function routeHtmlPath(routePath) {
  if (routePath === "/") return path.join(distDir, "index.html");
  const trimmed = routePath.replace(/^\/+|\/+$/g, "");
  return path.join(distDir, trimmed, "index.html");
}
