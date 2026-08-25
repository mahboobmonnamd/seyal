#!/usr/bin/env bash
set -euo pipefail

# Import every exported RILL/terminal issue into Seyal using the local gh login.
#
# This is intentionally separate from export so evidence can be inspected first.
# The importer is idempotent: each destination issue and comment carries a stable
# legacy-source marker, so rerunning the script skips already-imported records.
#
# Usage:
#   bash scripts/export-feature-source-issues.sh
#   bash scripts/import-feature-source-issues.sh --apply
#
# Optional:
#   bash scripts/import-feature-source-issues.sh --apply /tmp/seyal-feature-sources
#   DEST_REPO=owner/repo bash scripts/import-feature-source-issues.sh --apply

DEST_REPO="${DEST_REPO:-mahboobmonnamd/seyal}"
SOURCE_DIR=".feature-sources"
APPLY=false

if [[ "${1:-}" == "--apply" ]]; then
  APPLY=true
  shift
fi

if [[ $# -gt 0 ]]; then
  SOURCE_DIR="$1"
  shift
fi

if [[ $# -gt 0 ]]; then
  echo "error: unexpected arguments: $*" >&2
  exit 2
fi

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

for file in "$SOURCE_DIR/rill-issues.json" "$SOURCE_DIR/terminal-issues.json"; do
  [[ -f "$file" ]] || {
    echo "error: missing $file; run scripts/export-feature-source-issues.sh first" >&2
    exit 1
  }
done

if [[ "$APPLY" != true ]]; then
  rill_count="$(jq 'length' "$SOURCE_DIR/rill-issues.json")"
  terminal_count="$(jq 'length' "$SOURCE_DIR/terminal-issues.json")"
  echo "Dry run only. Would import $rill_count RILL issues + $terminal_count terminal issues into $DEST_REPO."
  echo "Run with --apply to create/update destination issues:"
  echo "  bash scripts/import-feature-source-issues.sh --apply $SOURCE_DIR"
  exit 0
fi

gh label create legacy-rill --repo "$DEST_REPO" --description "Historical issue imported from mahboobmonnamd/RILL" --color BFD4F2 --force >/dev/null
gh label create legacy-terminal --repo "$DEST_REPO" --description "Historical issue imported from mahboobmonnamd/terminal" --color D4C5F9 --force >/dev/null
gh label create historical-evidence --repo "$DEST_REPO" --description "Preserved historical evidence; not current implementation authority" --color EDEDED --force >/dev/null

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
DEST_CACHE="$TMP_DIR/destination-issues.json"

gh issue list --repo "$DEST_REPO" --state all --limit 10000 --json number,body,state > "$DEST_CACHE"

find_existing_issue() {
  local marker="$1"
  jq -r --arg marker "$marker" '.[] | select((.body // "") | contains($marker)) | .number' "$DEST_CACHE" | head -n 1
}

cache_issue() {
  local number="$1" body="$2" state="$3" next="$TMP_DIR/cache-next.json"
  jq --argjson number "$number" --arg body "$body" --arg state "$state" '. + [{number:$number,body:$body,state:$state}]' "$DEST_CACHE" > "$next"
  mv "$next" "$DEST_CACHE"
}

sync_issue_state() {
  local dest_number="$1" source_state="$2" source_reason="$3"
  if [[ "$source_state" == "CLOSED" ]]; then
    if [[ "$source_reason" == "NOT_PLANNED" || "$source_reason" == "DUPLICATE" ]]; then
      gh issue close "$dest_number" --repo "$DEST_REPO" --reason "not planned" >/dev/null 2>&1 || true
    else
      gh issue close "$dest_number" --repo "$DEST_REPO" --reason completed >/dev/null 2>&1 || true
    fi
  else
    gh issue reopen "$dest_number" --repo "$DEST_REPO" >/dev/null 2>&1 || true
  fi
}

import_comments() {
  local dest_number="$1" issue_json="$2" existing_comments="$TMP_DIR/comments-$1.json"
  gh api --paginate "repos/$DEST_REPO/issues/$dest_number/comments?per_page=100" --jq '.[] | {body: .body}' > "$existing_comments.jsonl"
  if [[ -s "$existing_comments.jsonl" ]]; then jq -s '.' "$existing_comments.jsonl" > "$existing_comments"; else printf '[]\n' > "$existing_comments"; fi

  local comment_count
  comment_count="$(jq '.comments | length' <<<"$issue_json")"
  for ((i=0; i<comment_count; i++)); do
    local comment_json comment_url marker existing author created body payload
    comment_json="$(jq -c --argjson i "$i" '.comments[$i]' <<<"$issue_json")"
    comment_url="$(jq -r '.url // empty' <<<"$comment_json")"
    created="$(jq -r '.createdAt // "unknown"' <<<"$comment_json")"
    author="$(jq -r '.author.login // "unknown"' <<<"$comment_json")"
    body="$(jq -r '.body // ""' <<<"$comment_json")"
    if [[ -n "$comment_url" ]]; then marker="Legacy-Comment: $comment_url"; else marker="Legacy-Comment: $author@$created#$i"; fi
    existing="$(jq -r --arg marker "$marker" '.[] | select((.body // "") | contains($marker)) | .body' "$existing_comments" | head -n 1)"
    [[ -n "$existing" ]] && continue
    payload="$(cat <<EOF
<!-- seyal-legacy-comment -->
**Legacy comment metadata**

- $marker
- Original author: @$author
- Original timestamp: $created

---

$body
EOF
)"
    gh issue comment "$dest_number" --repo "$DEST_REPO" --body "$payload" >/dev/null
    jq --arg body "$payload" '. + [{body:$body}]' "$existing_comments" > "$existing_comments.next"
    mv "$existing_comments.next" "$existing_comments"
  done
}

import_file() {
  local source_repo="$1" source_file="$2" source_label="$3" total
  total="$(jq 'length' "$source_file")"
  echo "Importing $total issues from $source_repo ..."

  for ((index=0; index<total; index++)); do
    local issue_json number title original_body url state state_reason author created updated closed labels assignees milestone marker existing dest_number dest_url destination_body
    issue_json="$(jq -c --argjson i "$index" '.[$i]' "$source_file")"
    number="$(jq -r '.number' <<<"$issue_json")"
    title="$(jq -r '.title' <<<"$issue_json")"
    original_body="$(jq -r '.body // ""' <<<"$issue_json")"
    url="$(jq -r '.url' <<<"$issue_json")"
    state="$(jq -r '.state // "OPEN"' <<<"$issue_json")"
    state_reason="$(jq -r '.stateReason // ""' <<<"$issue_json")"
    author="$(jq -r '.author.login // "unknown"' <<<"$issue_json")"
    created="$(jq -r '.createdAt // "unknown"' <<<"$issue_json")"
    updated="$(jq -r '.updatedAt // "unknown"' <<<"$issue_json")"
    closed="$(jq -r '.closedAt // "—"' <<<"$issue_json")"
    labels="$(jq '[.labels[]?.name] | if length == 0 then "—" else join(", ") end' -r <<<"$issue_json")"
    assignees="$(jq '[.assignees[]?.login] | if length == 0 then "—" else join(", ") end' -r <<<"$issue_json")"
    milestone="$(jq -r '.milestone.title // "—"' <<<"$issue_json")"
    marker="Legacy-Source: $source_repo#$number"
    existing="$(find_existing_issue "$marker")"

    destination_body="$(cat <<EOF
<!-- seyal-legacy-source -->
> Historical evidence imported during Seyal feature consolidation. Legacy completion/closure does **not** mean this capability is implemented or accepted in current Seyal.

### Legacy source metadata

- $marker
- Original issue: $url
- Original author: @$author
- Original state: $state
- Original state reason: ${state_reason:-—}
- Original created: $created
- Original updated: $updated
- Original closed: $closed
- Original labels: $labels
- Original assignees: $assignees
- Original milestone: $milestone

---

$original_body
EOF
)"

    if [[ -n "$existing" ]]; then
      dest_number="$existing"
      echo "  skip existing $source_repo#$number -> $DEST_REPO#$dest_number"
    else
      dest_url="$(gh issue create --repo "$DEST_REPO" --title "$title" --body "$destination_body" --label historical-evidence --label "$source_label")"
      dest_number="${dest_url##*/}"
      cache_issue "$dest_number" "$destination_body" "OPEN"
      echo "  created $source_repo#$number -> $DEST_REPO#$dest_number"
    fi
    import_comments "$dest_number" "$issue_json"
    sync_issue_state "$dest_number" "$state" "$state_reason"
  done
}

import_file "mahboobmonnamd/RILL" "$SOURCE_DIR/rill-issues.json" legacy-rill
import_file "mahboobmonnamd/terminal" "$SOURCE_DIR/terminal-issues.json" legacy-terminal

FINAL_CACHE="$TMP_DIR/final-destination-issues.json"
gh issue list --repo "$DEST_REPO" --state all --limit 10000 --json number,body > "$FINAL_CACHE"
rill_expected="$(jq 'length' "$SOURCE_DIR/rill-issues.json")"
terminal_expected="$(jq 'length' "$SOURCE_DIR/terminal-issues.json")"
rill_imported="$(jq '[.[] | select((.body // "") | contains("Legacy-Source: mahboobmonnamd/RILL#"))] | length' "$FINAL_CACHE")"
terminal_imported="$(jq '[.[] | select((.body // "") | contains("Legacy-Source: mahboobmonnamd/terminal#"))] | length' "$FINAL_CACHE")"

if [[ "$rill_imported" != "$rill_expected" || "$terminal_imported" != "$terminal_expected" ]]; then
  echo "error: import reconciliation failed" >&2
  echo "  RILL: expected=$rill_expected imported=$rill_imported" >&2
  echo "  terminal: expected=$terminal_expected imported=$terminal_imported" >&2
  echo "Rerun the same command; source markers make it safe to resume." >&2
  exit 1
fi

echo "Import reconciled successfully:"
echo "  RILL: $rill_imported/$rill_expected"
echo "  terminal: $terminal_imported/$terminal_expected"
echo "Rerunning is safe: source issue/comment markers prevent duplicates."
echo "If GitHub rate-limits the large write burst, rerun later; completed imports are skipped."
echo "Do not delete legacy repositories yet; Issues/comments are preserved here, but PR diffs and Git commit objects still need a separate archive."
