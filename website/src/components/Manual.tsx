import { useParams, Link, Navigate } from "react-router-dom";
import { useState, useEffect, useMemo } from "react";
import { sourceData } from "../generated/sourceData";
import MarkdownRenderer from "./MarkdownRenderer";

const manpages = sourceData.manpages;

// Group the man pages by role so the sidebar reads like the table in
// `man/main.md` rather than alphabetical order. The "main" page goes
// first; everything else slots into setup vs agent surfaces vs
// pipelines. Pages we don't have an explicit slot for fall back to
// "Other" at the bottom.
const groupOrder = ["Top-level", "Setup", "Agent surfaces", "Library round-trip", "Other"];

const groupAssignments: Record<string, string> = {
  main: "Top-level",
  install: "Setup",
  auth: "Setup",
  ask: "Agent surfaces",
  playlist: "Agent surfaces",
  api: "Agent surfaces",
  export: "Library round-trip",
  import: "Library round-trip",
};

interface Group {
  label: string;
  pages: typeof manpages;
}

function groupPages(): Group[] {
  const buckets: Record<string, typeof manpages> = {};
  for (const page of manpages) {
    const label = groupAssignments[page.slug] ?? "Other";
    (buckets[label] ??= []).push(page);
  }
  return groupOrder
    .map((label) => ({ label, pages: buckets[label] ?? [] }))
    .filter((g) => g.pages.length > 0);
}

// Convert internal "see also" backtick references like `man/<cmd>.md`
// into router links. The README + man pages both link this way so the
// rewrite works for either basePath.
function preprocessContent(content: string): string {
  return content
    .replace(/\[`([\w-]+)`\]\(([\w-]+)\.md\)/g, "[`$1`](/manual/$2)")
    .replace(/\[([^\]]+)\]\(\.\/([\w-]+)\.md\)/g, "[$1](/manual/$2)")
    .replace(/`spotifai man ([\w-]+)`/g, "[`spotifai man $1`](/manual/$1)");
}

export default function Manual() {
  const { slug } = useParams<{ slug: string }>();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const fallback = manpages.find((p) => p.slug === "main")?.slug ?? manpages[0]?.slug ?? "";
  const currentSlug = slug || fallback;
  const currentPage = manpages.find((p) => p.slug === currentSlug);

  const processedContent = useMemo(
    () => (currentPage ? preprocessContent(currentPage.content) : ""),
    [currentPage],
  );

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [currentSlug]);

  if (!manpages.length) {
    return (
      <div className="min-h-screen pt-32 px-6 text-center text-text-secondary">
        No manpages were found in <code>man/</code>.
      </div>
    );
  }

  if (!currentPage) {
    return <Navigate to={`/manual/${fallback}`} replace />;
  }

  const groups = groupPages();

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
          {currentPage.title}
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
          <nav className="space-y-4">
            {groups.map((group) => (
              <div key={group.label}>
                <div className="px-3 pb-1 text-xs font-semibold uppercase tracking-wider text-text-secondary">
                  {group.label}
                </div>
                <div className="space-y-0.5">
                  {group.pages.map((page) => (
                    <Link
                      key={page.slug}
                      to={`/manual/${page.slug}`}
                      onClick={() => setSidebarOpen(false)}
                      className={`
                        block rounded-md px-3 py-1.5 text-sm transition-colors
                        ${page.slug === currentSlug
                          ? "bg-accent/10 text-accent font-medium"
                          : "text-text-secondary hover:bg-surface-hover hover:text-text-primary"
                        }
                      `}
                    >
                      {page.slug === "main" ? "spotifai (overview)" : `spotifai ${page.slug}`}
                    </Link>
                  ))}
                </div>
              </div>
            ))}
          </nav>
        </aside>

        <main className="min-w-0 flex-1 px-6 py-8 lg:px-12 lg:py-10">
          <MarkdownRenderer content={processedContent} basePath="/manual" />
        </main>
      </div>
    </div>
  );
}
