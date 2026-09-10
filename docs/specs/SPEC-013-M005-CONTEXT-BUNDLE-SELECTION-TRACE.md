# SPEC-013 — M005 ContextBundle, SelectionTrace and source invalidation

- **Status:** Proposed for acceptance
- **Issue:** #854
- **Architecture:** `docs/architecture/ADR-013-CONTEXT-DURABLE-MEMORY.md`
- **Parent refinement:** #838
- **Implementation consumer:** #681
- **Related:** #847 / SPEC-012 MemoryRecord lifecycle

## 1. Purpose and scope

Define the observable M005 contract for the Local Context Engine when it discovers eligible sources, constructs versioned/provenance-bound `ContextItem`s, selects and orders them into an immutable `ContextBundle`, records a policy-safe `SelectionTrace`, and invalidates stale derived context after source, worktree, document or policy changes.

This specification is subordinate to ADR-013. It does not create another source-of-truth, memory store, transcript authority or action/effect state machine.

It covers:

- source classes and provenance;
- `ContextItem` identity;
- eligibility/filtering;
- authority ordering versus relevance;
- deterministic retrieval/deduplication/conflict handling;
- optional semantic/model-assisted reranking;
- token-budget partitioning;
- immutable `ContextBundle` dependency tracking;
- `SelectionTrace` explainability;
- filesystem/worktree/repository/symlink/submodule provenance;
- LSP/symbol/index/overlay generations;
- source and derived-data invalidation;
- bounded failure/resource behavior;
- provider/model neutrality;
- terminal hot-path isolation.

## 2. Authority and ownership

The following authorities remain distinct:

```text
source authority
  code / instructions / ADRs / specs / git / worktree / typed external sources
        |
        v
Local Context Engine
        |
        +--> ContextItem(s)        derived, provenance-bound
        +--> ContextBundle         immutable per-build snapshot
        +--> SelectionTrace        policy-safe explanation

MemoryStore                     separate durable semantic-memory authority
RunWorkingSet                   separate derived run-context authority
AgentRun evidence               separate durable run-evidence authority
indexes/caches/embeddings       disposable derivatives
```

Requirements:

1. `ContextBundle` is not source truth and is not durable semantic memory.
2. `SelectionTrace` is not source truth, durable memory or a secret-retention store.
3. An index, embedding, summary, lexical cache or semantic cache is derived/rebuildable and cannot become source authority.
4. `MemoryRecord` may be consumed as an eligible source class, but this specification does not define its lifecycle; SPEC-012/ADR-013 own that behavior.
5. `RunWorkingSet` may be consumed as a source class but its retention/resumability contract is defined separately.
6. Provider/model continuation state is never source authority.

## 3. Canonical source classes

The Local Context Engine may consume typed sources including:

```text
NormativeInstruction
RepositoryFile
WorktreeFile
GitState
UserPinnedContext
MemoryRecordRef
AgentRunEvidenceRef
RunWorkingSetRef
ArtifactRef
LspDocumentResult
SymbolOrIndexResult
TypedExternalSource
```

Every source must expose sufficient provenance and scope to decide whether it is eligible for a specific build.

Raw terminal output, arbitrary shell text, provider narration or model statements are not silently promoted to source truth. They may enter context only through their actual evidence/source class with preserved provenance.

Source discovery is read/inspect work only. Discovery must never execute a discovered file, shell fragment, project script, editor hook or model-suggested command merely to determine relevance.

## 4. `ContextItem` contract

A `ContextItem` is a derived, versioned/provenance-bound representation of one eligible source contribution.

Conceptually it carries at least:

```text
ContextItemId
source_kind
source_identity
source_scope
source_version / generation
content_fingerprint
provenance_ref(s)
authority_class
sensitivity_class
eligibility metadata
selection unit / byte-range / symbol-range when applicable
estimated token/byte cost
builder/source-adapter version
```

The exact serialized schema is an implementation specification detail, but these semantics are mandatory.

A `ContextItemId` must not be reused for materially different source content or materially different provenance.

If an item represents a range of a larger source, its range is part of source identity. A later edit that shifts or changes the represented range invalidates that item unless the owning source contract can prove equivalent identity.

## 5. Scope eligibility

A context build is always bound to an explicit build scope containing at least the applicable user/project/repository/workspace/worktree/WorkItem/Attempt/AgentRun identities where available.

Eligibility is deny-by-default across unrelated scopes.

Requirements:

- a worktree-local dirty or untracked file cannot leak into a sibling worktree merely because paths match;
- repository-local content cannot automatically become eligible in another repository or submodule;
- user-local memory or pinned context is not implicitly injected into every project;
- shared immutable derivatives may be reused only when content identity and policy/scope eligibility both match;
- crossing repository/workspace roots requires explicit authorized source scope;
- a provider/model identity never merges scopes.

Scope checks run before ranking.

## 6. Eligibility and filtering order

For each candidate source, the engine applies deterministic eligibility gates before relevance ranking:

```text
source discovered
  -> scope authorization
  -> permission/policy eligibility
  -> sensitivity/privacy eligibility
  -> source version/freshness validation
  -> source-type validity
  -> eligible candidate
```

An ineligible item cannot become eligible because semantic similarity or model reranking scores it highly.

Failure to establish current scope, permission, required source version or sensitivity eligibility yields explicit exclusion/unavailable state rather than speculative inclusion.

Privacy/security use-time dispatch races are specified separately, but this build-time contract must record the policy/privacy generation required by ADR-013 so later use-time validation can detect staleness.

## 7. Authority and relevance are separate dimensions

The engine must not collapse authority and relevance into one opaque score.

At minimum, selection respects this precedence:

1. current security/capability restrictions;
2. current normative instructions and accepted architecture/spec authority;
3. current repository/worktree/source truth;
4. exact task/path/symbol/user-pinned references;
5. eligible scoped durable memory with provenance;
6. current run/evidence sources;
7. lexical/task/worktree relevance;
8. optional semantic rerank;
9. diversity/deduplication;
10. budget fit.

Within a lower authority class, relevance may affect ordering. A stale summary or MemoryRecord cannot outrank a conflicting current ADR/spec/source fact solely because it is more semantically similar or more recent.

An optional model/embedding system may help choose among already-eligible candidates but may not alter normative authority ordering or bypass policy/scope filters.

## 8. Deterministic retrieval baseline

Correctness must not depend on a model, embedding service, vector database or cloud provider.

The permanent implementation must support a deterministic baseline using provider-neutral information such as:

- exact path/symbol/task references;
- file/repository structure;
- lexical matching;
- source authority class;
- current worktree/repository state;
- explicit pins;
- source recency/version where semantically valid;
- bounded deterministic tie-breakers.

Optional semantic retrieval/reranking may improve quality but can be disabled without making eligibility, provenance or invalidation incorrect.

Given identical eligible inputs, policy, builder version and deterministic retrieval configuration, the deterministic baseline must produce reproducible candidate identity/order before optional nondeterministic enhancement.

## 9. Conflict handling and deduplication

Deduplication must preserve provenance and authority differences.

Two sources with identical or near-identical text are not automatically interchangeable if they have different authority, version, repository, worktree or sensitivity provenance.

Rules:

- byte/content duplicates within the same authority/scope/version may be coalesced into one selected payload while retaining all relevant provenance references;
- a current normative/source fact and a conflicting lower-authority memory/summary remain distinguishable;
- conflicting current source facts from different authorized roots are both retained or explicitly surfaced as conflict when the engine cannot establish a single authority winner;
- a model-generated summary cannot silently replace exact normative/source content when exact content is required for correctness;
- deduplication must never erase the fact that one item was excluded while another equivalent-looking item was eligible under a different scope/policy.

## 10. Filesystem and repository provenance

Repository/worktree source discovery must preserve real VCS/security boundaries.

Each filesystem-derived item records sufficient identity to distinguish at least:

```text
repository identity
worktree identity
path relative to authorized root
tracked/untracked/ignored status
content fingerprint or source version
symlink provenance where applicable
submodule/nested-repository provenance where applicable
```

### 10.1 Tracked files

Tracked files are eligible only from the authorized repository/worktree snapshot actually requested by the build. Working-tree modifications are distinct from committed-tree content.

### 10.2 Untracked files

Non-ignored untracked files inside the authorized worktree may be eligible. They are explicitly worktree-scoped and must not be reused by sibling worktrees based on path alone.

### 10.3 Ignored files

Ignored files are excluded from automatic discovery by default.

`.gitignore` is not a security boundary. Explicit user/source authorization may include an ignored file only after normal policy/sensitivity checks.

### 10.4 Symlinks

A symlink inside an authorized root must not silently authorize reading an external target.

Eligibility requires either:

- target resolves within an already-authorized source root; or
- the external target/root is explicitly authorized as a source.

The item records link-path and resolved-target provenance sufficient to invalidate when either changes.

### 10.5 Submodules and nested repositories

Submodules/nested repositories retain separate repository identity, revision and dirty-state provenance. Parent-repository authorization does not erase that identity.

A submodule revision update, nested-repository replacement or dirty-state change invalidates affected items/derivatives even when parent paths are unchanged.

## 11. Source-version and filesystem invalidation

A `ContextItem` or dependent bundle becomes stale when a material dependency changes.

Examples include:

- file content change;
- file delete/create/rename where identity/range changes;
- tracked ↔ untracked transition;
- worktree switch or replacement;
- repository HEAD/index/working-tree state changing where depended upon;
- symlink path or target changing;
- submodule revision/dirty state changing;
- authorized root/policy/sensitivity state changing;
- source adapter/version contract changing incompatibly.

A rename may preserve a higher-level logical identity only if the source adapter can prove it with accepted source authority; path equality/heuristics alone are not enough.

Invalidation marks derived items/bundles stale or removes their reuse eligibility. It never mutates external source truth.

## 12. LSP, symbol and language-index sources

LSP/symbol/index results are optional context sources, not source truth.

Every such result is bound to at least:

```text
workspace/worktree
path/document identity
document/source version
overlay identity/version when unsaved
language-server/index generation
query/result kind
```

Requirements:

- unsaved editor overlays are separate versioned sources from on-disk files;
- a late result for an old document/index generation is stale and cannot silently become current;
- a server restart/generation reset invalidates prior generation-bound results unless the adapter proves continuity;
- if an exact source read conflicts with stale index/LSP output, current source authority wins;
- no language server is required for context correctness;
- absence/failure of LSP degrades enrichment only and does not stall terminal execution.

## 13. `ContextBundle` contract

A `ContextBundle` is an immutable per-build snapshot of exactly what was selected for one consumer/use attempt.

Conceptually it records:

```text
ContextBundleId
WorkItemId / AttemptId / AgentRunId when applicable
build scope
ordered ContextItem refs/payloads
source fingerprints/versions
dependency set
MemoryRecord ids/versions when selected
RunWorkingSet dependency fingerprint when selected
policy version
privacy/revocation generation
builder version
selection configuration version
budget/estimated token cost
created_at
SelectionTraceId
```

Once created, a bundle is never silently edited in place.

If an item/dependency becomes stale or eligibility changes, the old bundle is marked stale/undispatchable as required by the applicable later use-time contract and a new bundle is built or explicitly revalidated.

A new bundle receives a new `ContextBundleId`.

## 14. Dependency completeness

A bundle's dependency set must be sufficient to invalidate it when any selected material or eligibility assumption changes.

Dependencies include selected source identities/versions plus policy/scope generations needed to establish eligibility.

A bundle that depends on a summary/index range must retain the authoritative dependency chain back to the source identity/version required to judge freshness.

An implementation may compress dependency representation, but it may not discard dependencies merely to reduce metadata if that can cause stale reuse.

A hash proves equality only for the material it hashes. It cannot prove permission, freshness, repository/worktree identity or semantic authority by itself.

## 15. Token-budget partitioning

Context construction is bounded by an explicit budget supplied by the consuming harness/adapter/capability contract.

The engine must reserve/partition budget so lower-authority bulk content cannot crowd out mandatory higher-authority instructions or exact task references.

A valid implementation may use configurable partitions, but behavior must satisfy:

- mandatory policy/instruction items are considered before optional bulk context;
- exact user/task references are protected from unrelated high-volume repository matches;
- no one source class may consume unbounded memory/CPU/token budget;
- oversized items are truncated/chunked only with explicit provenance/range identity;
- omission due to budget is recorded in `SelectionTrace`;
- budget exhaustion returns a valid bounded result or explicit unable-to-build state, never an unbounded retry loop.

Changing the effective budget/partition configuration is part of selection configuration identity and prevents reuse when it could change selected content.

## 16. `SelectionTrace` contract

Every completed build produces a policy-safe `SelectionTrace` sufficient to explain important selection behavior without retaining forbidden payload.

The trace may record:

```text
SelectionTraceId
ContextBundleId
candidate/source identifiers safe to persist
included/excluded decision
policy-safe reason code
authority class
important relevance/ranking components
conflict/dedup outcome
budget drop/truncation reason
source/dependency versions where policy-safe
builder/selection configuration version
```

Required reason classes include at least:

- included mandatory authority;
- included exact task/pin match;
- included relevant source;
- excluded scope;
- excluded permission/policy;
- excluded sensitivity/privacy;
- excluded stale version/generation;
- excluded duplicate/coalesced;
- excluded lower-authority conflict;
- excluded budget;
- source unavailable/error.

For excluded secret-bearing or denied content, traces must not persist raw snippets, embeddings, reversible hashes, paths/locators or summaries when those would reveal/reconstruct the excluded source.

Explainability cannot become a second retention path.

## 17. Optional semantic/model enhancement

Optional semantic retrieval, embeddings, model-assisted reranking or summarization operate only after deterministic policy/scope eligibility.

They are derived helpers with these constraints:

- no model/provider-specific durable core type;
- no authority widening;
- no inclusion of previously excluded content;
- no hidden rewrite of source provenance;
- deterministic fallback remains functional;
- failure/timeout/cancellation returns to deterministic selection or explicit degraded result;
- derived semantic data obeys source sensitivity/retention/invalidation policy;
- repeated model output cannot become source authority through ranking feedback.

## 18. Cache and index behavior

All retrieval/index caches are bounded and rebuildable.

Cache keys include sufficient identity to prevent stale/cross-scope reuse, including relevant combinations of:

```text
source identity/version
repository/worktree scope
policy/privacy generation
builder/index version
selection configuration
query/task fingerprint where applicable
```

Cache hits never bypass eligibility or use-time policy checks required by ADR-013.

On corruption, version mismatch, missing derivative or cache eviction, rebuild from still-authorized source authority. Do not reconstruct erased/private payload from metadata or another scope's cache.

Persistent indexes obey the same sensitivity/retention rules as their sources.

## 19. Failure and degraded behavior

Context build failures must be explicit and bounded.

Required behavior:

- unreadable/missing individual optional source → mark unavailable/excluded and continue when correctness permits;
- required normative/exact source unavailable → fail the build or mark it incomplete; do not silently substitute lower authority;
- stale LSP/index → ignore/rebuild asynchronously; source reads remain authoritative;
- semantic provider failure → deterministic fallback;
- cache corruption → invalidate/rebuild derivative;
- repeated filesystem/index/persistence failure → bounded retry/backoff with visible degraded state;
- budget exhaustion → bounded selection or explicit unable-to-build result;
- cancellation → stop background build work without corrupting source/memory authority.

No failure mode may spin indefinitely or cause resource growth without bound.

## 20. Security requirements

The implementation must protect against at least:

- prompt/context poisoning from untrusted repository content;
- content that attempts to masquerade as normative instructions;
- symlink escape from authorized roots;
- sibling worktree/repository leakage;
- stale submodule/nested-repository reuse;
- ignored/secret file accidental discovery;
- cache/index cross-scope poisoning;
- model reranker widening authority;
- stale LSP/document-generation injection;
- trace/log leakage of excluded secret content;
- path traversal/malformed source identifiers;
- unbounded source expansion/decompression/resource exhaustion.

Repository content may contain instructions for an agent, but discovery/ranking treats it as content at its actual authority level. It does not become a higher-priority Seyal/system instruction merely because its text asks to.

## 21. Terminal hot-path isolation

Context work is control/background-plane work.

None of the following may synchronously gate:

```text
PTY -> byte stream -> VT/parser -> TerminalState -> damage/projection -> Metal
```

- filesystem discovery;
- repository/status inspection;
- indexing/search;
- ContextBundle construction;
- SelectionTrace persistence;
- LSP/symbol queries;
- memory retrieval;
- semantic/model/embedding work;
- cache/persistence writes;
- invalidation/rebuild work.

Under sustained context/index failure or large-repository load, unrelated terminal I/O/rendering must continue within accepted terminal performance budgets.

## 22. Resource and performance requirements

Concrete production budgets are calibrated/refined before #681 becomes Ready, but the implementation must measure at least:

- cold and warm context-build latency for small/medium/large repositories;
- exact-path/symbol lookup latency;
- deterministic retrieval latency;
- cache hit/miss cost;
- filesystem/status discovery cost;
- invalidation/rebuild cost after localized and broad changes;
- concurrent independent bundle builds;
- CPU/RSS/disk growth under sustained indexing/retrieval;
- queue/backpressure behavior;
- LSP/semantic enhancement cost when enabled;
- repeated failure/backoff behavior;
- terminal latency/throughput isolation during active/failure load.

Background work must be bounded, cancellable and priority-aware.

## 23. Required deterministic tests

At minimum, production implementation must include tests for:

1. identical deterministic inputs produce stable pre-semantic candidate ordering;
2. source-scope exclusion occurs before ranking;
3. lower-authority semantic match cannot outrank conflicting current normative/source truth;
4. selected source edit invalidates dependent item/bundle;
5. unrelated source edit does not invalidate an independent bundle unnecessarily;
6. worktree-local untracked file never leaks to sibling worktree;
7. tracked/untracked transition invalidates prior provenance;
8. ignored file excluded by default;
9. explicitly authorized ignored file still receives policy/sensitivity filtering;
10. symlink outside authorized root is rejected unless external root is explicitly authorized;
11. symlink target change invalidates derived context;
12. submodule revision/dirty-state change invalidates affected item/bundle;
13. nested repository identity remains distinct;
14. unsaved LSP overlay and on-disk source remain distinct versions;
15. late LSP/index generation is rejected as stale;
16. LSP failure falls back without changing source authority;
17. exact duplicate coalescing preserves provenance;
18. conflicting sources remain explainable rather than silently overwritten;
19. budget drops are deterministic and traceable;
20. oversized source chunk/range identity survives selection and invalidates on content/range change;
21. SelectionTrace for excluded secret does not retain raw/reconstructable payload;
22. cache hit cannot bypass changed scope/policy/source generation;
23. corrupted cache/index rebuilds from source authority;
24. semantic/model reranker cannot reintroduce excluded items;
25. semantic/model failure falls back deterministically;
26. source discovery never executes discovered project content;
27. malformed/path-traversal source identity is rejected;
28. repeated source/index failure converges under bounded backoff;
29. cancellation releases build resources;
30. heavy context/index load does not synchronously stall terminal progress.

Property/fuzz tests are required for source-identifier normalization, dependency invalidation and scope-key composition where malformed/untrusted input can reach them.

## 24. Acceptance criteria

SPEC-013 is satisfied only when implementation evidence proves:

- one provenance-first Local Context Engine consumes existing authorities rather than creating another one;
- scope/policy/sensitivity filtering occurs before relevance/model enhancement;
- authority and relevance remain distinct;
- filesystem/repository/worktree/symlink/submodule provenance is deterministic;
- LSP/index sources are generation-fenced and never source truth;
- ContextBundle is immutable and dependency-complete enough for correct invalidation;
- SelectionTrace is useful but policy-safe;
- deterministic provider-free retrieval remains functional;
- caches/indexes are rebuildable and cannot widen authority;
- required failure/security/property tests pass;
- calibrated resource/latency evidence is recorded before #681 implementation acceptance;
- terminal hot-path isolation is demonstrated under normal and failure load.

## 25. Explicit non-goals / deferred behavior

This specification does not define:

- `MemoryRecord` lifecycle, acceptance/conflict/memory-mode semantics (SPEC-012 / #847);
- `RunWorkingSet` retention/behavioral resumability;
- dispatch-time privacy/revocation races, provider-continuation abandonment or physical deletion completion;
- Action/approval/effect authority or retries (ADR-014/#846 and later specs);
- workflow DAG/multi-agent scheduling;
- learned routing/evaluation formulas;
- a concrete database/vector engine;
- a required language server;
- provider-specific context types;
- commercial shared/org context governance;
- production implementation authorization.

If implementation requires behavior outside these boundaries that changes authority, privacy, scope, compatibility, security or recovery semantics, stop and refine the owning ADR/spec before coding.
