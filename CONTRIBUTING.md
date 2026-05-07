# Contributing to spotifai

Thanks for your interest! This document describes how to set up a dev
environment, the conventions we follow, and how to get a change merged.

## Prerequisites

- Rust stable (≥ 1.78) — install via [rustup](https://rustup.rs/)
- A Spotify developer app with a `http://localhost:8888/callback` redirect URI (see [Getting started](docs/getting-started.md))
- `SPOTIFY_CLIENT_ID` and `SPOTIFY_CLIENT_SECRET` exported in your shell (needed for integration tests)

## Getting the source

```sh
git clone https://github.com/niclaslindstedt/spotifai.git
cd spotifai
```

## Build, test, lint

```sh
make build
make test
make lint
make fmt-check
```

## Development workflow

1. Fork the repo.
2. Create a topic branch: `git checkout -b feat/<slug>` or `fix/<slug>`.
3. Make focused commits using [Conventional Commits](https://www.conventionalcommits.org/):
   ```
   <type>(<scope>): <summary>
   ```
   Types: `feat`, `fix`, `perf`, `docs`, `test`, `refactor`, `chore`, `ci`,
   `build`, `style`. Breaking changes: `<type>!:` or `BREAKING CHANGE:` footer.
4. Open a PR. The **PR title** must be conventional-commit format because we
   squash-merge and that title becomes the commit message on `main`.
5. CI must be green and at least one reviewer must approve.

## Tests

Tests live in `tests/` as separate files (no `#[cfg(test)]` blocks in source). File names use the `_test` or `_tests` suffix. Run the full suite with:

```sh
make test
```

To run a single test:

```sh
cargo test <test_name>
```

Integration tests that hit the Spotify API are gated behind the `integration` feature flag and require valid credentials in the environment.

## Documentation

If your change touches user-visible behavior, update the relevant `docs/`
topic and the README quick start. See `AGENTS.md` for the full sync table.

## Code of Conduct

By participating you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

## Reporting security issues

See [SECURITY.md](SECURITY.md). Do **not** open public issues for security
problems.