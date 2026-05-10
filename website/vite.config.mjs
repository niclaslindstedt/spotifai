// Vite configuration. The custom plugin imports `siteConfig` (the
// single source of truth for SEO copy mandated by OSS_SPEC §11.3) and
// splices route-aware <head> metadata into every served / built HTML
// page. This is the "runtime client code" half of §11.3's SSOT
// requirement; the build-time generators in `scripts/build-seo.mjs`
// import the same module.

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

import { siteConfig, absoluteUrl } from "./src/seo/siteConfig.mjs";

function seoHeadPlugin() {
  return {
    name: "spotifai-seo-head",
    transformIndexHtml: {
      order: "pre",
      handler(html, ctx) {
        const route = pickRoute(ctx?.path);
        const head = renderHead(route);
        return html
          .replace(/<!--\s*SEO_HEAD\s*-->/, head)
          .replace(/<html(\s[^>]*)?>/, (m) =>
            /\blang=/.test(m)
              ? m
              : m.replace("<html", `<html lang="${siteConfig.language}"`),
          );
      },
    },
  };
}

function pickRoute(reqPath) {
  const normalized = (reqPath || "/").replace(/\/index\.html$/, "/") || "/";
  return (
    siteConfig.routes.find((r) => r.path === normalized) ||
    siteConfig.routes[0]
  );
}

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
  const lines = [
    `<title>${escapeHtml(title)}</title>`,
    `<meta name="description" content="${escapeAttr(description)}" />`,
    `<meta name="keywords" content="${escapeAttr(siteConfig.keywords.join(", "))}" />`,
    `<meta name="robots" content="index,follow,max-image-preview:large" />`,
    `<link rel="canonical" href="${escapeAttr(pageUrl)}" />`,
    `<link rel="sitemap" type="application/xml" href="${escapeAttr(siteConfig.paths.sitemap)}" />`,
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

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeAttr(s) {
  return escapeHtml(s).replace(/"/g, "&quot;");
}

export default defineConfig({
  base: siteConfig.basePath,
  plugins: [tailwindcss(), react(), seoHeadPlugin()],
  build: {
    outDir: "dist",
  },
});
