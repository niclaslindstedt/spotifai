#!/usr/bin/env bash
# Update version strings in language-specific manifests to match the given tag.
set -euo pipefail

tag="${1:?usage: update-versions.sh <tag>}"
ver="${tag#v}"

if [ -f Cargo.toml ]; then
  sed -i.bak -E "s/^version = \".*\"/version = \"${ver}\"/" Cargo.toml && rm Cargo.toml.bak
fi
if [ -f package.json ]; then
  sed -i.bak -E "s/(\"version\": \")[^\"]*(\")/\1${ver}\2/" package.json && rm package.json.bak
fi
if [ -f pyproject.toml ]; then
  sed -i.bak -E "s/^version = \".*\"/version = \"${ver}\"/" pyproject.toml && rm pyproject.toml.bak
fi

# Regenerate Cargo.lock so the bumped local-package version is
# committed alongside Cargo.toml. Without this, `cargo publish`
# downstream would rewrite Cargo.lock as part of its internal build
# and then refuse to publish a dirty working tree. Best-effort: the
# release workflow installs the rust toolchain before calling this
# script, but local invocations may not have cargo available, hence
# the fallback chain. (Pattern borrowed from sibling repo `zag`.)
if [ -f Cargo.toml ] && command -v cargo >/dev/null 2>&1; then
  cargo generate-lockfile 2>/dev/null || cargo check 2>/dev/null || true
fi
