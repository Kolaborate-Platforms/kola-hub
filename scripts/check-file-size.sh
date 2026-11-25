#!/usr/bin/env bash
# Fails if any Rust source file is over 1,500 lines (spec 11.1).
# This counts all lines in the file, which is stricter than "production lines".
set -euo pipefail
cd "$(dirname "$0")/.."
limit=1500
bad=0
while IFS= read -r f; do
  n=$(wc -l < "$f")
  if [ "$n" -gt "$limit" ]; then
    echo "TOO LONG: $f has $n lines (limit $limit)"
    bad=1
  fi
done < <(find crates -name '*.rs' -not -path '*/target/*')
exit $bad
