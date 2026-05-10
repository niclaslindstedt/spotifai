// Canonical list of crawlable routes for the spotifai marketing site.
//
// `siteConfig.routes` lists the *static* sections (`/`, `/docs`, `/manual`).
// This module fans those out to include every per-doc and per-manpage
// page reachable through the SPA router (`/docs/<slug>`, `/manual/<slug>`)
// so the sitemap, the splicer, and the SEO verifier all agree on the
// same set of indexable URLs. Without this expansion crawlers without
// JavaScript only ever see three pages even though `docs/` and `man/`
// hold ~13 markdown files, all of which we want indexed individually.
//
// One module is the SSOT used by `build-seo.mjs`, `splice-routes.mjs`,
// and `verify-seo.mjs`. Resolution rules:
//
//   - `lastmod`     latest git commit touching the source file, falling
//                   back to its working-tree mtime, falling back to
//                   `index.html`'s mtime. Per OSS_SPEC §11.3 the value
//                   must come from real source data — never a build-time
//                   `now()`.
//   - `changefreq`  conservative SEO hint: `weekly` for the landing
//                   page (release notes, hero copy churn), `monthly`
//                   for everything else.
//   - `priority`    `1.0` for `/`, `0.9` for the section indexes
//                   (`/docs`, `/manual`), `0.7` for individual
//                   documentation pages, `0.6` for individual manpages.

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { siteConfig } from "../../src/seo/siteConfig.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
export const websiteRoot = path.resolve(__dirname, "..", "..");
export const repoRoot = path.resolve(websiteRoot, "..");

const indexFallback = (() => {
  const indexHtml = path.join(websiteRoot, "index.html");
  return fs.existsSync(indexHtml) ? fs.statSync(indexHtml).mtime : new Date();
})();

export function enumerateRoutes() {
  const routes = [];
  for (const route of siteConfig.routes) {
    if (route.path === "/") {
      routes.push(decorate(route, sourceFileFor(route), {
        changefreq: "weekly",
        priority: 1.0,
      }));
    } else {
      routes.push(decorate(route, sourceFileFor(route), {
        changefreq: "monthly",
        priority: 0.9,
      }));
    }
  }

  for (const doc of listMarkdown("docs")) {
    routes.push(decorate(
      {
        path: `/docs/${doc.slug}`,
        title: titleWithSpotifai(doc.title, "docs"),
        description: doc.description,
        schemaType: "TechArticle",
        breadcrumb: [
          { name: "Home", path: "/" },
          { name: "Documentation", path: "/docs" },
          { name: doc.title, path: `/docs/${doc.slug}` },
        ],
      },
      doc.absPath,
      { changefreq: "monthly", priority: 0.7 },
    ));
  }

  for (const page of listMarkdown("man")) {
    const manName = page.slug === "main"
      ? "spotifai (overview)"
      : `spotifai ${page.slug}`;
    routes.push(decorate(
      {
        path: `/manual/${page.slug}`,
        title: `${manName} — manual`,
        description: page.description,
        schemaType: "TechArticle",
        breadcrumb: [
          { name: "Home", path: "/" },
          { name: "Manual", path: "/manual" },
          { name: manName, path: `/manual/${page.slug}` },
        ],
      },
      page.absPath,
      { changefreq: "monthly", priority: 0.6 },
    ));
  }

  return routes;
}

function decorate(route, sourcePath, hints) {
  const lastmod = lastmodFor(sourcePath);
  return {
    ...route,
    sourcePath,
    lastmod,
    changefreq: hints.changefreq,
    priority: hints.priority,
  };
}

function sourceFileFor(route) {
  if (route.path === "/") return path.join(websiteRoot, "index.html");
  if (route.path === "/docs") return path.join(repoRoot, "docs");
  if (route.path === "/manual") return path.join(repoRoot, "man");
  return path.join(websiteRoot, "index.html");
}

function listMarkdown(dirName) {
  const dir = path.join(repoRoot, dirName);
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".md"))
    .map((file) => {
      const slug = file.replace(/\.md$/, "").toLowerCase();
      const absPath = path.join(dir, file);
      const content = fs.readFileSync(absPath, "utf8");
      const titleMatch = /^#\s+(.+?)\s*$/m.exec(content);
      const title = titleMatch ? titleMatch[1] : humanise(slug);
      const description = firstParagraph(content) || `${title} — spotifai`;
      return { slug, title, description, absPath };
    })
    .sort((a, b) => a.slug.localeCompare(b.slug));
}

function firstParagraph(content) {
  // Strip H1/H2/H3 lines so we don't pick up the title itself.
  const stripped = content.replace(/^#{1,6}\s.*$/gm, "").trim();
  const para = stripped.split(/\n{2,}/).find((block) => {
    const trimmed = block.trim();
    if (!trimmed) return false;
    if (trimmed.startsWith("```")) return false;
    if (trimmed.startsWith("|")) return false;
    if (trimmed.startsWith("-")) return false;
    if (trimmed.startsWith("*")) return false;
    return true;
  });
  if (!para) return null;
  // Many `docs/*.md` and `man/*.md` pages open with a `> tagline`
  // blockquote summary right after the H1 — perfect for the meta
  // description, but only after we strip the leading `> ` markers.
  const flat = para
    .split(/\n/)
    .map((line) => line.replace(/^\s*>\s?/, "").trim())
    .filter(Boolean)
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
  if (!flat) return null;
  return flat.length > 240 ? `${flat.slice(0, 237)}…` : flat;
}

function humanise(slug) {
  return slug
    .split(/[-_]/)
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join(" ");
}

function titleWithSpotifai(title, section) {
  // Doc titles like "Architecture of spotifai" already include the
  // brand; suffixing "— spotifai docs" repeats it. When the title
  // already mentions spotifai, only attach the section noun.
  return /spotifai/i.test(title) ? `${title} — ${section}` : `${title} — spotifai ${section}`;
}

function lastmodFor(absPath) {
  if (absPath) {
    try {
      const stdout = execFileSync(
        "git",
        ["-C", repoRoot, "log", "-1", "--format=%cI", "--", absPath],
        { stdio: ["ignore", "pipe", "ignore"], encoding: "utf8" },
      ).trim();
      if (stdout) return stdout;
    } catch {
      // Fall through to mtime.
    }
    if (fs.existsSync(absPath)) {
      const stat = fs.statSync(absPath);
      return stat.mtime.toISOString();
    }
  }
  return indexFallback.toISOString();
}

export function dateOnly(iso) {
  return new Date(iso).toISOString().slice(0, 10);
}
