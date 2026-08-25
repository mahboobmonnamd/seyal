# Legacy issue migration

This directory is the durable migration ledger for repositories that predate Seyal.

## Sources

- `mahboobmonnamd/RILL`
- `mahboobmonnamd/terminal`

The migration population is **every GitHub Issue in both repositories, regardless of open/closed/completed/not-planned/duplicate state**. Pull requests are a different GitHub object type and are not recreated as Issues.

No source issue is considered safely migrated until its title/body, source identity, historical state classification, and comments have been preserved in Seyal and the reconciliation manifest maps the source issue to its Seyal issue.

## Critical implementation-status rule

A legacy issue being `closed` or `completed` means only that it was closed in RILL/terminal. **It is not evidence that the capability exists in current Seyal.** Seyal restarted implementation on a new architecture.

During feature backfill, every capability discovered in a legacy issue must be re-evaluated against current Seyal and classified as one of:

- accepted direction;
- foundation exists;
- implemented in current Seyal;
- deferred/rejected/superseded;
- historical-only;
- unresolved and requiring an explicit Seyal decision.

Likewise, rejected/wontfix/not-planned/deferred issues are preserved. Historical rejection is valuable decision evidence and must not be silently revived.

## Authority rule

Migration preserves knowledge; it does **not** promote legacy architecture into current Seyal authority.

Every migrated issue is classified as historical unless a current Seyal ADR/spec/R&D decision independently accepts the capability. In particular, old libghostty, daemon, renderer, Block, or runtime implementation choices never override the current Seyal constitution.

## Files

- `rill-issues.json` — exported RILL issue records.
- `terminal-issues.json` — exported terminal issue records.
- `issue-map.json` — source issue to Seyal issue mapping and reconciliation state.
- `migrate_issues.py` — reproducible export/import/reconcile helper for a GitHub token with access to the source and destination repositories.

## Required fields

Each exported issue record retains at least:

- source repository;
- source issue number and URL;
- original author where available;
- title and full Markdown body;
- open/closed state and state reason where available;
- labels, assignees and milestone names where available;
- created/updated/closed timestamps where available;
- comments with original comment id, author, timestamp and body;
- migration classification.

GitHub does not allow an imported issue to retain its original issue number or server-side `created_at`. Those values are therefore preserved explicitly as source metadata.

## Feature-backfill disposition gate

Every exported legacy issue must receive exactly one traceable disposition in the historical feature review:

1. maps to a current Seyal feature/capability;
2. duplicate of another mapped feature;
3. rejected/deferred/superseded historical idea;
4. implementation defect/test/build issue with no distinct product capability;
5. unresolved item requiring an explicit Seyal product/architecture decision.

No issue may be skipped because it is closed.

## Repository retirement rule

**Archive first; do not delete by default.**

Issue recreation alone is not a lossless replacement for a GitHub repository. Legacy issue bodies can reference old PRs, commits, branches, files, ADRs, screenshots and review discussions. Deleting the repository can destroy or invalidate that context even when every Issue was recreated.

RILL and terminal should therefore be made read-only/archived after the issue + feature reconciliation is complete. Permanent deletion is allowed only after a separate full-repository preservation check proves that Git history, PR/review history and referenced artifacts that still matter have been retained elsewhere.

## Reconciliation gates

A legacy repository is **not safe even to archive as retired** until all of these are true:

1. export used GitHub `state=all`, covering open and closed Issues;
2. exported source issue count equals the independently enumerated source issue count;
3. every exported issue has exactly one destination mapping;
4. every source comment is individually represented in the migrated issue;
5. original issue state/state reason remains preserved as historical metadata;
6. all 216 RILL `F-*` inventory rows are present in the product feature registry or explicitly classified as rejected/deferred/historical;
7. every other RILL issue has a feature-backfill disposition;
8. every terminal issue has a feature-backfill disposition;
9. source duplicates remain preserved at the issue-history layer but are deduplicated at the product-feature layer;
10. a final audit finds no unmapped source issue and no undisposed source issue.

Do not delete RILL or terminal merely because the importer completed without an HTTP error.