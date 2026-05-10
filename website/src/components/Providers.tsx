import { sourceData } from "../generated/sourceData";

// Each provider gets its own brand color and a short blurb. The
// blurbs are editorial — they explain the OAuth flow shape and the
// verb-surface differences that aren't obvious from the allowlists
// alone.
const providerBlurbs: Record<string, { tagline: string; oauth: string; verbShape: string; accent: string }> = {
  spotify: {
    tagline: "OAuth 2.0 PKCE — public client, no client secret.",
    oauth:
      "spotifai auth opens a per-session self-signed HTTPS loopback listener and runs the PKCE flow against the Spotify Web API. Pin http://127.0.0.1 as a redirect host in the developer dashboard once.",
    verbShape:
      "Library splits into library tracks ... and library albums ...; saved albums get their own surface. Search defaults to track + album + artist + playlist.",
    accent: "text-spotify border-spotify/30",
  },
  ymusic: {
    tagline: "Google OAuth 2.0 Desktop client — needs a client_secret.",
    oauth:
      "spotifai auth --provider ymusic uses Google's Desktop-app HTTP loopback flow (zad >= 0.6.0). Enable the YouTube Data API v3 in your Cloud project and add your account to the consent-screen test users.",
    verbShape:
      "Library is a single library list over rated videos (no saved-albums concept). Library writes are library like / library unlike. Playlists support a --title flag instead of --name.",
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
          {sourceData.providers.length} providers, one CLI surface
        </h2>
        <p className="mx-auto mt-4 max-w-2xl text-center text-text-secondary">
          Pick a backend with{" "}
          <code className="rounded bg-surface px-1.5 py-0.5 text-xs text-accent">--provider</code>
          {" "}— every command respects the flag. Adding a third provider is a single new variant in
          {" "}<code className="rounded bg-surface px-1.5 py-0.5 text-xs text-accent">src/providers.rs</code>.
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
                <div className="mb-1 text-xs font-medium uppercase tracking-wider text-text-dim">
                  {p.zadSubcommand} subcommand
                </div>
                <h3 className={`mb-1 text-2xl font-bold ${accent.split(" ")[0]}`}>
                  {p.displayName}
                </h3>
                <p className="mb-4 text-sm text-text-secondary">
                  {blurb?.tagline ?? `Backed by zad ${p.zadSubcommand}.`}
                </p>

                <dl className="space-y-3 text-sm">
                  <div>
                    <dt className="font-semibold text-text-primary">Authentication</dt>
                    <dd className="mt-1 text-text-secondary">{blurb?.oauth}</dd>
                  </div>
                  <div>
                    <dt className="font-semibold text-text-primary">Verb surface</dt>
                    <dd className="mt-1 text-text-secondary">{blurb?.verbShape}</dd>
                  </div>
                  <div>
                    <dt className="font-semibold text-text-primary">CLI flag</dt>
                    <dd className="mt-1">
                      <code className="rounded bg-surface-alt px-1.5 py-0.5 text-xs text-accent">
                        --provider {p.slug}
                      </code>
                    </dd>
                  </div>
                </dl>
              </div>
            );
          })}
        </div>

        <p className="mx-auto mt-10 max-w-2xl text-center text-sm text-text-dim">
          Tidal, Apple Music, anything else zad ships a service for: one new variant + one match arm
          per provider method. The CLI surface picks it up automatically through clap's value-enum derive.
        </p>
      </div>
    </section>
  );
}
