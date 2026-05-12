import { useState } from "react";
import InlineCode from "./InlineCode";
import type { ProviderData, ProviderProfile } from "../generated/sourceData";
import { sourceData } from "../generated/sourceData";

type ProfileKey = "ask" | "playlist";

const profileMeta: Record<
  ProfileKey,
  { label: string; command: string; tagline: string }
> = {
  ask: {
    label: "Reading your library",
    command: "spotifai ask",
    tagline:
      "Used when you ask questions about your music. The agent can search and look around, but it cannot change anything you have saved.",
  },
  playlist: {
    label: "Curating one new playlist",
    command: "spotifai playlist",
    tagline:
      "Used when you ask spotifai to build a playlist for you. It can create one new playlist and add tracks to it — but it cannot delete or modify any playlist you already have.",
  },
};

function profileFor(provider: ProviderData, key: ProfileKey): ProviderProfile | null {
  return key === "ask" ? provider.ask : provider.playlist;
}

export default function Permissions() {
  const [provider, setProvider] = useState(sourceData.providers[0]?.slug ?? "");
  const [profile, setProfile] = useState<ProfileKey>("ask");

  const activeProvider =
    sourceData.providers.find((p) => p.slug === provider) ?? sourceData.providers[0];
  const activeProfile = activeProvider ? profileFor(activeProvider, profile) : null;
  const meta = profileMeta[profile];

  return (
    <section
      id="permissions"
      className="border-t border-border py-20 md:py-28"
    >
      <div className="mx-auto max-w-5xl px-6">
        <h2 className="text-center text-3xl font-bold text-text-primary md:text-4xl">
          An AI that only does what you allow
        </h2>
        <p className="mx-auto mt-4 max-w-3xl text-center text-text-secondary">
          A normal Spotify API token — or an MCP server pointed at your account — hands
          the AI the keys to everything: it can read, edit, and delete anything on your
          account. spotifai is different. Each command comes with its own short list of
          actions, and the agent literally cannot do anything outside that list, no matter
          what you (or it) ask.
        </p>

        <div className="mt-12 overflow-hidden rounded-xl border border-border bg-surface-alt shadow-2xl">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-surface px-5 py-3">
            <div className="flex flex-wrap gap-1">
              {sourceData.providers.map((p) => (
                <button
                  key={p.slug}
                  onClick={() => setProvider(p.slug)}
                  className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
                    p.slug === provider
                      ? "bg-surface-alt text-accent"
                      : "text-text-dim hover:text-text-secondary"
                  }`}
                >
                  {p.displayName}
                </button>
              ))}
            </div>
            <div className="flex flex-wrap gap-1">
              {(Object.keys(profileMeta) as ProfileKey[]).map((key) => (
                <button
                  key={key}
                  onClick={() => setProfile(key)}
                  className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
                    key === profile
                      ? "bg-accent/10 text-accent"
                      : "text-text-dim hover:text-text-secondary"
                  }`}
                >
                  {profileMeta[key].label}
                </button>
              ))}
            </div>
          </div>

          <div className="grid grid-cols-1 gap-6 p-6 md:grid-cols-2">
            <div>
              <div className="mb-3 flex flex-wrap items-center gap-2 text-sm">
                <InlineCode>{meta.command}</InlineCode>
                <span className="text-text-dim">on</span>
                <span className="font-semibold text-text-primary">
                  {activeProvider?.displayName}
                </span>
              </div>
              <p className="text-sm leading-relaxed text-text-secondary">{meta.tagline}</p>
              {activeProfile?.description && (
                <p className="mt-3 text-sm italic leading-relaxed text-text-dim">
                  &ldquo;{activeProfile.description}&rdquo;
                </p>
              )}
            </div>

            <div className="grid grid-cols-1 gap-4 text-sm">
              <VerbList
                heading="Can do"
                tone="accent"
                verbs={activeProfile?.allowed ?? []}
              />
              <VerbList
                heading="Cannot do"
                tone="dim"
                verbs={activeProfile?.denied ?? []}
              />
            </div>
          </div>
        </div>

        <div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-3">
          {[
            {
              num: "1",
              title: "Smaller blast radius than an API key",
              body: "An API token (or an MCP server using one) lets the AI do anything your account can do. spotifai narrows that to a handful of safe actions per command — there is simply no command that can wipe your library.",
            },
            {
              num: "2",
              title: "Tamper-evident by design",
              body: "Your permission lists are sealed when you install spotifai. If anything — including the agent itself — tries to silently widen them on disk, the next run refuses to start until you re-approve.",
            },
            {
              num: "3",
              title: "Nothing lives in the cloud",
              body: "Login tokens are stored in your operating system's keychain, the same place your other apps keep passwords. No long-lived secret sits on a server somewhere waiting to leak.",
            },
          ].map((step) => (
            <div
              key={step.num}
              className="rounded-xl border border-border bg-surface p-5"
            >
              <div className="mb-2 inline-flex h-7 w-7 items-center justify-center rounded-full bg-accent/10 text-sm font-semibold text-accent">
                {step.num}
              </div>
              <h3 className="text-sm font-semibold text-text-primary">{step.title}</h3>
              <p className="mt-1 text-xs leading-relaxed text-text-dim">{step.body}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function VerbList({
  heading,
  tone,
  verbs,
}: {
  heading: string;
  tone: "accent" | "dim";
  verbs: string[];
}) {
  if (!verbs.length) return null;
  const headingClass =
    tone === "accent"
      ? "text-accent"
      : "text-text-dim";
  const dotClass =
    tone === "accent" ? "bg-accent" : "bg-text-dim/60";
  return (
    <div>
      <div className={`mb-2 text-xs font-semibold uppercase tracking-wider ${headingClass}`}>
        {heading} ({verbs.length})
      </div>
      <ul className="space-y-1">
        {verbs.map((v) => (
          <li key={v} className="flex items-center gap-2 text-text-secondary">
            <span className={`h-1.5 w-1.5 rounded-full ${dotClass}`} />
            <code className="text-xs">{v}</code>
          </li>
        ))}
      </ul>
    </div>
  );
}
