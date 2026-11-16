#!/bin/sh
# Structure rules that the compiler cannot check.
# Each rule prints the lines that break it, and the script exits with 1.
set -u
fail=0

# 1. The CLI does not depend on Slint or on any other GUI crate.
if cargo tree -p inkland-cli --edges normal,build --prefix none | grep -Ei '^(slint|i-slint|winit|femtovg|skia)' ; then
  echo "guard: inkland-cli depends on a GUI crate" >&2
  fail=1
fi

# 2. The pure crates do no file I/O and read no environment and no clock.
if grep -rnE 'std::(fs|env|time|net|process)\b|SystemTime|Instant::now' \
     crates/inkland-core/src crates/inkland-constraints/src ; then
  echo "guard: a pure crate uses I/O, the environment, or the clock" >&2
  fail=1
fi

exit $fail
