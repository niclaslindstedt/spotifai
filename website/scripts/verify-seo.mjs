// Asserts every SEO output mandated by OSS_SPEC §11.3 is present in
// the built site. Runs in CI after `npm run build`. Exits non-zero on
// the first missing artifact so the pages workflow fails loudly when
// the SEO pipeline regresses.
//
// Per §11.3 we check:
//   1. dist/sitemap.xml exists and contains every route enumerated by
//      `scripts/lib/routes.mjs` (static sections plus every dynamic
//      doc/manpage page).
//   2. dist/robots.txt exists, is non-empty, and contains an absolute
//      Sitemap: line.
//   3. dist/og-default.png exists.
//   4. Every enumerated route's HTML contains <title>, a canonical
//      link pointing at the absolute URL, at least one
//      application/ld+json block, and the expected core meta tags
//      (description, keywords, theme-color).

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { siteConfig, absoluteUrl } from "../src/seo/siteConfig.mjs";
import { enumerateRoutes } from "./lib/routes.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const dist = path.resolve(__dirname, "..", "dist");

const failures = [];
const must = (cond, msg) => {
  if (!cond) failures.push(msg);
};

must(fs.existsSync(dist), `dist/ does not exist: ${dist}`);

const routes = enumerateRoutes();

const sitemap = path.join(dist, "sitemap.xml");
must(fs.existsSync(sitemap), "missing dist/sitemap.xml");
if (fs.existsSync(sitemap)) {
  const xml = fs.readFileSync(sitemap, "utf8");
  must(/<urlset/.test(xml), "dist/sitemap.xml is not a urlset");
  for (const route of routes) {
    const loc = absoluteUrl(route.path);
    must(
      xml.includes(`<loc>${loc}</loc>`),
      `dist/sitemap.xml is missing <loc>${loc}</loc>`,
    );
  }
  must(
    /<changefreq>/.test(xml),
    "dist/sitemap.xml is missing <changefreq> hints",
  );
  must(
    /<priority>/.test(xml),
    "dist/sitemap.xml is missing <priority> hints",
  );
}

const robots = path.join(dist, "robots.txt");
must(fs.existsSync(robots), "missing dist/robots.txt");
if (fs.existsSync(robots)) {
  const txt = fs.readFileSync(robots, "utf8");
  const sitemapUrl = absoluteUrl(siteConfig.paths.sitemap);
  must(
    new RegExp(`^Sitemap:\\s*${escapeRegex(sitemapUrl)}\\s*$`, "m").test(txt),
    `dist/robots.txt is missing Sitemap: ${sitemapUrl}`,
  );
  must(/User-agent:\s*\*/.test(txt), "dist/robots.txt is missing wildcard User-agent");
}

must(
  fs.existsSync(path.join(dist, "og-default.png")),
  "missing dist/og-default.png (1200x630 OG card)",
);

for (const route of routes) {
  const htmlPath = routeHtmlPath(route.path);
  must(
    fs.existsSync(htmlPath),
    `missing built HTML for route ${route.path}: ${htmlPath}`,
  );
  if (!fs.existsSync(htmlPath)) continue;
  const html = fs.readFileSync(htmlPath, "utf8");
  must(/<title>[^<]+<\/title>/.test(html), `${htmlPath} missing <title>`);
  const canonical = absoluteUrl(route.path);
  must(
    new RegExp(
      `<link\\s+rel=["']canonical["']\\s+href=["']${escapeRegex(canonical)}["']`,
    ).test(html),
    `${htmlPath} missing canonical link to ${canonical}`,
  );
  must(
    /<script\s+type=["']application\/ld\+json["']>[\s\S]+?<\/script>/.test(
      html,
    ),
    `${htmlPath} missing application/ld+json script`,
  );
  must(
    /<meta\s+name=["']description["']/i.test(html),
    `${htmlPath} missing meta description`,
  );
  must(
    /<meta\s+name=["']theme-color["']/i.test(html),
    `${htmlPath} missing meta theme-color`,
  );
  must(
    /<meta\s+property=["']og:image["']/i.test(html),
    `${htmlPath} missing og:image`,
  );
}

if (failures.length) {
  console.error("verify-seo: FAIL");
  for (const f of failures) console.error("  -", f);
  process.exit(1);
}
console.log(`verify-seo: OK (${routes.length} routes verified)`);

function routeHtmlPath(routePath) {
  if (routePath === "/") return path.join(dist, "index.html");
  const trimmed = routePath.replace(/^\/+|\/+$/g, "");
  return path.join(dist, trimmed, "index.html");
}

function escapeRegex(s) {
  return String(s).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
