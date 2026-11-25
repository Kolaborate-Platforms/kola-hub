#!/usr/bin/env bash
# Stops secrets and personal information from entering the repository.
#   privacy-check.sh staged               what is about to be committed
#   privacy-check.sh message <file>       a commit message
#   privacy-check.sh history <rev>...     every commit reached by the given git revisions
set -euo pipefail

root=$(git rev-parse --show-toplevel)
config="$root/.gitleaks.toml"
private_list="$(git rev-parse --git-common-dir)/info/private-denylist"
allowed_email='(@users\.noreply\.github\.com|^noreply@(anthropic|github)\.com)$'
forbidden_path='(^|/)(\.env(\..*)?|\.npmrc|\.netrc|\.git-credentials|\.gitleaksignore|\.DS_Store|CLAUDE\.local\.md|credentials\.json|recent-projects\.json|id_(rsa|dsa|ecdsa|ed25519)[^/]*|[^/]*\.(pem|key|p12|pfx|ppk|jks|keystore|kdbx|sqlite|sqlite3|db|sqlite-wal|sqlite-shm|log|khbackup|xlsx|xls|docx|pdf|zip))$|(^|/)(\.kh-brief|\.claude|\.kola-hub|docs/design)/'
binary_allowed='^(crates/kh-app|spikes/tauri-grid/src-tauri)/icons/'
data_allowed='^tests/fixtures/'
failed=0

fail() {
  echo "privacy check: $*" >&2
  failed=1
}

command -v gitleaks >/dev/null || {
  echo "privacy check: gitleaks is not installed, so nothing can be checked. See CONTRIBUTING.md." >&2
  exit 1
}

leaks() {
  gitleaks "$@" --config "$config" --redact --no-banner --ignore-gitleaks-allow --log-level error --verbose >&2
}

check_text() {
  local label=$1 text=$2
  printf '%s\n' "$text" | leaks stdin || fail "secret or personal information in $label"
  check_private "$label" "$text"
}

check_private() {
  [ -f "$private_list" ] || return 0
  local patterns
  patterns=$(grep -vE '^[[:space:]]*(#|$)' "$private_list" || true)
  [ -n "$patterns" ] || return 0
  # The matched text is not printed, so the private word does not end up in a CI or terminal log.
  if grep -qiE -f <(printf '%s\n' "$patterns") <<<"$2"; then
    fail "a word from your private list appears in $1"
  fi
}

check_email() {
  grep -qE "$allowed_email" <<<"$1" || fail "$2 email is not a no-reply address (see CONTRIBUTING.md)"
}

check_paths() {
  local path
  while IFS= read -r path; do
    [ -n "$path" ] || continue
    if [[ $path =~ $forbidden_path && ! $path =~ (^|/)\.env\.example$ ]]; then
      fail "this file must never be committed: $path"
    fi
    if [[ $path == *.csv && ! $path =~ $data_allowed ]]; then
      fail "data files belong in tests/fixtures with invented data only: $path"
    fi
  done <<<"$1"
}

check_binaries() {
  local added removed path
  while IFS=$'\t' read -r added removed path; do
    if [ "$added" = "-" ] && [[ ! $path =~ $binary_allowed ]]; then
      fail "binary files cannot be checked for personal information: $path"
    fi
  done <<<"$1"
}

# Git must show real content whatever the local settings or attributes say.
plain_git() {
  git -c core.quotePath=false "$@"
}

check_staged() {
  local ident
  for ident in GIT_AUTHOR_IDENT GIT_COMMITTER_IDENT; do
    check_email "$(git var "$ident" | sed -E 's/.*<([^>]*)>.*/\1/')" "your commit"
  done
  check_paths "$(plain_git diff --cached --no-renames --diff-filter=AM --name-only)"
  check_binaries "$(plain_git diff --cached --no-renames --diff-filter=AM --numstat)"
  leaks git --pre-commit --staged "$root" || fail "secret or personal information in the staged changes"
  check_private "the staged changes" "$(plain_git diff --cached --no-ext-diff --no-textconv)"
}

check_history() {
  [ "$#" -gt 0 ] || { fail "history needs at least one revision"; return; }
  local count email
  count=$(git rev-list --count "$@")
  [ "$count" -gt 0 ] || return 0
  while IFS= read -r email; do
    check_email "$email" "a commit author or committer"
  done < <(git log --format='%ae%n%ce' "$@" | sort -u)
  check_paths "$(plain_git log -m --no-renames --diff-filter=AM --name-only --format= "$@" | sort -u)"
  check_binaries "$(plain_git log -m --no-renames --diff-filter=AM --numstat --format= "$@" | sed '/^$/d' | sort -u)"
  leaks git --log-opts="-m $*" "$root" || fail "secret or personal information in the commits"
  check_text "commit messages or names" "$(git log --format='%an%n%cn%n%B' "$@")"
  check_private "the commits" "$(plain_git log -m -p --no-ext-diff --no-textconv --format= "$@")"
}

mode=${1:-}
[ "$#" -gt 0 ] && shift
case "$mode" in
  staged) check_staged ;;
  message) check_text "the commit message" "$(grep -v '^#' "$1" || true)" ;;
  text) check_text "$1" "$2" ;;
  history) check_history "$@" ;;
  *) echo "usage: privacy-check.sh staged | message <file> | text <label> <text> | history <rev>..." >&2; exit 2 ;;
esac

if [ "$failed" -ne 0 ]; then
  echo "privacy check failed. Fix the cause. Do not skip the check." >&2
  exit 1
fi
