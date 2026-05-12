import { useState } from "react";
import InlineCode from "./InlineCode";
import UpstreamLink from "./UpstreamLink";
import { sourceData } from "../generated/sourceData";

const installMethods = [
  {
    title: "From crates.io",
    command: `cargo install ${sourceData.name}`,
    note: `Requires the Rust toolchain (rustc ${sourceData.rustVersion}+)`,
  },
  {
    title: "From source",
    command: `git clone ${sourceData.repository}\ncd ${sourceData.name} && make build`,
    note: `Build from latest source (also requires the Rust toolchain)`,
  },
  {
    title: "GitHub Releases",
    command: `# Download a pre-built binary from\n# ${sourceData.repository}/releases`,
    note: "Pre-built binaries per platform",
  },
];

const prereqs = [
  {
    name: "Spotify developer app",
    cmd: "https://developer.spotify.com/dashboard  (note your Client ID, add http://127.0.0.1)",
  },
  {
    name: "Google Cloud OAuth (YouTube Music)",
    cmd: "https://console.cloud.google.com/  (Desktop client + YouTube Data API v3)",
  },
];

function CopyCommand({ command }: { command: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative">
      <button
        onClick={handleCopy}
        className="absolute top-2 right-2 z-10 p-1.5 rounded text-text-dim hover:text-text-primary transition-colors cursor-pointer"
        aria-label="Copy command"
      >
        {copied ? (
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
          </svg>
        ) : (
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75" />
          </svg>
        )}
      </button>
      <pre className="whitespace-pre-wrap break-all rounded-lg bg-surface p-3 pr-10 text-xs leading-relaxed text-accent">
        <code>{command}</code>
      </pre>
    </div>
  );
}

export default function GettingStarted() {
  return (
    <section
      id="install"
      className="border-t border-border bg-surface py-20 md:py-28"
    >
      <div className="mx-auto max-w-5xl px-6">
        <h2 className="text-center text-3xl font-bold text-text-primary md:text-4xl">
          Install spotifai
        </h2>
        <p className="mx-auto mt-4 max-w-xl text-center text-text-secondary">
          Pick one of the install options below, complete the one-time
          prerequisites for the services you want to talk to, and then run the
          three-line setup at the bottom.
        </p>

        <div className="mt-12">
          <h3 className="mb-4 text-center text-sm font-semibold uppercase tracking-wider text-text-dim">
            1. Install the binary
          </h3>
          <div className="grid gap-6 md:grid-cols-3">
            {installMethods.map((m) => (
              <div key={m.title} className="rounded-xl border border-border bg-surface-alt p-5">
                <h3 className="mb-1 text-sm font-semibold text-text-primary">{m.title}</h3>
                <p className="mb-3 text-xs text-text-dim">{m.note}</p>
                <CopyCommand command={m.command} />
              </div>
            ))}
          </div>
        </div>

        <div className="mt-12">
          <h3 className="mb-4 text-center text-sm font-semibold uppercase tracking-wider text-text-dim">
            2. One-time prerequisites
          </h3>
          <div className="mx-auto max-w-2xl space-y-2">
            {prereqs.map((p) => (
              <div
                key={p.name}
                className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between rounded-lg border border-border bg-surface-alt px-4 py-2.5"
              >
                <span className="text-sm font-medium text-text-secondary">{p.name}</span>
                <code className="break-all text-xs text-text-dim">{p.cmd}</code>
              </div>
            ))}
          </div>
        </div>

        <div className="mx-auto mt-12 max-w-2xl">
          <h3 className="mb-4 text-center text-sm font-semibold uppercase tracking-wider text-text-dim">
            3. Set up and ask your first question
          </h3>
          <div className="rounded-xl border border-border bg-surface-alt p-6">
            <p className="mb-3 text-sm text-text-secondary">
              <InlineCode>spotifai install</InlineCode> creates the
              permission files in your home directory.{" "}
              <InlineCode>spotifai auth</InlineCode> opens a browser tab so
              you can sign in. After that, you can talk to your library.
            </p>
            <pre className="overflow-x-auto rounded-lg bg-surface p-4 text-sm leading-relaxed text-text-secondary">
              <code>
                <span className="text-accent">$</span> spotifai install{"\n"}
                <span className="text-accent">$</span> spotifai auth{"\n"}
                <span className="text-accent">$</span> spotifai ask "What are my most recently added albums?"
              </code>
            </pre>
          </div>
        </div>

        <p className="mt-10 text-center text-xs text-text-dim">
          Powered by <UpstreamLink name="zag">zag {sourceData.zagVersion}</UpstreamLink> (the AI runtime) and{" "}
          <UpstreamLink name="zad">zad {sourceData.zadVersion}</UpstreamLink> (the music-service wrapper for agentic use).
          Released under the {sourceData.license} license.
        </p>
      </div>
    </section>
  );
}
