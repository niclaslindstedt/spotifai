// Generates the SEO outputs mandated by OSS_SPEC §11.3:
//
//   - website/public/og-default.png  (1200x630)
//   - website/public/sitemap.xml     (one row per crawlable route —
//                                     including every /docs/<slug> and
//                                     /manual/<slug> under the SPA)
//   - website/public/robots.txt      (with absolute Sitemap: line)
//
// All three derive from `website/src/seo/siteConfig.mjs` plus the
// shared route enumerator under `scripts/lib/routes.mjs`. They are
// written into `public/` so Vite copies them into `dist/` verbatim.
// `<lastmod>` comes from the per-route source file's git history (or
// its working-tree mtime when git is unavailable), per §11.3's
// "real source data, never a build-time now()" rule.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { siteConfig, absoluteUrl } from "../src/seo/siteConfig.mjs";
import { enumerateRoutes, dateOnly } from "./lib/routes.mjs";
import { encodePng, parseHexColor } from "./png.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(__dirname, "..");
const publicDir = path.join(websiteRoot, "public");
fs.mkdirSync(publicDir, { recursive: true });

writeOgImage();
writeSitemap();
writeRobots();
console.log("wrote SEO outputs to", path.relative(process.cwd(), publicDir));

function writeOgImage() {
  const { width, height, bg, accent } = siteConfig.ogImage;
  const bgRgba = parseHexColor(bg);
  const accentRgba = parseHexColor(accent);
  const pixels = Buffer.alloc(width * height * 4);
  // Fill background.
  for (let i = 0; i < width * height; i++) {
    pixels[i * 4] = bgRgba[0];
    pixels[i * 4 + 1] = bgRgba[1];
    pixels[i * 4 + 2] = bgRgba[2];
    pixels[i * 4 + 3] = bgRgba[3];
  }
  // Accent stripe along the left edge — reads as branding even
  // without text rendering.
  const stripeWidth = Math.round(width * 0.04);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < stripeWidth; x++) {
      const idx = (y * width + x) * 4;
      pixels[idx] = accentRgba[0];
      pixels[idx + 1] = accentRgba[1];
      pixels[idx + 2] = accentRgba[2];
      pixels[idx + 3] = accentRgba[3];
    }
  }
  // Subtle bottom band in the same accent color.
  const bandStart = Math.round(height * 0.92);
  for (let y = bandStart; y < height; y++) {
    for (let x = stripeWidth; x < width; x++) {
      const idx = (y * width + x) * 4;
      pixels[idx] = accentRgba[0];
      pixels[idx + 1] = accentRgba[1];
      pixels[idx + 2] = accentRgba[2];
      pixels[idx + 3] = accentRgba[3];
    }
  }
  const png = encodePng(width, height, pixels);
  fs.writeFileSync(path.join(publicDir, "og-default.png"), png);
}

function writeSitemap() {
  const routes = enumerateRoutes();
  const rows = routes
    .map((route) => {
      const loc = absoluteUrl(route.path);
      const lastmod = dateOnly(route.lastmod);
      return [
        "  <url>",
        `    <loc>${escapeXml(loc)}</loc>`,
        `    <lastmod>${lastmod}</lastmod>`,
        `    <changefreq>${route.changefreq}</changefreq>`,
        `    <priority>${route.priority.toFixed(1)}</priority>`,
        "  </url>",
      ].join("\n");
    })
    .join("\n");
  const xml =
    `<?xml version="1.0" encoding="UTF-8"?>\n` +
    `<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n` +
    `${rows}\n` +
    `</urlset>\n`;
  fs.writeFileSync(path.join(publicDir, "sitemap.xml"), xml);
}

function writeRobots() {
  const sitemap = absoluteUrl(siteConfig.paths.sitemap);
  // Generic open policy, then explicit allow-everything stanzas for
  // the major search-engine crawlers so their fetchers can prove their
  // user-agent matched a rule. Sitemap line is global per RFC.
  const lines = [
    "# Robots policy for spotifai. See https://www.robotstxt.org/.",
    "User-agent: *",
    "Allow: /",
    "",
    "User-agent: Googlebot",
    "Allow: /",
    "",
    "User-agent: Googlebot-Image",
    "Allow: /",
    "",
    "User-agent: Bingbot",
    "Allow: /",
    "",
    "User-agent: DuckDuckBot",
    "Allow: /",
    "",
    "User-agent: Applebot",
    "Allow: /",
    "",
    `Sitemap: ${sitemap}`,
    "",
  ];
  fs.writeFileSync(path.join(publicDir, "robots.txt"), lines.join("\n"));
}

function escapeXml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}
