// Single source of truth for the SEO `<head>` block. Used by both
// `vite.config.mjs` (which injects it into the canonical
// `dist/index.html` during build / `index.html` during dev) and
// `scripts/splice-routes.mjs` (which generates per-route HTML shells
// for every dynamic route after the build runs). Keeping the renderer
// in one place ensures sub-pages don't drift away from the homepage's
// metadata shape.
//
// What the head block contains, per OSS_SPEC §11.3 plus the SEO
// hardening pass:
//
//   - Title, description, keywords, robots directive.
//   - Canonical link, sitemap autodiscovery link.
//   - Open Graph + Twitter card tags.
//   - Theme color, app name, author meta, og:locale.
//   - JSON-LD blocks: a `WebPage`/`SoftwareApplication`/`TechArticle`
//     for the page itself, a `BreadcrumbList` for sub-pages, plus a
//     site-wide `WebSite` (with `SearchAction`) and `Person` (the
//     author) on the canonical homepage to feed Google's sitelinks
//     search box and authorship signals.

import { siteConfig, absoluteUrl } from "../../src/seo/siteConfig.mjs";

export function renderHead(route, options = {}) {
  const { sitemapHref = siteConfig.paths.sitemap } = options;
  const pageUrl = absoluteUrl(route.path);
  const ogImage = absoluteUrl(siteConfig.ogImage.path);
  const title = route.title || siteConfig.tagline;
  const description = route.description || siteConfig.description;
  const isHome = route.path === "/";
  const lines = [
    `<title>${escapeHtml(title)}</title>`,
    `<meta name="description" content="${escapeAttr(description)}" />`,
    `<meta name="keywords" content="${escapeAttr(siteConfig.keywords.join(", "))}" />`,
    `<meta name="author" content="${escapeAttr(siteConfig.author)}" />`,
    `<meta name="application-name" content="${escapeAttr(siteConfig.name)}" />`,
    `<meta name="apple-mobile-web-app-title" content="${escapeAttr(siteConfig.name)}" />`,
    `<meta name="theme-color" content="${escapeAttr(siteConfig.themeColor)}" />`,
    `<meta name="color-scheme" content="dark light" />`,
    `<meta name="generator" content="Vite + React" />`,
    `<meta name="referrer" content="strict-origin-when-cross-origin" />`,
    `<meta name="robots" content="index,follow,max-snippet:-1,max-image-preview:large,max-video-preview:-1" />`,
    `<meta name="googlebot" content="index,follow" />`,
    `<meta name="bingbot" content="index,follow" />`,
    `<link rel="canonical" href="${escapeAttr(pageUrl)}" />`,
    `<link rel="sitemap" type="application/xml" href="${escapeAttr(sitemapHref)}" />`,
    `<link rel="author" href="${escapeAttr(siteConfig.authorUrl)}" />`,
    `<meta property="og:site_name" content="${escapeAttr(siteConfig.name)}" />`,
    `<meta property="og:type" content="${escapeAttr(isHome ? "website" : "article")}" />`,
    `<meta property="og:locale" content="${escapeAttr(siteConfig.locale)}" />`,
    `<meta property="og:title" content="${escapeAttr(title)}" />`,
    `<meta property="og:description" content="${escapeAttr(description)}" />`,
    `<meta property="og:url" content="${escapeAttr(pageUrl)}" />`,
    `<meta property="og:image" content="${escapeAttr(ogImage)}" />`,
    `<meta property="og:image:width" content="${siteConfig.ogImage.width}" />`,
    `<meta property="og:image:height" content="${siteConfig.ogImage.height}" />`,
    `<meta property="og:image:alt" content="${escapeAttr(siteConfig.ogImage.alt)}" />`,
    `<meta name="twitter:card" content="summary_large_image" />`,
    `<meta name="twitter:site" content="${escapeAttr(siteConfig.twitter)}" />`,
    `<meta name="twitter:creator" content="${escapeAttr(siteConfig.twitter)}" />`,
    `<meta name="twitter:title" content="${escapeAttr(title)}" />`,
    `<meta name="twitter:description" content="${escapeAttr(description)}" />`,
    `<meta name="twitter:image" content="${escapeAttr(ogImage)}" />`,
    `<meta name="twitter:image:alt" content="${escapeAttr(siteConfig.ogImage.alt)}" />`,
  ];

  for (const block of jsonLdBlocks(route, { pageUrl, ogImage, isHome })) {
    lines.push(
      `<script type="application/ld+json">${JSON.stringify(block)}</script>`,
    );
  }

  return lines.join("\n    ");
}

function jsonLdBlocks(route, ctx) {
  const blocks = [pageJsonLd(route, ctx)];
  if (route.breadcrumb && route.breadcrumb.length > 1) {
    blocks.push(breadcrumbJsonLd(route.breadcrumb));
  }
  if (ctx.isHome) {
    blocks.push(websiteJsonLd());
    blocks.push(personJsonLd());
  }
  return blocks;
}

function pageJsonLd(route, { pageUrl, ogImage }) {
  const base = {
    "@context": "https://schema.org",
    "@type": route.schemaType || "WebPage",
    "@id": pageUrl,
    name: route.title || siteConfig.name,
    headline: route.title || siteConfig.tagline,
    description: route.description || siteConfig.description,
    url: pageUrl,
    image: ogImage,
    inLanguage: siteConfig.language,
    isPartOf: { "@type": "WebSite", "@id": siteConfig.url, name: siteConfig.name },
    author: {
      "@type": "Person",
      name: siteConfig.author,
      url: siteConfig.authorUrl,
    },
    publisher: {
      "@type": "Person",
      name: siteConfig.author,
      url: siteConfig.authorUrl,
    },
    keywords: siteConfig.keywords.join(", "),
  };
  if (route.schemaType === "SoftwareApplication") {
    return {
      ...base,
      applicationCategory: "DeveloperApplication",
      applicationSubCategory: "CommandLineApplication",
      operatingSystem: "Linux, macOS, Windows",
      offers: {
        "@type": "Offer",
        price: "0",
        priceCurrency: "USD",
        availability: "https://schema.org/InStock",
      },
      downloadUrl: `${siteConfig.repository}/releases`,
      softwareHelp: absoluteUrl("/docs"),
      codeRepository: siteConfig.repository,
      license: `${siteConfig.repository}/blob/main/LICENSE`,
    };
  }
  return base;
}

function breadcrumbJsonLd(breadcrumb) {
  return {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    itemListElement: breadcrumb.map((crumb, idx) => ({
      "@type": "ListItem",
      position: idx + 1,
      name: crumb.name,
      item: absoluteUrl(crumb.path),
    })),
  };
}

function websiteJsonLd() {
  return {
    "@context": "https://schema.org",
    "@type": "WebSite",
    "@id": siteConfig.url,
    url: siteConfig.url,
    name: siteConfig.name,
    description: siteConfig.description,
    inLanguage: siteConfig.language,
    publisher: {
      "@type": "Person",
      name: siteConfig.author,
      url: siteConfig.authorUrl,
    },
    potentialAction: {
      "@type": "SearchAction",
      target: {
        "@type": "EntryPoint",
        urlTemplate: `${siteConfig.url.replace(/\/+$/, "")}/docs?q={search_term_string}`,
      },
      "query-input": "required name=search_term_string",
    },
  };
}

function personJsonLd() {
  return {
    "@context": "https://schema.org",
    "@type": "Person",
    "@id": siteConfig.authorUrl,
    name: siteConfig.author,
    url: siteConfig.authorUrl,
    sameAs: [siteConfig.repository, `https://twitter.com/${siteConfig.twitter.replace(/^@/, "")}`],
  };
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
