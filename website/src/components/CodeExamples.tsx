import { useState } from "react";
import { sourceData } from "../generated/sourceData";

interface Tab {
  label: string;
  description: string;
  code: string;
}

const tabs: Tab[] = [
  {
    label: "Ask",
    description:
      "spotifai ask opens an interactive zag session pre-loaded with the read-only ask.toml profile. The agent talks to your library exclusively through spotifai api.",
    code: `# Quick read-only question against Spotify (the default).
$ spotifai ask "What are my most recently added albums?"

# Same question against YouTube Music.
$ spotifai ask --provider ymusic "What playlists do I have?"

# Drop into an interactive session — no opening question.
$ spotifai ask
`,
  },
  {
    label: "Playlist",
    description:
      "spotifai playlist swaps in playlist.toml — the curator profile that adds three create/add/rename verbs while every destructive verb stays denied.",
    code: `# Build one new Spotify playlist.
$ spotifai playlist "a 30-minute focus playlist with no vocals"

# Build one new YouTube Music playlist.
$ spotifai playlist --provider ymusic \\
    "an upbeat 45-minute commute playlist"

# The agent calls spotifai api for every action — search, then
# playlists create, then playlists add, then (optional) rename.
`,
  },
  {
    label: "Export / Import",
    description:
      "Round-trip your library through the unified spotifai schema. Same-provider re-imports reuse the embedded IDs; cross-provider migrations resolve tracks via ISRC then title + primary artist.",
    code: `# Migrate from Spotify to YouTube Music in a single pipe.
$ spotifai export --provider spotify | \\
  spotifai import --provider ymusic

# Snapshot one provider, pretty-printed, to a file.
$ spotifai export --provider ymusic --pretty \\
    -o ~/backups/ymusic.json

# Preview an import without making any zad write calls.
$ cat library.json | spotifai import --provider ymusic --dry-run
`,
  },
  {
    label: "API",
    description:
      "spotifai api is the typed verb the agent uses under the hood. You can call it yourself once a profile is active (set by ask / playlist / export / import).",
    code: `# Search the catalogue (default: track + album + artist + playlist).
$ spotifai api search "moon river"

# List your playlists, JSON-piped through jq.
$ spotifai api playlists list --json | jq '.[] | .name'

# Show a specific playlist by id or name.
$ spotifai api playlists show "Focus" --json

# Create a new playlist (curator profile only).
$ spotifai api playlists create --name "New mix" --json
`,
  },
  {
    label: "Auth & Install",
    description:
      "Setup is two commands. spotifai install scaffolds and signs the per-(provider, profile) permission files; spotifai auth runs the in-process OAuth loopback flow per provider.",
    code: `# 1. Bootstrap the signing key, scaffold and sign every profile.
$ spotifai install

# 2. Authenticate against Spotify (PKCE, no client secret).
$ spotifai auth

# (Optional) authenticate against YouTube Music.
$ spotifai auth --provider ymusic \\
    --client-id <id> --client-secret <secret>

# Re-run install after editing any allowed/denied list to resign it.
$ $EDITOR ~/.spotifai/permissions/spotify/playlist.toml
$ spotifai install
`,
  },
];

export default function CodeExamples() {
  const [active, setActive] = useState(0);

  return (
    <section
      id="examples"
      className="border-t border-border bg-surface-alt py-20 md:py-28"
    >
      <div className="mx-auto max-w-4xl px-6">
        <h2 className="text-center text-3xl font-bold text-text-primary md:text-4xl">
          See it in action
        </h2>
        <p className="mx-auto mt-4 max-w-2xl text-center text-text-secondary">
          From a one-shot question against your library to a full cross-provider migration —
          {" "}{sourceData.name} keeps the interface consistent.
        </p>

        <div className="mt-12 overflow-hidden rounded-xl border border-border bg-surface shadow-2xl">
          <div className="flex overflow-x-auto border-b border-border">
            {tabs.map((t, i) => (
              <button
                key={t.label}
                onClick={() => setActive(i)}
                className={`shrink-0 whitespace-nowrap px-5 py-3 text-sm font-medium transition-colors ${
                  i === active
                    ? "border-b-2 border-accent text-accent bg-surface-alt"
                    : "text-text-dim hover:text-text-secondary"
                }`}
              >
                {t.label}
              </button>
            ))}
          </div>
          <div className="border-b border-border px-6 py-3 text-xs text-text-dim">
            {tabs[active].description}
          </div>
          <pre className="overflow-x-auto p-6 text-sm leading-relaxed text-text-secondary">
            <code>{tabs[active].code}</code>
          </pre>
        </div>
      </div>
    </section>
  );
}
