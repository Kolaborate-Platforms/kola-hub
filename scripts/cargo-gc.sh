#!/usr/bin/env bash
# Free disk space used by Rust build artifacts.
# Usage: scripts/cargo-gc.sh [--all]
#   (default)  remove incremental caches and debug-only artifacts, keep release builds
#   --all      remove the whole target directory
set -euo pipefail

cd "$(dirname "$0")/.."

free_gb() { df -Pk . | awk 'NR==2 {printf "%.1f", $4/1024/1024}'; }

before=$(free_gb)
echo "Free disk before: ${before} GB"

if [ "${1:-}" = "--all" ]; then
  cargo clean
else
  rm -rf target/debug/incremental target/debug/deps target/debug/build target/debug/.fingerprint target/debug/examples
  find target -name '*.rlib' -path '*debug*' -delete 2>/dev/null || true
fi

after=$(free_gb)
echo "Free disk after:  ${after} GB"
awk -v a="$after" 'BEGIN { if (a < 10) print "WARNING: free disk is under 10 GB." }'
