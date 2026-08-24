# Seyal Local Context Engine R&D

**Status:** Proposed  
**Issue:** #52  
**Dependency:** #48 / PR #60  
**Scope:** OSS local context architecture; no implementation.

## Decision

Build a provenance-first, inspectable Local Context Engine. Start with deterministic repository discovery + lexical/path/symbol ranking. Make semantic embeddings an optional measurable enhancement, not the storage foundation.

```text
Sources
 → discovery
 → normalized ContextItems + provenance
 → deterministic indexes
 → retrieve candidates
 → authority/security/freshness filters
 → rank + deduplicate
 → optional semantic/enhancement stage
 → token budget
 → ContextBundle + SelectionTrace
```

## Sources

Initial local sources:

- source files and repository structure;
- symbols and language metadata where a reliable parser/indexer exists;
- README/docs/ADRs/specs/AGENTS/CLAUDE-style instructions;
- git HEAD, branch, worktree, status and diff;
- retained local AgentRun discoveries/artifacts explicitly eligible for reuse;
- user-pinned context;
- project-local memory/context records;
- plugin-provided local context through the same provenance contract.

Source discovery does not execute content.

## Filesystem and repository-boundary semantics

Repository discovery must be explicit about state Git does not represent as ordinary tracked blobs.

### Untracked and ignored files

- Non-ignored untracked files **inside the current worktree scope are eligible local context** because newly created source/tests/config may be decisive before commit.
- Every included untracked file is worktree-scoped and fingerprinted from current bytes; it is never shared across worktrees merely by path.
- Git-ignored files/directories are **excluded from automatic discovery by default**. They are commonly generated, large, machine-local, or secret-bearing. A user/project policy may explicitly include a bounded ignored source, but normal sensitivity checks still apply.
- `.gitignore` is a discovery hint, not a security boundary. Secret/sensitivity policy is evaluated independently.
- Deletion/rename/untracked→tracked transitions invalidate affected discovery/index/selection state even when the resulting bytes are unchanged.

### Symlinks and path traversal

Discovery uses filesystem metadata before following a symlink. A symlink ContextItem records the link identity/path and resolved target scope.

- a symlink resolving outside the authorized workspace/repository source roots is excluded by default;
- `..`, nested symlinks and canonicalization tricks cannot expand source authorization;
- explicitly authorized external roots are separate ContextSources with their own provenance/sensitivity scope rather than an implicit symlink escape;
- changing the link target invalidates derivatives even if the link pathname is unchanged;
- directory symlink cycles are detected/bounded and never create unbounded traversal.

### Submodules and nested repositories

A Git submodule is a **separate repository identity/revision authority**. The parent repository records the gitlink/submodule path + pinned commit; traversal into the checked-out submodule creates ContextItems scoped to the submodule repository/worktree identity, including its own dirty/untracked state.

Do not flatten parent and submodule provenance into one revision. Nested independent repositories follow the same rule. Cross-repository retrieval may combine eligible items in a ContextBundle, but each item retains its own repository/revision/authority lineage.

## ContextItem

A normalized item carries:

```text
ContextItemId
content_ref/content_hash
source_kind + source_locator
repo/worktree identity
source revision/fingerprint
authority class
sensitivity class
trust/provenance
derivation lineage?       # absent for source truth
created/observed metadata
freshness dependencies
```

Derived summaries/chunks/embeddings keep the source hashes and derivation configuration that produced them. They are disposable and rebuildable.

## Storage/index recommendation

### Baseline

Use a small local metadata store with transactional semantics plus content-addressed derived blobs. SQLite is the leading implementation candidate because it provides a durable embedded store and FTS5 full-text search/BM25 ranking without adding a daemon. Official FTS5 documentation: https://sqlite.org/fts5.html

Repository content itself remains the source of truth; do not copy the entire repository into an authoritative database.

For syntax/symbol enrichment, prefer incremental language-aware indexers only where measured value justifies their maintenance. Tree-sitter is a candidate because its parse trees can be incrementally updated, but the Context Engine contract must not require Tree-sitter for every language.

### Rejected: vector database as the starting architecture

A dedicated vector DB adds another service/storage format, indexing cost, privacy surface and invalidation problem before retrieval benefit is proven. It is not needed for exact filenames, symbols, ADRs, git diffs or instruction precedence.

Embeddings remain an optional `SemanticIndexProvider` keyed by source hash + model/version. If the evaluation corpus shows material recall/precision improvement, the implementation can choose an embedded vector index or another bounded provider later.

### Rejected: LLM-only context selection

Sending a broad repository inventory to an LLM to decide what context to retrieve is costly, difficult to reproduce and creates circular context dependence. Model-assisted reranking/compaction is optional after deterministic candidate generation.

## Ranking

Ranking is staged, not one opaque score:

1. permission/sensitivity scope filter;
2. authoritative instruction/source precedence;
3. exact path/file/symbol/reference matches;
4. lexical relevance (BM25-style);
5. task/recent-diff/worktree proximity;
6. optional semantic rerank;
7. diversity/deduplication;
8. token-budget selection.

An ADR or project instruction does not lose authority because a semantically similar source file scored higher. Authority and relevance are separate dimensions.

`SelectionTrace` records included/excluded candidates and the major reason/score components so users and evaluators can inspect the bundle.

### SelectionTrace sensitivity

Explainability must not create a second secret store.

For excluded `secret`/policy-denied candidates, the trace persists only the minimum policy-safe metadata required to explain exclusion, for example a redacted/stable candidate reference, source kind, policy rule and exclusion reason. It must not persist source bytes/snippets, embeddings, semantic excerpts, secret-derived summaries, or values that reconstruct the excluded content.

If a path/locator itself is sensitive under source policy, the persisted trace uses a redacted or one-way scoped identifier and exposes the full locator only to an authorized live inspector. Trace retention/clear semantics follow the sensitivity policy of the highest-sensitivity metadata it contains.

## Freshness and invalidation

Use dependency fingerprints, not elapsed wall-clock time:

```text
source bytes/hash changes
 → invalidate source-derived chunks
 → symbol/index rows
 → embeddings
 → summaries
 → retrieval/selection results that referenced them
 → ContextBundles
```

Other invalidators include:

- HEAD/tree change for revision-scoped metadata;
- dirty/untracked-file content hash or lifecycle change;
- symlink target/scope change;
- submodule gitlink, checked-out revision or dirty-state change;
- branch/worktree diff fingerprint change;
- instruction/authority file change;
- indexer/summarizer/embedding model or configuration version change;
- sensitivity/permission policy change.

Invalidation is dependency-driven. A file timestamp alone is insufficient.

## Cross-worktree behavior

Share immutable content-derived data by content hash. Never share stateful selection results merely because two worktrees come from the same repository.

```text
safe to share: identical eligible source blob → chunk/symbol/embedding derived from that blob

must be worktree-scoped: dirty/untracked state, diff, branch, symlink scope,
                    selection, bundle, user-pinned context
```

Submodule/nested-repository derivatives are shareable only when both repository identity and immutable content/policy scope permit it.

This preserves cache efficiency without leaking one agent's uncommitted assumptions into another run.

## Conflict and precedence

Context can contain disagreement. Do not silently merge conflicting claims.

- Preserve each source and provenance.
- Apply explicit project authority order for normative instructions.
- For factual derived memory, mark conflict when two currently eligible sources disagree and neither has authority to supersede the other.
- A newer derived summary does not supersede an older authoritative ADR merely because it is newer.
- User-pinned context is always visible in the manifest but cannot silently override security policy.

## Enhancement/compaction

Baseline deterministic compaction includes structural excerpts, symbol bodies, diff hunks and bounded surrounding context.

Optional local/BYOK model enhancement may summarize or rank after candidate retrieval. Its output must be:

- labeled derived;
- fingerprinted by model/config/prompt version;
- linked to source ContextItems;
- invalidated when dependencies change;
- removable without losing source truth.

## Evaluation corpus

Create retained tasks covering:

- exact symbol/file lookup;
- architecture question requiring ADR precedence;
- implementation task where dirty diff is decisive;
- newly created non-ignored untracked source required for the answer;
- ignored generated/secret-like file excluded unless explicitly authorized;
- symlink attempting to escape workspace scope;
- submodule revision/dirty-state divergence from parent repo;
- stale summary after source modification;
- conflicting documentation;
- large repository with common ambiguous symbol names;
- secret-like file that must not enter a bundle or leak through SelectionTrace;
- two worktrees with divergent uncommitted changes.

Measure retrieval precision/recall at K, authoritative-source inclusion, stale-item rate, bundle tokens, retrieval latency, and user/evaluator relevance. Compare deterministic baseline vs semantic enhancement; do not report a semantic win without the same corpus/budget.

## Privacy/security

- Context content never receives execution authority.
- Sensitivity filtering happens before model/provider transmission.
- Credential/secret material defaults to excluded from persistent derived caches and external bundles.
- Git ignore state is not treated as sufficient secret classification.
- Symlinks cannot expand authorized source roots implicitly.
- Plugins get explicit source scope, not unrestricted repository access by default.
- Provenance survives compaction so prompt injection from repository text cannot masquerade as a Seyal/system instruction.
- Explainability traces are sensitivity-aware and cannot retain excluded secret content as debugging metadata.

## Performance

Index/retrieval work runs in bounded background pools and can be cancelled/throttled. It cannot synchronously gate terminal parsing/rendering. Large repository indexing is incremental; active task retrieval may read source directly when an index is stale rather than blocking for a full rebuild.

Filesystem traversal is bounded against symlink cycles, ignored build trees and nested-repository explosions.

## OSS/commercial boundary

**OSS:** complete local pipeline, deterministic retrieval, optional local/BYOK semantic provider, provenance, freshness, token budgets, inspection and local memory.  
**Commercial:** permissioned synchronized team/org context, organization relevance learning, cross-user reuse and managed indexing services.

## Success / kill criteria

The OSS baseline passes when retained evaluation tasks can deterministically explain why context was selected without leaking excluded secret material, stale/unauthorized items are excluded, dirty/untracked worktrees remain isolated, symlink/submodule boundaries preserve authority, and optional semantic retrieval can be disabled without breaking correctness.

Reject any design that requires a vector service, cloud model, commercial account, full-repository prompt, implicit filesystem-scope expansion, or terminal hot-path callback for ordinary local operation.

## ADR/spec before implementation

- Local Context ownership/provenance/invalidation ADR after cache R&D #53 aligns fingerprints.
- ContextItem/ContextBundle/SelectionTrace behavior spec including filesystem/submodule/symlink/untracked semantics.
- Local index benchmark and privacy/threat specification.
