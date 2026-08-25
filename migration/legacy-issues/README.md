# Legacy issue migration

This directory is the durable migration ledger for repositories that predate Seyal.

## Sources

- `mahboobmonnamd/RILL`
- `mahboobmonnamd/terminal`

The legacy repositories may eventually be deleted. No source issue is considered safely migrated until its title/body, source identity, state classification, and comments have been preserved in Seyal and the reconciliation manifest maps the source issue to its Seyal issue.

## Authority rule

Migration preserves knowledge; it does **not** promote legacy architecture into current Seyal authority.

Every migrated issue is classified as historical unless a current Seyal ADR/spec/R&D decision independently accepts the capability. In particular, old libghostty, daemon, renderer, Block, or runtime implementation choices never override the current Seyal constitution.

Rejected/wontfix/deferred issues are migrated too. A preserved rejection prevents future agents from rediscovering and silently reviving an already-considered direction.

## Files

- `rill-issues.json` — exported RILL issue records.
- `terminal-issues.json` — exported terminal issue records.
- `issue-map.json` — source issue to Seyal issue mapping and reconciliation state.
- `migrate_issues.py` — reproducible export/import/reconcile helper for a GitHub token with access to the source and destination repositories.

## Required fields

Each exported issue record retains at least:

- source repository;
- source issue number and URL;
- title and full Markdown body;
- open/closed state and state reason where available;
- labels, assignees and milestone names where available;
- created/updated/closed timestamps where available;
- comments with author, timestamp and body;
- migration classification.

GitHub does not allow an imported issue to retain its original issue number or server-side `created_at`. Those values are therefore preserved explicitly as source metadata.

## Reconciliation gates

A legacy repository is **not safe to delete** until all of these are true:

1. exported source issue count equals the independently enumerated source issue count;
2. every exported issue has exactly one destination mapping;
3. all comments in the export are represented in the migrated issue;
4. all 216 RILL `F-*` inventory rows are present in the product feature registry or explicitly classified as rejected/deferred/historical;
5. terminal feature-bearing issues have been reconciled against the same product registry;
6. source duplicates remain preserved at the issue-history layer but are deduplicated at the product-feature layer;
7. a final audit finds no unmapped source issue.

Do not delete RILL or terminal merely because the importer completed without an HTTP error.