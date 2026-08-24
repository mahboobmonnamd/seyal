# Seyal Local Cache + Cache-Aware Context Builder R&D

**Status:** Proposed  
**Issue:** #53  
**Dependencies:** #48, #52  
**Scope:** OSS local caches and provider-cache-aware prompt assembly; no implementation.

## Decision

Caching is a derived acceleration layer, never project truth. Use content-addressed fingerprints and explicit dependency lineage. Keep Seyal local caches separate from provider prompt caching.

```text
source truth
  ↓ hashes / revision fingerprints
local derived caches
  ↓ deterministic ContextBundle
provider-specific layout adapter
  ↓
provider prompt cache (when supported)
```

## Cache namespaces

| Namespace | Example key inputs | Notes |
|---|---|---|
| `source-content` | normalized source bytes hash | immutable/shareable by content |
| `repo-metadata` | repository identity + revision + dirty-state fingerprint + indexer version | stateful |
| `symbol-index` | content hash + parser/indexer version | shareable for identical blobs |
| `embedding` | chunk hash + embedding model/provider/version/config | optional |
| `retrieval` | task/query fingerprint + eligible ContextItem set fingerprint + retriever config + policy scope | worktree/policy sensitive |
| `selection` | candidate/result hashes + ranking/budget config | records deterministic selection |
| `summary` | ordered source hashes + summarizer model/config/prompt version | derived only |
| `context-bundle` | ordered selected item versions + budget + renderer/layout policy | exact deterministic bundle |
| `evaluation` | artifact/worktree/test/evaluator/config hashes | only deterministic/replay-safe evaluators |
| `provider-cache-metadata` | provider/model/session + reported cache identity/usage | observations, not local cache entries |

## Fingerprints

Use cryptographic content hashes for immutable content and a deterministic structured fingerprint for stateful selections. Keys include the **algorithm/schema version** so a bug fix cannot silently reuse old semantics.

For a dirty worktree:

```text
WorktreeFingerprint =
  repo identity
  + base revision
  + changed path set
  + content hash for each dirty path
  + relevant instruction/policy fingerprints
```

Do not key dirty-state caches by branch name or mtime alone.

## Invalidation

Invalidation follows dependency edges:

```text
source changed
 → chunk/symbol invalid
 → embedding/summary invalid
 → retrieval result referencing it invalid
 → selection/bundle invalid
```

Policy/sensitivity changes invalidate any cache whose eligible-source set changes even if file bytes do not.

Cache corruption or unreadable schema is a miss, not a reason to block terminal or agent execution.

## Provider prompt caching

Provider-side prompt caching is a separate external optimization. Current provider behavior differs and changes over time; adapters expose capabilities/observations rather than core code assuming one policy.

Prompt assembly should nevertheless maximize reusable prefixes where correctness permits:

```text
stable system/tool definitions
+ stable authoritative project instructions/context
+ stable reusable retrieved context
+ task-specific context/diff
+ current user request
```

Rules:

- never move stale/irrelevant content into the stable prefix solely to chase cache hits;
- exact provider layout/caching rules stay in provider/harness adapters;
- cache-token hits/savings are recorded only when reported or calculable from published provider semantics;
- task-specific sensitive material must not be widened in scope for reuse.

## Sensitive data

Default policy:

- secrets/credentials: do not persist in derived caches;
- sensitive workspace content: local cache permitted only under explicit local policy and ownership permissions;
- cross-worktree sharing: only content-addressed immutable derivatives whose sensitivity scope permits sharing;
- external/provider cache eligibility: independently evaluated from local cache eligibility.

Encryption at rest may reduce disk exposure but does **not** make cross-user/cross-workspace reuse safe.

## Cache poisoning controls

- namespace keys include adapter/indexer/model/config versions;
- provenance is stored with every derived object;
- untrusted provider/harness output cannot overwrite source-content entries;
- no executable object deserialization from cache;
- size/schema bounds before decoding;
- atomic write + checksum/length validation;
- failed validation becomes a miss and can quarantine/delete the entry;
- plugin caches are namespaced by plugin identity/version and permission scope.

## Storage and eviction

Prefer one bounded embedded cache service/library owned by the local Seyal Runtime/background subsystem rather than a daemon per workspace.

Separate eviction classes:

1. cheap/rebuildable retrieval/selection entries — shortest retention;
2. expensive embeddings/summaries — retain by LRU/value while dependencies remain valid;
3. content/index metadata — retain while repository active, with global bounds;
4. sensitive entries — shorter policy-controlled retention and explicit clear.

Exact byte limits are deferred to #57 resource-budget measurements; architecture requires per-workspace and global quotas, inspect/clear commands, and low-disk emergency eviction.

## Metrics

Record by namespace:

- hit/miss/invalid/corrupt counts;
- bytes and entries;
- build/lookup latency;
- work avoided (index/chunk/summary operations);
- ContextBundle tokens reused;
- provider cached vs uncached tokens when reported;
- provider cost avoided using contemporaneous published rate metadata, clearly separated from actual charged cost;
- cache correctness failures (target: zero accepted stale/wrong-scope reuse).

Do not label local retrieval hits as provider prompt-cache hits.

## Experiments

Compare on the same retained task/repository corpus:

A. no derived cache;  
B. content/index cache only;  
C. all deterministic local caches;  
D. deterministic caches + summaries/embeddings;  
E. provider cache-aware prefix layout where supported.

Measure latency, CPU, disk, bundle tokens, provider-reported cache tokens and outcome quality. An optimization fails if it improves hit rate but worsens correctness/outcomes.

## Rejected approaches

- global cache keyed only by file path;
- TTL-only correctness;
- semantic similarity as sufficient reuse key;
- persisting arbitrary prompts/responses indefinitely;
- shared cache across workspaces without sensitivity/ownership checks;
- provider cache metadata as local source truth;
- cache rebuild that blocks PTY/render progress.

## OSS/commercial boundary

**OSS:** every local namespace above, deterministic fingerprints, cache-aware bundle builder, provider cache capability metadata, inspection/clear and local savings metrics.  
**Commercial:** permissioned cross-user reusable derived knowledge, organization cache analytics, provider-specific adaptive layout optimization and managed distributed caches when they outperform OSS baseline.

## Success / kill criteria

Pass when identical immutable inputs deterministically reuse derivatives, dirty worktrees never cross-contaminate selection/bundles, policy changes invalidate affected entries, corrupted entries degrade to misses, and terminal benchmarks show no cache/index dependency in hot-path progress.

Reject commercializing any optimization that cannot beat the same OSS baseline on outcome quality plus measured cost/latency.

## ADR/spec before implementation

Coordinate one Local Context + Cache ownership/invalidation ADR with #52 rather than separate competing state models. Add a fingerprint/cache behavior spec and retained invalidation/security fixtures before production code.
