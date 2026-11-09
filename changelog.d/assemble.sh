#!/usr/bin/env bash
# Validates (--check) or prints, grouped by section, the changelog fragments
# in changelog.d/. See changelog.d/README.md.
set -euo pipefail
shopt -s nullglob

cd "$(dirname "$0")"

sections="upstream added changed fixed removed"
pattern='^[0-9]+\.(upstream|added|changed|fixed|removed)(\.[0-9]+)?\.md$'
status=0

for file in *.md; do
  [ "$file" = README.md ] && continue

  if ! [[ $file =~ $pattern ]]; then
    echo "changelog.d/$file: name must be <number>.<section>[.<k>].md, section one of: $sections" >&2
    status=1
    continue
  fi

  if ! grep -q '^- \*\*' "$file"; then
    echo "changelog.d/$file: needs a '- **Bold lead (#N)** — ...' bullet" >&2
    status=1
  fi

  if ! grep -q 'Signex:' "$file"; then
    echo "changelog.d/$file: needs a 'Signex:' sentence" >&2
    status=1
  fi
done

if [ "${1:-}" = --check ] || [ "$status" -ne 0 ]; then
  exit "$status"
fi

for section in $sections; do
  files=(*."$section".md *."$section".[0-9]*.md)
  [ "${#files[@]}" -eq 0 ] && continue

  case "$section" in
    upstream) echo "## Upstream PRs merged" ;;
    *) echo "### $(tr '[:lower:]' '[:upper:]' <<<"${section:0:1}")${section:1}" ;;
  esac
  echo

  printf '%s\n' "${files[@]}" | sort -n | while read -r file; do
    cat "$file"
  done
  echo
done
