import { Link } from "react-router-dom";
import { sourceData } from "../generated/sourceData";

const lastUpdatedDate = new Date(sourceData.lastUpdated.iso);
const lastUpdatedLabel = Number.isNaN(lastUpdatedDate.getTime())
  ? sourceData.lastUpdated.iso
  : lastUpdatedDate.toISOString().slice(0, 10);

export default function Footer() {
  return (
    <footer className="border-t border-border py-12">
      <div className="mx-auto max-w-6xl px-6">
        <div className="flex flex-col items-center justify-between gap-6 md:flex-row">
          <div>
            <span className="text-lg font-bold text-text-primary">
              <span className="text-accent">&#127925;</span> {sourceData.name}
            </span>
            <p className="mt-1 text-sm text-text-dim">
              Spotify, by way of natural language. v{sourceData.version}.
            </p>
            <p className="mt-1 text-xs text-text-dim">
              Last updated{" "}
              <time dateTime={sourceData.lastUpdated.iso}>{lastUpdatedLabel}</time>
              {sourceData.lastUpdated.source === "mtime" ? " (working tree)" : ""}
            </p>
          </div>

          <div className="flex flex-wrap justify-center gap-x-6 gap-y-2 text-sm text-text-secondary">
            <a
              href={sourceData.repository}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-text-primary transition-colors"
            >
              GitHub
            </a>
            <Link
              to="/docs/getting-started"
              className="hover:text-text-primary transition-colors"
            >
              Documentation
            </Link>
            <Link
              to="/manual/main"
              className="hover:text-text-primary transition-colors"
            >
              Manual
            </Link>
            <a
              href={`https://crates.io/crates/${sourceData.name}`}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-text-primary transition-colors"
            >
              crates.io
            </a>
            <a
              href={`${sourceData.repository}/blob/main/CHANGELOG.md`}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-text-primary transition-colors"
            >
              Changelog
            </a>
            <a
              href={`${sourceData.repository}/blob/main/LICENSE`}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-text-primary transition-colors"
            >
              {sourceData.license} License
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
}
