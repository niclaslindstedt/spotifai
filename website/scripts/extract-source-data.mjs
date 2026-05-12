// Extract project metadata from the Rust source tree so the website
// never goes stale (OSS_SPEC §11.2). Runs as the first step of every
// `npm run dev` / `npm run build` / `npm run extract` and emits a
// single generated module that the React components import.
//
// Inputs (resolved at the repo root):
//   - Cargo.toml                  (version, description, repository)
//   - src/providers.rs            (provider enum + per-profile policies)
//   - man/main.md                 (subcommand list + descriptions)
//   - docs/<topic>.md             (hosted-docs corpus)
//   - man/<command>.md            (hosted-manpage corpus)
//   - examples/                   (runnable shell-script demos)
//
// Output:
//   - website/src/generated/sourceData.ts
//
// Failure mode: if a required marker is missing (the version key, the
// providers enum block, the man/<cmd>.md corpus, …) the script exits
// non-zero — never silently emits stale data.

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(websiteRoot, "..");

const cargoToml = readFile(path.join(repoRoot, "Cargo.toml"));
const providersRs = readFile(path.join(repoRoot, "src", "providers.rs"));
const mainMd = readFile(path.join(repoRoot, "man", "main.md"));
const readmeMd = readFile(path.join(repoRoot, "README.md"));

const version = mustMatch(
  /^version\s*=\s*"([^"]+)"/m,
  cargoToml,
  "Cargo.toml [package].version",
);
const description = mustMatch(
  /^description\s*=\s*"([^"]+)"/m,
  cargoToml,
  "Cargo.toml [package].description",
);
const repository = mustMatch(
  /^repository\s*=\s*"([^"]+)"/m,
  cargoToml,
  "Cargo.toml [package].repository",
);
const license = mustMatch(
  /^license\s*=\s*"([^"]+)"/m,
  cargoToml,
  "Cargo.toml [package].license",
);
const rustVersion = mustMatch(
  /^rust-version\s*=\s*"([^"]+)"/m,
  cargoToml,
  "Cargo.toml [package].rust-version",
);
const zadVersion = mustMatch(
  /^zad\s*=\s*"([^"]+)"/m,
  cargoToml,
  "Cargo.toml [dependencies].zad",
);
const zagVersion = mustMatch(
  /^zag\s*=\s*"([^"]+)"/m,
  cargoToml,
  "Cargo.toml [dependencies].zag",
);

const providers = extractProviders(providersRs);
const commands = extractCommands(mainMd);
const docs = extractMarkdownCorpus(path.join(repoRoot, "docs"));
const manpages = extractMarkdownCorpus(path.join(repoRoot, "man"));
const examples = extractExamples(readmeMd);
const lastUpdated = lastSourceCommit();

const out = {
  name: "spotifai",
  version,
  description,
  repository,
  license,
  rustVersion,
  zadVersion,
  zagVersion,
  lastUpdated,
  providers,
  commands,
  docs,
  manpages,
  examples,
};

const dest = path.join(websiteRoot, "src", "generated");
fs.mkdirSync(dest, { recursive: true });
const outPath = path.join(dest, "sourceData.ts");
fs.writeFileSync(outPath, renderTs(out));
console.log("wrote", path.relative(process.cwd(), outPath));

// ---- helpers ----------------------------------------------------------

function readFile(absPath) {
  if (!fs.existsSync(absPath)) {
    fail(`missing source file: ${path.relative(repoRoot, absPath)}`);
  }
  return fs.readFileSync(absPath, "utf8");
}

function mustMatch(re, source, label) {
  const m = re.exec(source);
  if (!m) fail(`could not extract ${label}`);
  return m[1];
}

function fail(msg) {
  console.error(`extract-source-data: ${msg}`);
  process.exit(1);
}

// Most recent commit on disk that touches a source-of-truth file the
// website renders. Used by the footer's "last updated" stamp so the
// site never claims to be fresher than its source data. Falls back to
// the working-tree mtime of `Cargo.toml` if `git` is unavailable
// (e.g. on a release artifact extracted from a tarball).
function lastSourceCommit() {
  try {
    const stdout = execFileSync(
      "git",
      [
        "-C",
        repoRoot,
        "log",
        "-1",
        "--format=%cI",
        "--",
        "Cargo.toml",
        "README.md",
        "src",
        "docs",
        "man",
      ],
      { stdio: ["ignore", "pipe", "ignore"], encoding: "utf8" },
    ).trim();
    if (stdout) {
      return { iso: stdout, source: "git" };
    }
  } catch {
    // Fall through to mtime fallback.
  }
  const stat = fs.statSync(path.join(repoRoot, "Cargo.toml"));
  return { iso: stat.mtime.toISOString(), source: "mtime" };
}

// Walk `src/providers.rs` for the canonical provider list. We grep for
// the small handful of facts the website needs — slug, display name,
// zad subcommand — plus the read-only and curator allowlists out of
// the per-profile match arms. Every provider in `Provider::ALL` must
// resolve all four facets or the extractor exits non-zero.
function extractProviders(src) {
  const variantBlock = mustMatch(
    /pub enum Provider \{([^}]+)\}/s,
    src,
    "Provider enum body",
  );
  const variants = [...variantBlock.matchAll(/\b([A-Z][A-Za-z0-9]*)\b,/g)].map(
    (m) => m[1],
  );
  if (!variants.length) fail("Provider enum has no variants");

  const slugMap = parseMatchArms(src, "as_str");
  const displayMap = parseMatchArms(src, "display_name");
  const zadSubMap = parseMatchArms(src, "zad_subcommand");

  const askVerbs = parseProfileVerbs(src, "Ask");
  const playlistVerbs = parseProfileVerbs(src, "Playlist");
  const cleanVerbs = parseProfileVerbs(src, "Clean");

  return variants.map((variant) => {
    const slug = slugMap[variant];
    const displayName = displayMap[variant];
    const zadSubcommand = zadSubMap[variant];
    if (!slug || !displayName || !zadSubcommand) {
      fail(
        `Provider::${variant} is missing one of as_str/display_name/zad_subcommand`,
      );
    }
    // The verb arms come from `*_default` helpers, keyed by the
    // canonical slug (`spotify_default` / `ymusic_default`). If a new
    // helper is added the regex below picks it up automatically.
    return {
      variant,
      slug,
      displayName,
      zadSubcommand,
      ask: askVerbs[slug] ?? null,
      playlist: playlistVerbs[slug] ?? null,
      clean: cleanVerbs[slug] ?? null,
    };
  });
}

// Pulls a `match self { Variant => "literal", ... }` block out of a
// known method on `impl Provider`. Returns a `{ Variant: literal }`
// map.
function parseMatchArms(src, methodName) {
  const re = new RegExp(
    `pub fn ${methodName}\\([^)]*\\)[^\\{]*\\{[\\s\\S]*?match self \\{([\\s\\S]*?)\\}`,
    "m",
  );
  const m = re.exec(src);
  if (!m) fail(`could not find Provider::${methodName} match block`);
  const arms = [...m[1].matchAll(/Provider::(\w+)\s*=>\s*"([^"]+)"/g)];
  const out = {};
  for (const arm of arms) out[arm[1]] = arm[2];
  return out;
}

// Pulls the `allowed: vec![...]` / `denied: vec![...]` lists out of a
// `<provider>_default` helper for the given profile arm. Returns a
// `{ slug: { mode, description, allowed, denied } }` map.
function parseProfileVerbs(src, profileVariant) {
  const out = {};
  const re = new RegExp(
    `pub fn (\\w+)_default\\(profile: Profile\\)[\\s\\S]*?Profile::${profileVariant}\\s*=>\\s*Permissions\\s*\\{([\\s\\S]*?\\n\\s*\\},)`,
    "g",
  );
  let m;
  while ((m = re.exec(src)) !== null) {
    const slug = m[1].replace(/_$/, "");
    const body = m[2];
    out[slug] = {
      mode: matchOrNull(/mode:\s*([A-Z_]+)\.to_string\(\)/, body),
      description: collectStringChunks(
        /description:\s*([\s\S]*?\n\s+\.to_string\(\))/,
        body,
      ),
      allowed: collectVecLiterals(/allowed: vec!\[([\s\S]*?)\]/, body),
      denied: collectVecLiterals(/denied: vec!\[([\s\S]*?)\]/, body),
    };
  }
  return out;
}

function matchOrNull(re, body) {
  const m = re.exec(body);
  return m ? m[1] : null;
}

function collectStringChunks(re, body) {
  const m = re.exec(body);
  if (!m) return null;
  // Each Rust string literal we capture may use `\` line continuation,
  // which in the source file appears as `\` followed by a newline and
  // leading whitespace. Strip those before collapsing the rest of the
  // whitespace so the rendered description reads as one prose line.
  return [...m[1].matchAll(/"([^"]*)"/g)]
    .map((x) => x[1].replace(/\\\s*/g, " "))
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}

function collectVecLiterals(re, body) {
  const m = re.exec(body);
  if (!m) return [];
  return [...m[1].matchAll(/"([^"]+)"\s*\.into\(\)/g)].map((x) => x[1]);
}

// Walk `man/main.md`'s subcommand table for the canonical command
// list. The table is owned by hand (one row per subcommand, in source
// order), so the extractor pulls (name, description) pairs straight
// out of it.
function extractCommands(mainMd) {
  const tableMatch =
    /^## Subcommands\s*\n\s*\n(\| Command \| Description \|\n\|[^\n]+\|\n(?:\|[^\n]+\|\n)+)/m.exec(
      mainMd,
    );
  if (!tableMatch) fail("could not find ## Subcommands table in man/main.md");
  const rows = tableMatch[1].split("\n").slice(2).filter(Boolean);
  const cmds = [];
  for (const row of rows) {
    const cells = row
      .split("|")
      .map((c) => c.trim())
      .filter((c, i, arr) => i > 0 && i < arr.length - 1);
    if (cells.length < 2) continue;
    const name = stripBackticks(cells[0]);
    const description = cleanDescription(cells[1]);
    if (name) cmds.push({ name, description });
  }
  if (!cmds.length) fail("subcommand table parsed but yielded zero rows");
  return cmds;
}

function stripBackticks(s) {
  return s.replace(/^`|`$/g, "").trim();
}

function cleanDescription(s) {
  // Collapse whitespace and strip lingering pipe escapes; keep
  // markdown otherwise so `<code>` / `**bold**` still render.
  return s.replace(/\\\|/g, "|").replace(/\s+/g, " ").trim();
}

// Read every markdown file directly under `dir` and emit a sorted
// array of `{ slug, title, content }`. The slug is the filename stem
// (lowercased). The title is the first `# Heading` if present, else
// the slug humanised.
function extractMarkdownCorpus(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".md"))
    .map((file) => {
      const slug = file.replace(/\.md$/, "").toLowerCase();
      const content = fs.readFileSync(path.join(dir, file), "utf8");
      const titleMatch = /^#\s+(.+?)\s*$/m.exec(content);
      const title = titleMatch ? titleMatch[1] : humanise(slug);
      return { slug, title, content };
    })
    .sort((a, b) => a.slug.localeCompare(b.slug));
}

function humanise(slug) {
  return slug
    .split(/[-_]/)
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join(" ");
}

// Pull the fenced shell-script block out of the README's "Quick start"
// section so the landing page's terminal demo always reflects the
// canonical first-five-minutes story.
function extractExamples(readme) {
  // Pull the first fenced `sh` block under `## Quick start`. The
  // surrounding prose in README explains the grammar; the block is
  // structured as: leading `#` lines describe the example, the first
  // non-`#` line is the command (or pipeline), and `#>` lines capture
  // the program's stdout/stderr so the website terminal can render
  // grounded output without re-running anything.
  const m =
    /^## Quick start\s*\n[\s\S]*?```sh\n([\s\S]*?)```/m.exec(readme);
  if (!m) fail("could not find Quick start fenced block in README.md");
  const text = m[1];
  const groups = [];
  let current = { comment: [], steps: [] };
  const flush = () => {
    if (current.steps.length) {
      groups.push(current);
      current = { comment: [], steps: [] };
    }
  };
  for (const rawLine of text.split("\n")) {
    const line = rawLine.replace(/\s+$/, "");
    if (!line) {
      flush();
      continue;
    }
    if (line.startsWith("#>")) {
      if (!current.steps.length) {
        fail(`Quick start: output line precedes any command: ${line}`);
      }
      const stripped = line.replace(/^#>\s?/, "");
      current.steps[current.steps.length - 1].output.push(stripped);
      continue;
    }
    if (line.startsWith("#")) {
      if (current.steps.length) flush();
      current.comment.push(line.replace(/^#\s?/, ""));
      continue;
    }
    current.steps.push({ command: line, output: [] });
  }
  flush();
  if (!groups.length) fail("Quick start block contained no example commands");
  return groups.map((g) => ({
    title: g.comment[0] ?? "",
    comment: g.comment.join(" "),
    steps: g.steps,
  }));
}

// Render the extracted facts as a typed TypeScript module so consumer
// components stay type-checked end-to-end.
function renderTs(data) {
  const banner = [
    "// AUTO-GENERATED by website/scripts/extract-source-data.mjs.",
    "// Do not edit by hand — re-run `npm run extract` (or",
    "// `npm run build`) from the website directory after touching any",
    "// source-of-truth file under the repo root.",
    "",
  ].join("\n");
  const types = `export interface ProviderProfile {
  mode: string | null;
  description: string | null;
  allowed: string[];
  denied: string[];
}

export interface ProviderData {
  variant: string;
  slug: string;
  displayName: string;
  zadSubcommand: string;
  ask: ProviderProfile | null;
  playlist: ProviderProfile | null;
  clean: ProviderProfile | null;
}

export interface CommandData {
  name: string;
  description: string;
}

export interface MarkdownDoc {
  slug: string;
  title: string;
  content: string;
}

export interface ExampleStep {
  command: string;
  output: string[];
}

export interface ExampleGroup {
  title: string;
  comment: string;
  steps: ExampleStep[];
}

export interface LastUpdated {
  iso: string;
  source: "git" | "mtime";
}

export interface SourceData {
  name: string;
  version: string;
  description: string;
  repository: string;
  license: string;
  rustVersion: string;
  zadVersion: string;
  zagVersion: string;
  lastUpdated: LastUpdated;
  providers: ProviderData[];
  commands: CommandData[];
  docs: MarkdownDoc[];
  manpages: MarkdownDoc[];
  examples: ExampleGroup[];
}
`;
  const literal = `export const sourceData: SourceData = ${JSON.stringify(data, null, 2)} as const;\n`;
  return `${banner}\n${types}\n${literal}`;
}
