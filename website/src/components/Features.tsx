import InlineCode from "./InlineCode";
import UpstreamLink from "./UpstreamLink";
import { sourceData } from "../generated/sourceData";

const providerList = sourceData.providers.map((p) => p.displayName).join(" and ");

const features = [
  {
    title: "Plain-English library queries",
    description: (
      <>
        Ask &ldquo;what are my most recently added albums?&rdquo; or &ldquo;list every
        playlist with more than 100 tracks.&rdquo; <InlineCode>spotifai ask</InlineCode>{" "}
        starts a read-only chat with your library — the AI can search and look
        around, but it cannot change anything you have saved.
      </>
    ),
    icon: "\u{1F5E3}\u{FE0F}",
  },
  {
    title: "Conversational playlist curator",
    description: (
      <>
        <InlineCode>spotifai playlist</InlineCode> builds one new playlist per
        session. Hand the brief in plain English &mdash; the AI can search, add
        and rename, but it can never delete a playlist, overwrite your saved
        library or touch anything you already have.
      </>
    ),
    icon: "\u{1F3B7}",
  },
  {
    title: "Polite library cleanup",
    description: (
      <>
        <InlineCode>spotifai clean</InlineCode> is the destructive surface
        &mdash; delete a playlist, drop songs from a playlist, unsave old
        albums. It always enumerates the candidates first and asks for an
        explicit yes/no before deleting anything; it cannot add, create or
        search the public catalogue.
      </>
    ),
    icon: "\u{1F9F9}",
  },
  {
    title: "Locked down by default",
    description: (
      <>
        Every command runs against a short, fixed list of music-service actions
        &mdash; just enough to do its job, and no more. There is simply no
        command that can wipe your library, even if the AI goes off the rails.
      </>
    ),
    icon: "\u{1F510}",
  },
  {
    title: `${providerList} out of the box`,
    description: (
      <>
        Switch backends with a single flag. Every command &mdash; ask, playlist,
        export, import and auth &mdash; works the same way on both services, so
        you only have to learn one tool.
      </>
    ),
    icon: "\u{1F500}",
  },
  {
    title: "Cross-provider library migration",
    description: (
      <>
        <InlineCode>spotifai export | spotifai import</InlineCode> moves your
        library between services in one pipe. Tracks are matched first by their
        unique audio code, then by title and artist. Re-runs skip playlists
        that already exist, so a half-finished migration is safe to repeat.
      </>
    ),
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
          {sourceData.name} is a small command-line tool that turns plain-English
          requests into safe, scoped actions on your{" "}
          {sourceData.providers.map((p) => p.displayName).join(" or ")} library.
          It is built on top of <UpstreamLink name="zag" /> (the AI runtime) and{" "}
          <UpstreamLink name="zad" /> (the music-service wrapper for agentic
          use), and every command has access to just the handful of
          music-service endpoints it needs &mdash; nothing more.
        </p>

        <div className="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <div
              key={f.title}
              className="group rounded-xl border border-border bg-surface-alt p-6 transition-all hover:border-accent/40 hover:bg-surface-hover"
            >
              <div className="mb-2 flex items-center gap-3">
                <span className="text-2xl leading-none">{f.icon}</span>
                <h3 className="text-lg font-semibold text-text-primary">{f.title}</h3>
              </div>
              <p className="text-sm leading-relaxed text-text-secondary">{f.description}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
