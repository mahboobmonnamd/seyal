#!/usr/bin/env bash
set -euo pipefail

# Export every open/closed GitHub Issue from the legacy Seyal source repos.
# This is evidence collection for feature consolidation only. It does not create,
# modify, migrate, or close any GitHub Issue.
#
# Usage:
#   bash scripts/export-feature-source-issues.sh
#   bash scripts/export-feature-source-issues.sh /tmp/seyal-feature-sources

OUT_DIR="${1:-.feature-sources}"
mkdir -p "$OUT_DIR"

command -v gh >/dev/null 2>&1 || {
  echo "error: GitHub CLI (gh) is required" >&2
  exit 1
}

gh auth status >/dev/null 2>&1 || {
  echo "error: run 'gh auth login' first" >&2
  exit 1
}

export_repo() {
  local repo="$1"
  local name="$2"
  local out="$OUT_DIR/$name-issues.json"

  echo "Exporting $repo ..."
  gh issue list \
    --repo "$repo" \
    --state all \
    --limit 10000 \
    --json number,title,body,comments,state,stateReason,url,author,labels,assignees,milestone,createdAt,updatedAt,closedAt \
    > "$out"

  local expected
  local exported
  expected="$(gh api --method GET search/issues -f q="repo:$repo is:issue" --jq '.total_count')"
  exported="$(gh issue list --repo "$repo" --state all --limit 10000 --json number --jq 'length')"

  if [[ "$expected" != "$exported" ]]; then
    echo "error: $repo issue count mismatch: expected=$expected exported=$exported" >&2
    exit 1
  fi

  echo "  $exported issues -> $out"
}

export_repo "mahboobmonnamd/RILL" "rill"
export_repo "mahboobmonnamd/terminal" "terminal"

rill_inventory="$(gh issue list \
  --repo "mahboobmonnamd/RILL" \
  --state all \
  --limit 10000 \
  --label inventory \
  --json number \
  --jq 'length')"

if [[ "$rill_inventory" != "216" ]]; then
  echo "error: expected 216 RILL inventory rows, found $rill_inventory" >&2
  exit 1
fi

# One convenience file containing both complete exports.
{
  printf '{\n  "rill": '
  cat "$OUT_DIR/rill-issues.json"
  printf ',\n  "terminal": '
  cat "$OUT_DIR/terminal-issues.json"
  printf '\n}\n'
} > "$OUT_DIR/all-issues.json"

echo "Verified RILL inventory rows: 216"
echo "Combined export: $OUT_DIR/all-issues.json"
