import { useState } from "react";
import type { ProviderData, ProviderProfile } from "../generated/sourceData";
import { sourceData } from "../generated/sourceData";

type ProfileKey = "ask" | "playlist";

const profileMeta: Record<ProfileKey, { title: string; verb: string; tagline: string }> = {
  ask: {
    title: "ask.toml",
    verb: "spotifai ask",
    tagline:
      "Read-only profile, injected into spotifai ask. Search the catalogue, list playlists, walk the library. No writes, no deletes.",
  },
  playlist: {
    title: "playlist.toml",
    verb: "spotifai playlist",
    tagline:
      "Adds three curator verbs to the read-only baseline. The agent can create one new playlist, populate it, and rename it — but never delete or modify your saved library.",
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
          The agent only uses verbs you signed off on
        </h2>
        <p className="mx-auto mt-4 max-w-3xl text-center text-text-secondary">
          spotifai ships two profiles per provider under{" "}
          <code className="rounded bg-surface-alt px-1.5 py-0.5 text-xs text-accent">
            ~/.spotifai/permissions/&lt;provider&gt;/
          </code>
          . They are signed at install time with a per-machine Ed25519 key in the OS keychain;
          zad fails closed at load time on any unsigned change.
        </p>

        <div className="mt-12 overflow-hidden rounded-xl border border-border bg-surface-alt shadow-2xl">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-surface px-5 py-3">
            <div className="flex gap-1">
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
            <div className="flex gap-1">
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
                  {profileMeta[key].title}
                </button>
              ))}
            </div>
          </div>

          <div className="grid grid-cols-1 gap-6 p-6 md:grid-cols-2">
            <div>
              <div className="mb-3 flex items-center gap-2 text-sm">
                <span className="font-semibold text-text-primary">{meta.verb}</span>
                <span className="text-text-dim">&middot;</span>
                <span className="text-text-dim">
                  {activeProvider?.displayName} &middot; {meta.title}
                </span>
              </div>
              <p className="text-sm leading-relaxed text-text-secondary">{meta.tagline}</p>
              {activeProfile?.description && (
                <p className="mt-3 text-sm italic leading-relaxed text-text-dim">
                  &ldquo;{activeProfile.description}&rdquo;
                </p>
              )}
              {activeProfile?.mode && (
                <p className="mt-3 text-xs text-text-dim">
                  mode = <code className="text-accent">{activeProfile.mode.toLowerCase()}</code>
                </p>
              )}
            </div>

            <div className="grid grid-cols-1 gap-4 text-sm">
              <VerbList
                heading="allowed"
                tone="accent"
                verbs={activeProfile?.allowed ?? []}
              />
              <VerbList
                heading="denied"
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
              title: "Mint signing key",
              body: "spotifai install bootstraps a per-machine Ed25519 keypair in the OS keychain (account zad/signing:v1).",
            },
            {
              num: "2",
              title: "Scaffold profiles",
              body: "Writes ask.toml + playlist.toml under each provider directory. Existing hand-edits are preserved across re-runs.",
            },
            {
              num: "3",
              title: "Sign and trust",
              body: "Each file is signed and the signature lands in ~/.zad/signing/trusted.toml. zad's load-time check passes; the agent surfaces light up.",
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
