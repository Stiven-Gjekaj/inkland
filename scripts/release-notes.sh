#!/usr/bin/env bash
# Write the release notes of a tag in Markdown to stdout.
# The notes list the new config options and the commits since the tag before.
# Usage: scripts/release-notes.sh <tag> [previous tag]
set -euo pipefail

tag="$1"
prev="${2:-$(git describe --tags --abbrev=0 "$tag^" 2>/dev/null || true)}"

# Print one line "Type.field" for each option in a version of docs/CONFIG.md.
options() {
  git show "$1:docs/CONFIG.md" 2>/dev/null | awk '
    /^## Top level/ { type = "top"; next }
    /^### /         { type = $2; next }
    /^\| `[^`]+` \|/ {
      split($0, cell, "`")
      if (type == "top") print cell[2]; else print type "." cell[2]
    }' | sort -u
}

echo "## New options"
echo
if [ -n "$prev" ]; then
  added=$(comm -13 <(options "$prev") <(options "$tag"))
else
  added=$(options "$tag")
fi
if [ -z "$added" ]; then
  echo "None."
else
  while IFS= read -r line; do echo "- \`$line\`"; done <<< "$added"
fi
echo
echo "See [docs/CONFIG.md](https://github.com/Stiven-Gjekaj/inkland/blob/$tag/docs/CONFIG.md) for each option."

range="$tag"
[ -n "$prev" ] && range="$prev..$tag"

section() {
  local title="$1" prefix="$2" lines
  lines=$(git log --format=%s "$range" | grep -E "^$prefix(\(.*\))?: " | sed -E "s/^$prefix(\([^)]*\))?: //" || true)
  [ -z "$lines" ] && return 0
  echo
  echo "## $title"
  echo
  while IFS= read -r line; do echo "- $line"; done <<< "$lines"
}

section "Features" feat
section "Fixes" fix
section "Documentation" docs

if [ -n "$prev" ]; then
  echo
  echo "**Full Changelog**: https://github.com/Stiven-Gjekaj/inkland/compare/$prev...$tag"
fi
