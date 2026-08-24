# Seyal Runtime, Workspace & Agentic Continuity R&D

**Document:** SEYAL-RUNTIME-WORKSPACE-CONTINUITY-RD-001  
**Date:** 2026-08-24  
**Issue:** #82  
**Status:** R&D decision package for pre-M001-Pass-4 review  
**Scope:** Runtime/workspace identity, persistence classes, memory/resource accounting, and future agentic continuity constraints. No production implementation is authorized by this document.

## 1. Purpose

M001 Pass 4 is where Seyal introduces its first physical headless Runtime registry. Before that registry becomes production code, the product needs an explicit answer to four questions that otherwise become expensive migrations:

1. What survives when presentation disappears?
2. What does a Workspace own, and what does it only reference?
3. What stays hot in memory versus becoming warm/cold/visible-only state?
4. How can future agent work resume independently of the chat, provider session, model, terminal pane, or GUI that initiated it?

This R&D does **not** implement production persistence. It establishes ownership and lifetime semantics so later persistence can be added without moving PTYs, VT state, workspace authority, or agent-work identity.

The terminal hot path remains non-negotiable:

```text
PTY
→ TerminalExecution
→ Seyal VT/parser
→ authoritative TerminalState
→ damage
```

Workspace persistence, Block metadata, agent context, routing, caches, provider sessions, artifacts, cloud, licensing and UI may observe or act through bounded asynchronous/control seams. They never synchronously gate canonical terminal progress.

---

## 2. Existing authority this R&D must preserve

The accepted foundation already establishes:

- one persistent headless-capable per-user Runtime;
- one PTY/child lifecycle and one authoritative `TerminalState` per `TerminalExecution`;
- GUI presentation never owns or mirrors terminal authority;
- Block timeline authority is Runtime/workspace metadata keyed by `ExecutionId` and logical history anchors;
- Window/Tab/Split/PaneView are presentation objects, not process owners;
- layout persistence is separate from execution persistence;
- agent identity is separate from execution identity;
- agents, semantic extraction and persistence stay outside terminal hot paths;
- M001 proves GUI detach/crash survival, not Runtime-crash PTY survival;
- production history persistence, production layout persistence, multiple workspaces and agent orchestration are not M001 implementation scope.

This R&D narrows those rules into a future-compatible Pass 4 registry model. It does not reopen them.

---

## 3. External evidence reviewed

External systems are mechanism evidence, not Seyal architecture authority.

### 3.1 tmux

The tmux server owns sessions, windows and panes while clients attach/detach from sessions. Detaching a client leaves programs running. Windows may also be linked into multiple sessions.

Useful lesson for Seyal:

> presentation/client attachment is not execution lifetime.

Seyal does **not** copy tmux's session/window/pane ownership model because a Seyal Workspace also contains non-terminal work, agents, artifacts, attention and future context.

Sources:

- <https://man.openbsd.org/tmux.1>
- <https://github.com/tmux/tmux/wiki/Getting-Started>

### 3.2 WezTerm mux domains/workspaces

WezTerm's multiplexer can exist without a GUI. Detaching a mux domain removes its windows/tabs/panes from the local GUI without closing those panes, and attaching later restores them. WezTerm workspaces group mux windows for presentation/navigation.

Useful lessons for Seyal:

- a headless mux/runtime can own execution independent of GUI;
- detach is not close;
- workspace/presentation grouping should not become PTY ownership.

Seyal's Workspace is intentionally richer and more durable than a presentation label.

Sources:

- <https://wezterm.org/multiplexing.html>
- <https://wezterm.org/config/lua/MuxDomain/detach.html>
- <https://wezterm.org/recipes/workspaces.html>

### 3.3 Resumable coding-agent sessions

Claude Code exposes resumable sessions through `--resume <session-id>` and continuation of the latest conversation. That is useful adapter capability evidence, but a provider session identifier is controlled by the provider/harness and cannot be Seyal's durable work identity.

Useful lesson for Seyal:

> provider session resumption is one way to continue an `AgentRun`; it is not the definition of the WorkItem, Workspace, Attempt, outcome or context authority.

Source:

- <https://docs.anthropic.com/en/docs/claude-code/cli-usage>

---

## 4. Failure modes to prevent before Pass 4

### F-1 — `pane == execution`

If a PTY is owned by a pane/tab/window, closing presentation kills or migrates the execution.

**Prevented by:** presentation objects only reference stable execution identity.

### F-2 — `workspace == window group`

If Workspace is only current GUI layout, headless work, mobile attach, agent work and cold artifacts have no durable parent.

**Prevented by:** Workspace is a domain identity independent of presentation.

### F-3 — execution belongs to many owning workspaces

Multiple owners create ambiguous deletion, retention, policy and context boundaries.

**Prevented by:** exactly one owning Workspace association for an execution; other views/references are explicit and non-owning.

### F-4 — `chat == work`

If provider conversation/chat/session identity is the task identity, model switching, retries, handoffs, routing and provider-session loss destroy continuity.

**Prevented by:** durable `WorkItemId` / `AttemptId` / `AgentRunId` with provider resume locators as adapter metadata only.

### F-5 — persist everything as transcript/event bytes

Recording terminal bytes, alternate-screen frames, prompts, agent text and layout as one journal creates unbounded disk/memory/privacy cost and still cannot restore a live PTY.

**Prevented by:** separate persistence classes and source-truth/derived-data rules.

### F-6 — database-first Pass 4

Choosing SQLite/event sourcing/index engines before the stable domain/lifecycle contract is known freezes accidental schemas.

**Prevented by:** Pass 4 implements only the permanent live Runtime boundary and identity/association seams required now.

### F-7 — hidden memory grows with historical/agent context

Keeping full terminal history, Block output, provider transcripts, context bundles or agent event logs resident per execution makes hundreds of persistent executions impractical.

**Prevented by:** hot/warm/cold/visible-only ownership and explicit memory accounting.

### F-8 — Runtime restart falsely claims live continuity

Restoring metadata after Runtime restart and calling it the same live terminal would misrepresent a dead PTY as recovered.

**Prevented by:** Runtime incarnation identity and explicit separation of durable record continuity from live OS-resource continuity.

---

## 5. Identity taxonomy

Identity semantics are more important than the eventual byte encoding. Exact UUID/ULID/storage encoding is intentionally deferred unless implementation evidence requires a decision.

| Identity | Lifetime / durability | Owner | Reuse rule | Notes |
|---|---|---|---|---|
| `RuntimeId` | one Runtime process incarnation | Runtime | never reused intentionally | changes after Runtime restart; detects stale client/control state |
| `WorkspaceId` | durable domain identity | Runtime/workspace metadata | never reused after deletion | survives GUI/provider/model changes; named multi-workspace persistence is later |
| `ExecutionId` | durable logical record for one terminal execution | Runtime execution registry / later durable metadata | never reused | one ID refers to one PTY execution lifetime; a dead PTY is never resurrected under the same ID |
| `AttachmentId` | ephemeral attachment lifetime | Runtime attachment manager | may use scoped generation; never durable authority | always scoped to current Runtime incarnation |
| `BlockId` | durable metadata identity | workspace Block timeline | never reused within Workspace | references `ExecutionId` + logical anchors; never owns output/PTy |
| `WorkItemId` | durable user/workflow goal | future workspace/work domain | never reused | survives model/provider/chat changes |
| `AttemptId` | durable evidence for one attempt/candidate | future work domain | never reused | retries/parallel candidates create new Attempts |
| `AgentRunId` | durable record of one harness/agent run | future agent/work domain | never reused | may contain provider resume locator(s) |
| provider/harness session locator | provider-defined, adapter-scoped | harness adapter metadata | provider semantics | never a Seyal Workspace/WorkItem/Attempt identity |
| Window/Tab/Split/PaneView identity | presentation lifetime | client UI | client semantics | never execution/process/context authority |

### 5.1 Runtime restart semantics

`RuntimeId` deliberately changes after Runtime restart.

A durable `WorkspaceId`, `WorkItemId`, `AttemptId`, `AgentRunId` or completed `ExecutionId` record may later be loaded from persistent metadata, but M001 must not infer that the old PTY is live.

For an execution whose Runtime died unexpectedly:

```text
old ExecutionId
→ durable record may later say RuntimeEnded / Lost / UnknownFinalState
→ no live PTY is reconstructed
→ any replacement execution receives a new ExecutionId
```

The exact durable record-state vocabulary is deferred to the persistence milestone. The invariant is not.

---

## 6. Workspace ownership model

### 6.1 Workspace is domain state, not presentation

Target model:

```text
User Runtime
│
├─ Workspace A
│    ├─ owning Execution associations
│    ├─ BlockTimeline / activity metadata
│    ├─ future WorkItems / AgentRuns / Attempts
│    ├─ artifacts / attention references
│    └─ context/security scope
│
└─ Workspace B
     └─ ...

Presentation clients
└─ Window / Tab / Split / PaneView
      └─ reference WorkspaceId / ExecutionId / AgentRunId / ArtifactId
```

A Workspace can exist while zero windows are open.

A Window can show references into a Workspace without owning it.

### 6.2 Exactly one owning Workspace association per execution

A live terminal execution has exactly one owning `WorkspaceId` **in Runtime/workspace metadata**.

This association must not be stored as PTY ownership inside `TerminalExecution` itself. `TerminalExecution` remains terminal infrastructure.

Why one owner:

- deterministic retention/deletion semantics;
- deterministic policy/context boundary;
- no accidental cross-project context sharing;
- simpler resource accounting;
- future collaboration/RBAC can bind to a clear domain owner.

A different Workspace/client may hold an explicit non-owning reference/view if product UX later requires it. That does not transfer ownership or context authority.

### 6.3 Implicit/default Workspace

Seyal must always have a valid Workspace association even before multi-workspace UI exists.

Pass 4 may therefore use one typed implicit/default Workspace in Runtime metadata. Because `WorkspaceId` is durable domain identity, this default identity must be stable within the local user scope across Runtime incarnations; it must not be regenerated from `RuntimeId` or another process-local value. The exact persistence/storage encoding remains deferred.

It must not use `None`, current window, cwd, repository path or pane identity as the hidden owner.

The exact persistence/storage representation of named Workspaces is deferred.

### 6.4 Close, hide, archive and delete are different

Future semantics must preserve these distinctions:

- **close/hide presentation:** removes views only; no workspace/execution lifecycle change;
- **detach client:** removes attachment only; no workspace/execution lifecycle change;
- **archive workspace:** metadata/presentation state, not implicit execution termination;
- **delete workspace:** destructive domain operation; must not silently kill live executions.

If a delete is requested while the Workspace owns live executions, the operation must require an explicit disposition such as rehome or terminate. M001 does not implement Workspace deletion.

---

## 7. Persistence classes

Do not create one generic "session persistence" subsystem. These classes have different correctness and performance properties.

### P-1 — Live execution persistence

Meaning:

```text
GUI disappears
→ Runtime remains
→ PTY + child + TerminalState remain live
```

Storage: live memory + kernel/process resources.

M001: required.

A journal cannot replace this class.

### P-2 — Durable workspace/work metadata

Examples:

- Workspace identity/name/policy metadata;
- execution records/associations;
- Block metadata;
- WorkItem/Attempt/AgentRun records;
- outcome/attention/artifact references.

Storage: future durable metadata store.

M001 Pass 4: semantic IDs/association seams only; no production metadata database required.

### P-3 — Cold payload/history/context/artifacts/cache

Examples:

- terminal scrollback chunks;
- completed Block payload references;
- agent event logs;
- context manifests/indexes;
- derived summaries/embeddings;
- large artifacts.

Storage: bounded/adaptive, often disk-backed, content-addressable/chunked where later evidence justifies it.

M001 Pass 4: not implemented.

### P-4 — Presentation/layout persistence

Examples:

- windows;
- tab/split trees;
- pane sizes;
- inspector state;
- selected workspace/view mode.

Storage/lifetime belongs to client/presentation state and may reference durable runtime IDs.

M001 Pass 4: not implemented.

### P-5 — Runtime crash/reboot recovery

This is not a stronger form of P-1.

It contains at least two separate concerns:

1. reconstruct durable metadata after Runtime restart;
2. preserve live PTY/process resources across Runtime failure.

M001 does not implement #2. Future PTY keeper/worker/supervisor architecture requires its own R&D and measurements.

---

## 8. Agentic continuity model

Seyal must not inherit the temporary-chat lifecycle of general AI chat products.

Future domain chain:

```text
Workspace
→ WorkItem
→ Attempt
→ AgentRun
→ Execution(s)
→ Artifact / Evaluation / Outcome / Attention
```

### 8.1 Chat is a view

A conversation/chat surface may show interactions with one or many `AgentRun`s, but it is not the durable authority for:

- the goal;
- accepted constraints/decisions;
- work status;
- selected source context;
- artifacts;
- execution ownership;
- routing history;
- final outcome.

There is intentionally no requirement for a durable core `ChatId`.

If a product surface needs conversation identity later, it references the work/agent domain; it does not replace it.

### 8.2 WorkItem is the continuity anchor

A `WorkItem` represents the durable goal and its accepted constraints/decision references.

Changing model/provider/harness does not create a new WorkItem.

Retries or parallel alternatives create distinct Attempts so evidence is never overwritten.

### 8.3 AgentRun is not provider session identity

An `AgentRun` may hold adapter metadata such as:

```text
provider = Claude Code
resume_locator = <provider session id>
capabilities = ...
```

If the provider session still exists, the adapter may resume it.

If it does not exist, Seyal can still continue the WorkItem by building context from durable source truth/provenance and creating a new Attempt/AgentRun as appropriate.

### 8.4 Execution association is many-to-many at the work layer

One AgentRun may use multiple executions.

One terminal execution may be observed/used by multiple AgentRuns over time.

Therefore:

```text
AgentRun != TerminalExecution
```

and neither lifecycle implicitly owns the other unless a future WorkItem policy explicitly requests termination.

---

## 9. Context continuity without transcript dependence

Future context should be reconstructed from scoped source truth plus derived manifests, not by replaying an unbounded chat transcript.

Recommended layers:

```text
WorkspaceContext
  stable project/workspace sources + policy

WorkItemContext
  goal, constraints, decisions, source/artifact references

AttemptContext
  attempt-specific plan/evidence/inputs

AgentRunContextBundle
  provider-ready selected bundle + provenance manifest + fingerprint
```

Rules:

- source files, repository state, explicit user decisions and retained artifacts are source truth;
- summaries, embeddings, rankings and compacted context are derived/rebuildable;
- context bundle records provenance and version/fingerprint;
- provider prompt/session state is adapter data;
- context does not cross Workspace boundaries merely because the same provider/model is used;
- sensitive context gets explicit retention/cache policy;
- terminal output needed as context is referenced/selected through bounded history/artifact seams later, never copied synchronously from the PTY hot path.

---

## 10. Routing continuity

Routing input should eventually be:

```text
WorkItem
+ required capabilities
+ Workspace policy/context
+ budget
+ available harness/model/execution targets
+ prior Attempt outcomes/evidence
→ routing decision
→ AgentRun / Attempt
```

Routing must **not** be based on whichever chat tab/model is currently active.

A provider/model switch therefore creates a routing/action record rather than destroying continuity.

Local deterministic routing remains OSS foundation per the Agent Platform R&D; proprietary learned routing may consume the same seams without changing work identity.

---

## 11. Memory/resource ownership model

### 11.1 Hot Runtime-shared state

Examples:

- reactor/kqueue and reusable event buffers;
- execution registry indexes;
- bounded control queues;
- shared immutable/configuration state;
- small Workspace association indexes.

Properties:

- one/small bounded set per Runtime;
- no per-execution threads/daemons;
- no full workspace history/context duplicated here.

### 11.2 Hot per-live-execution state

Examples:

- PTY master/child lifecycle bookkeeping;
- VT parser;
- primary screen/current terminal modes;
- alternate screen only while active/required;
- cursor/damage state;
- bounded input queue;
- small reactor/lifecycle metadata;
- minimum live logical-history seam required by current milestone.

This is the memory class constrained by the architecture's hidden/detached execution target **before scrollback payload**.

### 11.3 Warm domain state

Examples:

- active Workspace metadata/indexes;
- current lightweight Block skeleton;
- active WorkItem/AgentRun state;
- active attention/artifact indexes.

Properties:

- bounded;
- asynchronously loadable/rebuildable where possible;
- not required for terminal byte progress.

### 11.4 Cold/evictable state

Examples:

- completed terminal history chunks;
- completed Blocks/event streams;
- old AgentRun/Attempt evidence;
- context indexes/embeddings/summaries;
- artifacts;
- caches.

Properties:

- disk-backed/paged later;
- not permanently resident simply because a Workspace is open;
- explicit retention/eviction/security policy;
- derived caches may be deleted and rebuilt.

### 11.5 Visible-only state

Examples:

- local display projection consumer state;
- renderer draw/shaping buffers;
- glyph/atlas references;
- Metal surfaces/textures tied to visible clients.

A hidden/detached execution must not retain a dedicated full GPU terminal surface.

---

## 12. Make the memory target measurable

The accepted foundation states these targets:

- hidden/detached idle terminal: `<= 256 KiB` Seyal-owned hot resident memory before scrollback payload;
- 100 hidden idle terminals: `<= 25 MiB` Seyal-owned execution overhead;
- thread count does not scale linearly with pane/execution count;
- hidden/detached executions hold no dedicated GPU render surface.

An absolute per-execution number is meaningless without terminal dimensions because current screen storage is proportional to `rows × cols` and an active alternate screen adds a second grid while it is active.

Therefore use these profiles:

### Baseline profile for target comparison

```text
80 x 24
primary screen active
alternate screen inactive
zero presentation clients
minimal/no scrollback payload beyond M001 seam
idle shell
```

Compare the `<= 256 KiB` target against this profile.

### Scaling profiles

Also report, without silently applying the same absolute target:

```text
120 x 40
200 x 60
80 x 24 with alternate screen active
120 x 40 with alternate screen active
```

The active alternate-screen case is required because a long-running TUI may remain detached.

### Runtime population profiles

For each relevant profile, measure at least:

```text
1
10
50
100
```

live executions.

Future R&D may add 500-execution stress evidence; M001 does not need to turn that into an acceptance workload.

---

## 13. Memory measurement methodology for Pass 4

Pass 4 should report both process-level and component-level evidence.

### 13.1 Process-level

Record:

- Runtime RSS after warm-up;
- idle CPU;
- thread count;
- file descriptor count;
- execution count;
- terminal dimensions;
- alternate-screen state;
- queue capacities;
- build mode/commit/macOS/chip.

Use repeated samples and report median/range or the repository's defined percentile method.

### 13.2 Incremental/slope view

Do not infer per-execution memory from one subtraction only.

Use measurements at 1/10/50/100 and report the approximate incremental RSS slope after shared Runtime startup cost. This helps distinguish:

```text
shared Runtime overhead
vs
per-execution overhead
```

### 13.3 Component ledger

Where practical, record Rust value sizes and owned allocation capacities for major execution components in a diagnostic/benchmark-only path:

- `TerminalState` fixed values;
- primary cell allocation capacity;
- line-id allocation capacity;
- alternate cell/line allocation while active;
- input queue capacity/occupancy;
- Runtime registry/association metadata;
- reactor registration metadata.

Do not add bookkeeping to the production per-byte path merely to measure it.

### 13.4 Redesign trigger

Do not optimize cell/history representation based on intuition alone.

If measured baseline materially exceeds the architecture target or scaling slope predicts unacceptable 100-execution RSS, open a focused Issue with the allocation ledger and compare alternatives before changing `Cell`/screen/history representation.

Issue #71 remains the separate LineId/scroll-heavy durability/performance follow-up unless evidence proves it blocks Pass 4.

---

## 14. Workspace/resource lifecycle matrix

| Event | Workspace | Live execution | presentation | future work/agent metadata |
|---|---|---|---|---|
| close window/tab/pane | unchanged | unchanged | removed/changed | unchanged |
| GUI process exits/crashes | unchanged | unchanged | detached/reclaimed | unchanged |
| last attachment detaches | unchanged | unchanged while primary live | none | unchanged |
| primary child exits | owning Workspace remains | execution finalizes per ADR-006 | view becomes completed/stale reference | may retain outcome/evidence later |
| explicit terminate execution | unchanged | termination state machine | view updates | records may reference result later |
| archive Workspace (future) | archived metadata state | no implicit kill | hidden/filtered | retained by policy |
| delete Workspace (future) | destructive | requires explicit disposition for live executions | removed | deletion/retention policy applied |
| Runtime controlled shutdown | metadata concept unchanged | M001 terminates/finalizes live executions | detached | future durable records may survive |
| Runtime crash | durable records may later reload | no M001 live-PTY survival claim | disconnected | future metadata recovery separate |
| provider session disappears | unchanged | terminal execution unaffected unless explicitly tied by work policy | chat may lose direct resume | WorkItem remains; new Attempt/AgentRun may continue |

---

## 15. Cleanup and retention boundaries

### 15.1 Execution

When a live execution finalizes:

- PTY/process/reactor/input hot resources are released promptly;
- no completed execution retains a live-grid allocation merely for future history UI unless a later measured design explicitly requires it;
- future durable execution/history records may retain identifiers and cold payload references.

### 15.2 Workspace

Closing or hiding is never deletion.

Deletion is a separate explicit operation with policy for live executions, artifacts, context and derived caches.

### 15.3 Agent/work

Completed Attempts/AgentRuns become cold metadata/evidence. Provider transcripts should not remain hot just because the Workspace remains active.

### 15.4 Caches

Caches are derived:

- bounded;
- inspectable/clearable;
- invalidatable by source hash/revision/model/config;
- never required to reconstruct source truth;
- sensitive cache deletion is scoped by Workspace/security metadata.

---

## 16. Security/isolation rules

Workspace is a future security/context boundary even in single-user OSS.

Rules:

- an execution's owning Workspace is explicit Runtime/workspace metadata;
- cross-workspace view/reference does not grant automatic context sharing;
- context retrieval defaults to the owning WorkItem/Workspace scope;
- handoff across Workspace boundaries must be explicit and provenance-carrying;
- provider session/resume locator is adapter-scoped and may be sensitive;
- using the same provider/model/session mechanism does not merge Workspace context;
- derived caches must carry Workspace/sensitivity/provenance metadata sufficient for safe invalidation/deletion;
- terminal hot state does not contain agent credentials, cloud tokens or commercial entitlement state unless required by the child process environment itself, which is a separate execution/security concern.

---

## 17. Pass 4 constraints established by this R&D

The Headless Runtime implementation should now be constrained as follows.

### Required now

- `RuntimeId` represents one Runtime incarnation and changes on restart;
- `WorkspaceId` represents domain identity; the Pass-4 implicit/default Workspace identity remains stable in the local user scope across Runtime incarnations;
- `ExecutionId` is opaque, non-reused and persistence-compatible rather than a PID/FD/pane/reactor token;
- execution publication remains separate from presentation attachment;
- Runtime maintains an execution→owning-Workspace association seam outside `TerminalExecution` terminal ownership;
- Pass 4 may use one implicit/default Workspace; do not implement named multi-workspace persistence/UI;
- no API makes Window/Tab/Split/Pane the owner of a live execution;
- memory/resource benchmarks use fixed dimension/state profiles and 1/10/50/100 populations;
- no full transcript/history/agent/context payload becomes resident Runtime registry state;
- no durable core `ChatId` or provider-session ID is introduced as work identity;
- any future-facing work/agent fields added by Pass 4 must be opaque references/events only and must not pull the agent platform into the terminal Runtime.

### Not required now

- persistent Workspace database;
- named Workspace CRUD;
- layout restore;
- production history paging;
- WorkItem/AgentRun implementation;
- provider adapter/resume implementation;
- context engine/cache/router/workflow;
- Runtime-crash PTY keeper/recovery.

---

## 18. Alternatives rejected

### A. Workspace owns PTY/TerminalExecution directly

Rejected. Workspace owns association/domain metadata; `TerminalExecution` remains terminal infrastructure owned by Runtime execution registry.

### B. Execution has no Workspace until UI assigns one

Rejected. Headless and agent-created executions would have ambiguous retention/context ownership. Use a typed implicit/default Workspace association.

### C. Execution can have many owning Workspaces

Rejected. Use one owner plus explicit non-owning references/views.

### D. Runtime-scoped default Workspace identity

Rejected. Regenerating the implicit/default Workspace from each `RuntimeId` would contradict the durable Workspace contract and force identity migration as soon as Workspace metadata persistence is added.

### E. Use provider conversation/session as WorkItem identity

Rejected. It prevents provider switching, deterministic routing, independent retries and recovery when provider session state disappears.

### F. Persist the whole agent chat as canonical context

Rejected. Transcript is useful evidence/presentation, not complete source truth; it is unbounded, provider-shaped and vulnerable to stale context.

### G. Put persistence/event writes inline with PTY output

Rejected permanently for canonical terminal progress.

### H. Implement production persistence before Pass 4

Rejected. The semantic seams are required now; storage technology is not.

---

## 19. Implementation/R&D sequence after acceptance

```text
Pass 1-3 + VT correction             done
Runtime reactor ADR                  done
this continuity/persistence R&D      current gate
        ↓
M001 Pass 4 Headless Runtime
        ↓
Pass 5 attachment/projection
        ↓
...
Pass 8 minimal Block/workspace seam
Pass 9 real GUI detach/reconnect proof
        ↓
future production workspace/history persistence
future agent platform verticals
```

Agent platform work later consumes the identities/seams established here instead of forcing a Runtime migration.

---

## 20. Exit gate

This R&D is ready for architecture acceptance when all are true:

- [x] live execution persistence is separated from durable metadata persistence;
- [x] Runtime crash/reboot recovery is not conflated with journaling;
- [x] Workspace is domain identity, not GUI layout;
- [x] one owning Workspace association per execution is explicit;
- [x] implicit/default Workspace identity is restart-stable in local user scope rather than Runtime-scoped;
- [x] close/hide/detach/delete semantics are distinct;
- [x] Runtime/Workspace/Execution/Attachment identity lifetimes are explicit;
- [x] future WorkItem/Attempt/AgentRun identity is independent of provider chat/session;
- [x] chat is presentation, not durable work authority;
- [x] context reconstruction is provenance/source based, not transcript-only;
- [x] routing continuity is WorkItem-based rather than active-chat based;
- [x] hot/warm/cold/visible-only memory ownership is explicit;
- [x] the 256 KiB target has a fixed baseline profile and scaling profiles;
- [x] Pass 4 measurement methodology covers 1/10/50/100 live executions;
- [x] Workspace security/context isolation and cleanup boundaries are explicit;
- [x] no rule adds synchronous persistence/agent/context work to PTY→VT→damage;
- [x] Pass 4 can remain narrow and does not need production persistence or agent implementation.

The resulting architectural decision is recorded separately in ADR-007.