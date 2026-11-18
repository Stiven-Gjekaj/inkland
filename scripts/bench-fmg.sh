#!/bin/sh
# Measure Azgaar's Fantasy Map Generator in headless Chrome, for the speed
# table of the README. The script starts Chrome with a new, empty profile,
# runs scripts/bench-fmg.mjs, and stops Chrome.
#
# Usage: scripts/bench-fmg.sh
# CHROME: the Chrome binary. Default: the macOS install location.
# Node 22 or newer runs the script, because it uses the WebSocket of Node.
set -eu
chrome=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
port=9333
profile=$(mktemp -d)
"$chrome" --headless=new --remote-debugging-port=$port --user-data-dir="$profile" \
  --window-size=1024,768 --no-first-run --no-default-browser-check about:blank \
  >/dev/null 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null; wait $pid 2>/dev/null; rm -rf "$profile"' EXIT
node "$(dirname "$0")/bench-fmg.mjs" $port
