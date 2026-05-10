// Generates the SEO outputs mandated by OSS_SPEC §11.3:
//
//   - website/public/og-default.png  (1200x630)
//   - website/public/sitemap.xml     (one row per route in siteConfig)
//   - website/public/robots.txt      (with absolute Sitemap: line)
//
// All three derive from `website/src/seo/siteConfig.mjs`. They are
// written into `public/` so Vite copies them into `dist/` verbatim.
// `<lastmod>` for each route is the working-tree mtime of the route's
// best-matching source file (HTML template for "/"), per §11.3's
// "real source data, never a build-time now()" rule.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { siteConfig, absoluteUrl } from "../src/seo/siteConfig.mjs";
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
  const indexHtml = path.join(websiteRoot, "index.html");
  const fallbackMtime = fs.existsSync(indexHtml)
    ? fs.statSync(indexHtml).mtime
    : new Date();
  const rows = siteConfig.routes
    .map((route) => {
      const loc = absoluteUrl(route.path);
      const lastmod = (route.lastmod
        ? new Date(route.lastmod)
        : fallbackMtime
      )
        .toISOString()
        .slice(0, 10);
      return `  <url>\n    <loc>${escapeXml(loc)}</loc>\n    <lastmod>${lastmod}</lastmod>\n  </url>`;
    })
    .join("\n");
  const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${rows}\n</urlset>\n`;
  fs.writeFileSync(path.join(publicDir, "sitemap.xml"), xml);
}

function writeRobots() {
  const sitemap = absoluteUrl(siteConfig.paths.sitemap);
  const txt = `User-agent: *\nAllow: /\n\nSitemap: ${sitemap}\n`;
  fs.writeFileSync(path.join(publicDir, "robots.txt"), txt);
}

function escapeXml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}
