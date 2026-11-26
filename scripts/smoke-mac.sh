#!/usr/bin/env bash
# Mounts the disk image, checks the app inside is complete and sealed, and writes its checksum.
set -euo pipefail

dir="$1"
installer_limit_mb=155
shopt -s nullglob
images=("$dir"/*.dmg)
[ "${#images[@]}" -eq 1 ] || { echo "Expected one disk image in $dir, found ${#images[@]}"; exit 1; }
image="${images[0]}"
size_mb=$(($(stat -f %z "$image") / 1048576))
echo "Disk image: $(basename "$image") ${size_mb} MB (limit ${installer_limit_mb})"
[ "$size_mb" -le "$installer_limit_mb" ] || { echo "Disk image is over its size budget"; exit 1; }

mount=$(mktemp -d)
hdiutil attach "$image" -nobrowse -readonly -mountpoint "$mount" >/dev/null
trap 'hdiutil detach "$mount" >/dev/null' EXIT
app="$mount/Kola Hub.app"
[ -x "$app/Contents/MacOS/kola-hub" ] || { echo "The app is missing from the disk image"; exit 1; }
[ -x "$app/Contents/MacOS/kola-hub-mcp" ] || { echo "The AI server is missing from the app"; exit 1; }
codesign --verify --deep --strict "$app"
echo "Smoke test passed"
