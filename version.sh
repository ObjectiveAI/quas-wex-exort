#!/usr/bin/env bash
#
# Bump the quas-wex-exort version everywhere it appears, in sync.
#
#   ./version.sh <new-version>     e.g.  ./version.sh 0.2.0
#
# Updates the three places the version lives:
#   - Cargo.toml        [package] version
#   - Cargo.lock        the quas-wex-exort package entry
#   - objectiveai.json  the plugin manifest version
#
# (The MCP server's initialize-response version comes from CARGO_PKG_VERSION via
# Implementation::from_build_env(), so there's no literal to update.)
#
# Pure sed, no compile. Does NOT commit — committing the bump is what triggers
# the release workflow (which fires on Cargo.toml changes to main). Requires
# GNU sed (git-bash on Windows, or Linux).
set -euo pipefail

new="${1:-}"
if [[ -z "$new" ]]; then
  echo "usage: $0 <new-version>" >&2
  exit 1
fi
if [[ ! "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]]; then
  echo "error: '$new' is not a valid version (expected X.Y.Z)" >&2
  exit 1
fi

cd "$(dirname "$0")"

# Cargo.toml — the [package] version is the first `version = "..."` line.
sed -i -E '0,/^version = "[^"]*"/ s//version = "'"$new"'"/' Cargo.toml

# Cargo.lock — the `version` line directly after the package's name line.
sed -i -E '/^name = "quas-wex-exort"$/{n;s/^version = "[^"]*"/version = "'"$new"'"/}' Cargo.lock

# Plugin manifest.
sed -i -E 's/"version": "[^"]*"/"version": "'"$new"'"/' objectiveai.json

echo "Bumped to $new in: Cargo.toml, Cargo.lock, objectiveai.json"
