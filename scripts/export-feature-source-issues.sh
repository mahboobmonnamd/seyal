#!/usr/bin/env bash
set -euo pipefail

# Export every open/closed GitHub Issue and every comment from the legacy Seyal
# source repositories. This script is read-only.
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

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is required" >&2
  exit 1
}

gh auth status >/dev/null 2>&1 || {
  echo "error: run 'gh auth login' first" >&2
  exit 1
}

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

export_repo() {
  local repo="$1"
  local name="$2"
  local raw="$TMP_DIR/$name-issues-raw.json"
  local rows="$TMP_DIR/$name-issues.ndjson"
  local out="$OUT_DIR/$name-issues.json"

  echo "Exporting $repo ..."

  gh issue list \
    --repo "$repo" \
    --state all \
    --limit 10000 \
    --json number,title,body,state,stateReason,url,author,labels,assignees,milestone,createdAt,updatedAt,closedAt \
    > "$raw"

  local expected exported
  expected="$(gh api --method GET search/issues -f q="repo:$repo is:issue" --jq '.total_count')"
  exported="$(jq 'length' "$raw")"

  if [[ "$expected" != "$exported" ]]; then
    echo "error: $repo issue count mismatch: expected=$expected exported=$exported" >&2
    exit 1
  fi

  : > "$rows"

  local index=0 total_comments=0
  while IFS= read -r issue_json; do
    index=$((index + 1))
    local number comments_json comment_count
    number="$(jq -r '.number' <<<"$issue_json")"

    # Fetch comments through the paginated REST endpoint instead of relying on a
    # GraphQL connection embedded by `gh issue list`, so long threads are complete.
    comments_json="$(
      gh api --paginate "repos/$repo/issues/$number/comments?per_page=100" \
        --jq '.[] | {id: .id, url: .html_url, body: (.body // ""), createdAt: .created_at, updatedAt: .updated_at, author: {login: (.user.login // "unknown")}}' \
      | jq -s '.'
    )"
    comment_count="$(jq 'length' <<<"$comments_json")"
    total_comments=$((total_comments + comment_count))

    jq -c --argjson comments "$comments_json" '. + {comments:$comments}' <<<"$issue_json" >> "$rows"

    if (( index % 25 == 0 || index == exported )); then
      echo "  exported $index/$exported issues ($total_comments comments)"
    fi
  done < <(jq -c '.[]' "$raw")

  jq -s '.' "$rows" > "$out"

  local final_count
  final_count="$(jq 'length' "$out")"
  if [[ "$final_count" != "$expected" ]]; then
    echo "error: $repo final export count mismatch: expected=$expected final=$final_count" >&2
    exit 1
  fi

  echo "  $final_count issues + $total_comments comments -> $out"
}

export_repo "mahboobmonnamd/RILL" "rill"
export_repo "mahboobmonnamd/terminal" "terminal"

# The RILL `inventory` label is also attached to the catalog epic (#33), so the
# label count is 217. The product catalog itself is the inventory issues whose
# title begins with an F-* feature id; that set must contain 216 unique rows.
rill_inventory_total="$(gh issue list \
  --repo "mahboobmonnamd/RILL" \
  --state all \
  --limit 10000 \
  --label inventory \
  --json number \
  --jq 'length')"

rill_feature_rows="$(gh issue list \
  --repo "mahboobmonnamd/RILL" \
  --state all \
  --limit 10000 \
  --label inventory \
  --json title \
  --jq '[.[] | select(.title | test("^F-[0-9]+ "))] | length')"

rill_unique_feature_ids="$(gh issue list \
  --repo "mahboobmonnamd/RILL" \
  --state all \
  --limit 10000 \
  --label inventory \
  --json title \
  --jq '[.[] | .title | select(test("^F-[0-9]+ ")) | split(" ")[0]] | unique | length')"

if [[ "$rill_feature_rows" != "216" || "$rill_unique_feature_ids" != "216" ]]; then
  echo "error: expected 216 unique RILL F-* feature rows, found rows=$rill_feature_rows unique_ids=$rill_unique_feature_ids" >&2
  exit 1
fi

# Convenience file containing both complete exports.
{
  printf '{\n  "rill": '
  cat "$OUT_DIR/rill-issues.json"
  printf ',\n  "terminal": '
  cat "$OUT_DIR/terminal-issues.json"
  printf '\n}\n'
} > "$OUT_DIR/all-issues.json"

echo "Verified RILL feature catalog: 216 unique F-* rows ($rill_inventory_total inventory-labeled issues including the catalog epic)"
echo "Combined export: $OUT_DIR/all-issues.json"
echo "Next: bash scripts/import-feature-source-issues.sh --apply $OUT_DIR"
