#!/usr/bin/env bash
# Proves the privacy check blocks what it should. Every bad value is built at run time,
# so this file passes the check itself.
set -euo pipefail

source_root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=Tester GIT_COMMITTER_NAME=Tester
good_email="1+tester@users.noreply.github.com"
export GIT_AUTHOR_EMAIL=$good_email GIT_COMMITTER_EMAIL=$good_email
failures=0

cd "$work"
git init -q -b main .
mkdir -p scripts tests/fixtures
cp -R "$source_root/.githooks" "$source_root/.gitleaks.toml" .
cp "$source_root/scripts/privacy-check.sh" scripts/
git config core.hooksPath .githooks
git add -A
git commit -q -m "Start"

reset() {
  git reset -q --hard HEAD
  git clean -qfd
}

blocked() {
  local name=$1 file=$2 content=$3
  mkdir -p "$(dirname "$file")"
  printf '%b\n' "$content" >"$file"
  git add -f "$file"
  if git commit -q -m "try" >/dev/null 2>&1; then
    echo "NOT BLOCKED: $name"
    failures=$((failures + 1))
    git reset -q --hard HEAD~1
  fi
  reset
}

at="@"
blocked "access key" a.txt "aws_access_key_id = AKIA""Z7Q4MNB2XKD5PLRT"
blocked "private key" a.txt "-----BEGIN RSA PRIVATE ""KEY-----\n$(head -c 600 /dev/urandom | base64)\n-----END RSA PRIVATE ""KEY-----"
blocked "email" a.txt "write to jane.doe${at}mailprovider.com"
blocked "mac home path" a.txt "/Us""ers/janedoe/file"
blocked "linux home path" a.txt "/ho""me/janedoe/file"
blocked "windows home path" a.txt 'C:\\\\Us''ers\\\\janedoe\\\\file'
blocked "private link" a.txt "https://claude.""ai/code/session_abc"
blocked "private address" a.txt "host 192.168"".1.20"
blocked "phone" a.txt "call +256 ""700 123 456"
blocked "inline allow comment" a.txt "jane${at}mailprovider.com # gitleaks:allow"
blocked "env file" .env "A=1"
blocked "database" x.sqlite "text"
blocked "ignore file" .gitleaksignore "abc"
blocked "data file outside fixtures" people.csv "name"
blocked "binary file" blob.bin 'a\0b'

mkdir -p tests/fixtures
printf 'hello\n' >ok.txt
printf 'a,b\n' >tests/fixtures/ok.csv
git add -A
git commit -q -m "A normal change" || { echo "WRONGLY BLOCKED: normal change"; failures=$((failures + 1)); }

printf 'x\n' >>ok.txt
if git commit -qam "mail jane${at}mailprovider.com" >/dev/null 2>&1; then
  echo "NOT BLOCKED: email in message"
  failures=$((failures + 1))
fi
reset
printf 'x\n' >>ok.txt
if GIT_AUTHOR_EMAIL="jane${at}mailprovider.com" git commit -qam "author" >/dev/null 2>&1; then
  echo "NOT BLOCKED: personal author email"
  failures=$((failures + 1))
fi
reset
mkdir -p .git/info
echo 'secretclient' >.git/info/private-denylist
blocked "private word" a.txt "built for SecretClient"
rm .git/info/private-denylist

base=$(git rev-parse HEAD)
scripts/privacy-check.sh history HEAD || { echo "WRONGLY BLOCKED: clean history"; failures=$((failures + 1)); }
echo "jane${at}mailprovider.com" >skipped.txt
git add skipped.txt
git commit -q --no-verify -m "skipped hooks"
git rm -q skipped.txt
git commit -q -m "remove it again"
if scripts/privacy-check.sh history HEAD "^$base" >/dev/null 2>&1; then
  echo "NOT BLOCKED: removed later but still in history"
  failures=$((failures + 1))
fi
git init -q --bare "$work/remote.git"
if git push -q "$work/remote.git" main >/dev/null 2>&1; then
  echo "NOT BLOCKED: push of skipped commit"
  failures=$((failures + 1))
fi

[ "$failures" -eq 0 ] || { echo "$failures privacy checks failed"; exit 1; }
echo "privacy check blocks every planted case"
