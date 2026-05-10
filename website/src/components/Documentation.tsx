import { useParams, Link, Navigate } from "react-router-dom";
import { useState, useEffect } from "react";
import { sourceData } from "../generated/sourceData";
import MarkdownRenderer from "./MarkdownRenderer";

const docs = sourceData.docs;

// Pin the most-used pages to the top; everything else falls back to
// alphabetical order from the extractor. The order list is editorial
// — it matches the README's "Documentation" section so the sidebar
// reads the same way.
const preferredOrder = [
  "getting-started",
  "configuration",
  "architecture",
  "export_schema",
  "troubleshooting",
];

function orderedDocs() {
  const known = new Set(preferredOrder);
  const seen = new Set<string>();
  const ordered = [] as typeof docs;
  for (const slug of preferredOrder) {
    const found = docs.find((d) => d.slug === slug);
    if (found) {
      ordered.push(found);
      seen.add(slug);
    }
  }
  for (const doc of docs) {
    if (!known.has(doc.slug) && !seen.has(doc.slug)) ordered.push(doc);
  }
  return ordered;
}

export default function Documentation() {
  const { slug } = useParams<{ slug: string }>();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const fallback = docs[0]?.slug ?? "";
  const currentSlug = slug || fallback;
  const currentDoc = docs.find((d) => d.slug === currentSlug);
  const sidebarDocs = orderedDocs();

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [currentSlug]);

  if (!docs.length) {
    return (
      <div className="min-h-screen pt-32 px-6 text-center text-text-secondary">
        No documentation pages were found in <code>docs/</code>.
      </div>
    );
  }

  if (!currentDoc) {
    return <Navigate to={`/docs/${fallback}`} replace />;
  }

  return (
    <div className="min-h-screen pt-[73px]">
      <div className="sticky top-[73px] z-40 border-b border-border bg-surface/95 backdrop-blur-sm px-4 py-3 lg:hidden">
        <button
          onClick={() => setSidebarOpen(!sidebarOpen)}
          className="flex items-center gap-2 text-sm text-text-secondary hover:text-text-primary transition-colors"
        >
          <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d={sidebarOpen ? "M6 18L18 6M6 6l12 12" : "M4 6h16M4 12h16M4 18h16"} />
          </svg>
          {currentDoc.title}
        </button>
      </div>

      {sidebarOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/50 lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      <div className="mx-auto flex max-w-7xl">
        <aside
          className={`
            fixed top-[118px] bottom-0 z-40 w-full shrink-0 overflow-y-auto border-r border-border bg-surface px-4 py-6
            transition-transform duration-200 ease-in-out
            sm:w-72
            lg:sticky lg:top-[73px] lg:w-64 lg:translate-x-0 lg:block
            ${sidebarOpen ? "translate-x-0" : "-translate-x-full"}
          `}
        >
          <div className="mb-3 px-3 text-xs font-semibold uppercase tracking-wider text-text-dim">
            Documentation
          </div>
          <nav className="space-y-1">
            {sidebarDocs.map((doc) => (
              <Link
                key={doc.slug}
                to={`/docs/${doc.slug}`}
                onClick={() => setSidebarOpen(false)}
                className={`
                  block rounded-md px-3 py-2 text-sm transition-colors
                  ${doc.slug === currentSlug
                    ? "bg-accent/10 text-accent font-medium"
                    : "text-text-secondary hover:bg-surface-hover hover:text-text-primary"
                  }
                `}
              >
                {doc.title}
              </Link>
            ))}
          </nav>
        </aside>

        <main className="min-w-0 flex-1 px-6 py-8 lg:px-12 lg:py-10">
          <MarkdownRenderer content={currentDoc.content} basePath="/docs" />
        </main>
      </div>
    </div>
  );
}
