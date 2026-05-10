// Vite configuration. The custom plugin imports `siteConfig` (the
// single source of truth for SEO copy mandated by OSS_SPEC §11.3) and
// splices route-aware <head> metadata into every served / built HTML
// page. This is the "runtime client code" half of §11.3's SSOT
// requirement; the build-time generators in `scripts/build-seo.mjs`
// import the same module, and the per-route HTML splicer in
// `scripts/splice-routes.mjs` shares the `renderHead` helper so
// homepage and sub-page metadata never drift.

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

import { siteConfig } from "./src/seo/siteConfig.mjs";
import { renderHead } from "./scripts/lib/render-head.mjs";

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

export default defineConfig({
  base: siteConfig.basePath,
  plugins: [tailwindcss(), react(), seoHeadPlugin()],
  build: {
    outDir: "dist",
  },
});
