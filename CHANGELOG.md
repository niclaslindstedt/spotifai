# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is **auto-generated from conventional commits at release time** —
do not edit manually.

## [Unreleased]

### Added

- YouTube Music as an alternative backing provider (zad ≥ 0.6.0).
  Pick the backend with `--provider <slug>` on `auth`, `ask`, `playlist`,
  and `export`; default stays `spotify`. The provider abstraction in
  `src/providers.rs` is sized so additional backends drop in by adding
  one enum variant + one default-policy helper.

### Changed

- Bumped pinned zad version from `v0.4.0` to `v0.6.0`.
- Permission profile files moved from
  `~/.spotifai/permissions/<profile>.toml` to
  `~/.spotifai/permissions/<provider>/<profile>.toml`. Existing
  installs need to re-run `spotifai install` to scaffold the new
  layout. Previously hand-edited files are not migrated automatically.
- `spotifai api` now reads `SPOTIFAI_PROVIDER` (default: `spotify`)
  alongside `SPOTIFAI_PROFILE` and forwards to the matching
  `zad <provider>` subcommand.
- `spotifai install` now scaffolds and signs four files
  (Spotify × ask/playlist + YouTube Music × ask/playlist) instead of
  two.
- System prompts bumped to `1.1.0` with new `{{ provider_name }}` and
  `{{ provider_examples }}` placeholders; `1.0.x` versions are kept on
  disk per the immutable-prompts policy.

