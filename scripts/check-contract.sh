#!/usr/bin/env bash
# Type checks real Rust command results against the interface types.
set -euo pipefail
cd "$(dirname "$0")/.."
out="ui/contract.gen.ts"
trap 'rm -f "$out"' EXIT
KH_CONTRACT_OUT="$PWD/$out" cargo test -q -p kh-service contract_tests
test -s "$out"
pnpm exec tsc --noEmit --strict --target ES2022 --module ES2022 --lib ES2022,DOM "$out"
echo "Interface types match the command results."
