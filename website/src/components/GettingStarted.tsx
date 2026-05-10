import { sourceData } from "../generated/sourceData";

const installMethods = [
  {
    title: "From crates.io",
    command: `cargo install ${sourceData.name}`,
    note: `Requires Rust ${sourceData.rustVersion}+`,
  },
  {
    title: "From source",
    command: `git clone ${sourceData.repository}\ncd ${sourceData.name} && make build`,
    note: "Build from latest source",
  },
  {
    title: "GitHub Releases",
    command: `# Download a pre-built binary from\n# ${sourceData.repository}/releases`,
    note: "Pre-built binaries per platform",
  },
];

const prereqs = [
  {
    name: "Rust toolchain",
    cmd: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh",
  },
  {
    name: "Spotify developer app",
    cmd: "https://developer.spotify.com/dashboard  (note your Client ID, add http://127.0.0.1)",
  },
  {
    name: "Google Cloud OAuth (YouTube Music)",
    cmd: "https://console.cloud.google.com/  (Desktop client + YouTube Data API v3)",
  },
];

export default function GettingStarted() {
  return (
    <section
      id="install"
      className="border-t border-border bg-surface py-20 md:py-28"
    >
      <div className="mx-auto max-w-5xl px-6">
        <h2 className="text-center text-3xl font-bold text-text-primary md:text-4xl">
          Get started in two commands
        </h2>
        <p className="mx-auto mt-4 max-w-xl text-center text-text-secondary">
          Install the binary, walk the guided setup, authenticate, and ask your library a question.
        </p>

        <div className="mt-12 grid gap-6 md:grid-cols-3">
          {installMethods.map((m) => (
            <div key={m.title} className="rounded-xl border border-border bg-surface-alt p-5">
              <h3 className="mb-1 text-sm font-semibold text-text-primary">{m.title}</h3>
              <p className="mb-3 text-xs text-text-dim">{m.note}</p>
              <pre className="overflow-x-auto rounded-lg bg-surface p-3 text-xs leading-relaxed text-accent">
                <code>{m.command}</code>
              </pre>
            </div>
          ))}
        </div>

        <div className="mt-12">
          <h3 className="mb-4 text-center text-lg font-semibold text-text-primary">
            Prerequisites
          </h3>
          <div className="mx-auto max-w-2xl space-y-2">
            {prereqs.map((p) => (
              <div
                key={p.name}
                className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between rounded-lg border border-border bg-surface-alt px-4 py-2.5"
              >
                <span className="text-sm font-medium text-text-secondary">{p.name}</span>
                <code className="text-xs text-text-dim">{p.cmd}</code>
              </div>
            ))}
          </div>
        </div>

        <div className="mx-auto mt-12 max-w-2xl rounded-xl border border-border bg-surface-alt p-6">
          <p className="mb-3 text-center text-sm text-text-secondary">
            Then run the guided setup, authenticate, and ask your first question:
          </p>
          <pre className="overflow-x-auto rounded-lg bg-surface p-4 text-sm leading-relaxed text-text-secondary">
            <code>
              <span className="text-accent">$</span> spotifai install{"\n"}
              <span className="text-accent">$</span> spotifai auth{"\n"}
              <span className="text-accent">$</span> spotifai ask "What are my most-played albums?"
            </code>
          </pre>
        </div>

        <p className="mt-10 text-center text-xs text-text-dim">
          Powered by zag {sourceData.zagVersion} (LLM agent runtime) and zad {sourceData.zadVersion} (music-service client).
          Released under the {sourceData.license} license.
        </p>
      </div>
    </section>
  );
}
