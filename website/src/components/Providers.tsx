import InlineCode from "./InlineCode";
import { sourceData } from "../generated/sourceData";

// Each provider gets its own brand color and a short, user-facing
// blurb. Blurbs focus on what the user has to do to connect each
// service and what their library looks like once connected — not on
// the internal HTTP/OAuth mechanics.
const providerBlurbs: Record<
  string,
  { tagline: string; setup: string; library: string; accent: string }
> = {
  spotify: {
    tagline: "Sign in once with your Spotify account — no password ever leaves your machine.",
    setup:
      "Create a free Spotify developer app (a one-time, two-minute click-through), paste its ID into spotifai once, and you're done. After that, signing in just opens a browser tab on your own machine.",
    library:
      "Liked songs, saved albums and playlists all show up the way you'd expect them to in the Spotify app. Search covers tracks, albums, artists and playlists.",
    accent: "text-spotify border-spotify/30",
  },
  ymusic: {
    tagline: "Sign in with the same Google account you use for YouTube Music.",
    setup:
      "Create a free Google Cloud OAuth client (the same kind of setup as for any third-party YouTube app), turn on the YouTube Data API, and add yourself as a test user. After that, signing in just opens a browser tab on your own machine.",
    library:
      "Your library is the videos and songs you have liked on YouTube Music. Playlists work the same way as on Spotify; YouTube Music does not have a separate “saved albums” surface.",
    accent: "text-ymusic border-ymusic/30",
  },
};

export default function Providers() {
  return (
    <section
      id="providers"
      className="border-t border-border bg-surface-alt py-20 md:py-28"
    >
      <div className="mx-auto max-w-6xl px-6">
        <h2 className="text-center text-3xl font-bold text-text-primary md:text-4xl">
          {sourceData.providers.length} services, one tool
        </h2>
        <p className="mx-auto mt-4 max-w-2xl text-center text-text-secondary">
          Pick which service you want to talk to with{" "}
          <InlineCode>--provider</InlineCode> &mdash; every command takes the
          same flag, and works the same way regardless of which one you choose.
        </p>

        <div className="mt-14 grid gap-6 md:grid-cols-2">
          {sourceData.providers.map((p) => {
            const blurb = providerBlurbs[p.slug];
            const accent = blurb?.accent ?? "text-text-primary border-border";
            return (
              <div
                key={p.slug}
                className={`rounded-xl border ${accent} bg-surface p-6`}
              >
                <h3 className={`mb-1 text-2xl font-bold ${accent.split(" ")[0]}`}>
                  {p.displayName}
                </h3>
                <p className="mb-4 text-sm text-text-secondary">
                  {blurb?.tagline ?? `Use --provider ${p.slug} to talk to ${p.displayName}.`}
                </p>

                <dl className="space-y-3 text-sm">
                  <div>
                    <dt className="font-semibold text-text-primary">Getting connected</dt>
                    <dd className="mt-1 text-text-secondary">{blurb?.setup}</dd>
                  </div>
                  <div>
                    <dt className="font-semibold text-text-primary">What spotifai sees</dt>
                    <dd className="mt-1 text-text-secondary">{blurb?.library}</dd>
                  </div>
                  <div>
                    <dt className="font-semibold text-text-primary">Pick this service with</dt>
                    <dd className="mt-1">
                      <InlineCode>--provider {p.slug}</InlineCode>
                    </dd>
                  </div>
                </dl>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
