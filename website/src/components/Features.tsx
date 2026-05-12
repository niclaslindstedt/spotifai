import { sourceData } from "../generated/sourceData";

const providerList = sourceData.providers.map((p) => p.displayName).join(" and ");

const features = [
  {
    title: "Plain-English library queries",
    description: `Ask "what are my most recently added albums?" or "list every playlist with more than 100 tracks." spotifai ask opens a read-only zag session with the right permissions profile injected into the system prompt.`,
    icon: "\u{1F5E3}\u{FE0F}",
  },
  {
    title: "Conversational playlist curator",
    description: `spotifai playlist builds one new playlist per session. The agent can search, add, and rename — but never delete, never overwrite, never touch your saved library. Hand the brief in plain English and walk away.`,
    icon: "\u{1F3B7}",
  },
  {
    title: "Per-command permission profiles",
    description: `Two profiles per provider — ask.toml is read-only, playlist.toml adds three curator verbs. Both are signed at install time with a per-machine Ed25519 key in the OS keychain. zad fails closed at load time on any unsigned change.`,
    icon: "\u{1F510}",
  },
  {
    title: `${providerList} out of the box`,
    description: `Switch backends with --provider. Every command (ask, playlist, export, import, auth) respects the flag. Adding a third provider is one new variant in src/providers.rs — the rest of the codebase picks it up automatically.`,
    icon: "\u{1F500}",
  },
  {
    title: "Cross-provider library migration",
    description: `spotifai export | spotifai import is the canonical migration form: dump your Spotify library, recreate the playlists on YouTube Music. Tracks resolve via ISRC, then title + primary artist. Same-name playlists are skipped, so re-runs are idempotent.`,
    icon: "\u{1F501}",
  },
];

export default function Features() {
  return (
    <section id="features" className="border-t border-border py-20 md:py-28">
      <div className="mx-auto max-w-6xl px-6">
        <h2 className="text-center text-3xl font-bold text-text-primary md:text-4xl">
          A polite agent for your music library
        </h2>
        <p className="mx-auto mt-4 max-w-2xl text-center text-text-secondary">
          {sourceData.name} is a thin Rust CLI that wires natural-language queries through zag and routes
          the resulting actions through zad — with per-command permission profiles so the agent only ever
          uses the verbs you signed off on.
        </p>

        <div className="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <div
              key={f.title}
              className="group rounded-xl border border-border bg-surface-alt p-6 transition-all hover:border-accent/40 hover:bg-surface-hover"
            >
              <div className="mb-4 text-2xl">{f.icon}</div>
              <h3 className="mb-2 text-lg font-semibold text-text-primary">{f.title}</h3>
              <p className="text-sm leading-relaxed text-text-secondary">{f.description}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
