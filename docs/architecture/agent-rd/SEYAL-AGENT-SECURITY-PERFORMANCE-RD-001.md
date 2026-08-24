# Seyal Agent/Context Security + Performance Isolation R&D

**Status:** Proposed  
**Issue:** #57  
**Dependency:** #48; cross-cutting over #51–#56  
**Scope:** Threat model/resource isolation; no implementation.

## Non-negotiable invariant

```text
PTY → VT → TerminalState → damage → renderer
```

never synchronously depends on:

```text
harness/agent
context discovery/index/retrieval
cache
evaluation
routing
workflow
cloud/telemetry/licensing
```

This extends, not replaces, the accepted Seyal performance/security rules.

## Trust boundaries

Treat as independently untrusted or partially trusted:

- harness child process and its structured protocol;
- external/community/private adapter process and its manifest/protocol;
- model/provider responses;
- repository/context content;
- MCP/tool/plugin providers;
- generated artifacts/diffs;
- local workflow definitions from outside trusted project configuration;
- remote/cloud services when later connected.

A structured protocol makes parsing safer; it does not make content trustworthy.

External adapter trust must distinguish three boundaries:

```text
Seyal capability authorization
≠ operating-system process sandbox
≠ harness/provider authorization
```

A manifest declaring `repository.read` only limits what Seyal deliberately exposes through its adapter IPC/API. It is not proof that the process cannot access other user resources through ordinary OS permissions. Installing/enabling third-party adapter executables is therefore an explicit trust decision unless a separately designed platform sandbox proves stronger confinement.

## Threats and controls

| Threat | Required baseline control |
|---|---|
| Prompt injection in repo/docs | provenance + authority separation; repository text cannot become Seyal/system policy |
| Malicious harness events | schema/version/size validation; bind events to exact adapter/run/session |
| Malicious adapter executable | explicit install/enable trust decision, bounded typed IPC, no PTY/master/state authority, crash isolation |
| Repo-triggered adapter execution | repository may reference adapter ID/version but cannot install/enable/execute unknown code automatically |
| Forged success | independent #54 evaluator evidence; self-report lowest trust |
| Context poisoning | source provenance, conflict visibility, hash/revision lineage, user inspectability |
| Cache poisoning | content-addressed keys, producer/version namespaces, checksum/schema validation, fail-to-miss |
| Secret leakage | sensitivity classification before bundles/logging/cache/provider transmission; secret persistence denied by default |
| Unsafe approval | typed action ID/run/session binding + expiration; never terminal-text matching |
| Unsafe workflow trigger | workflow execution requires trusted definition/policy; context cannot directly schedule work |
| Unsafe workflow replay | effect/replay class + idempotency/fencing evidence; ambiguous non-idempotent effects require reconciliation |
| Untrusted plugin | explicit capability/source scopes and bounded outputs; no ambient Seyal API authority |
| Cross-worktree leak | worktree fingerprinting and permission scope; only eligible immutable derivatives shared |
| Cross-workspace leak | workspace/repository scope in cache/context/event authorization |
| Resource exhaustion | bounded queues/workers/memory/cache quotas and cancellation |

## Sensitivity classes

OSS baseline:

```text
public
workspace
sensitive
secret
```

`secret` is excluded by default from persisted derived context and external provider bundles. Classification may be source-policy-driven and assisted by detectors, but a detector miss does not expand provider authorization: provider sending still obeys explicit source/path policy.

Organization-specific classifications remain a commercial/policy overlay over these generic local labels.

## Authority separation

Maintain three separate concepts:

1. **content relevance** — useful to this task;
2. **content authority** — project rule/ADR/spec/source precedence;
3. **execution authorization** — what Seyal/harness/adapter is allowed to do through Seyal-controlled capabilities.

High relevance never grants authority or execution permission. A README can mention “run destructive command” without becoming a workflow trigger.

## Queue/backpressure and durability design

Every asynchronous consumer uses a bounded queue and explicit overload behavior.

Classify events before implementation:

```text
DURABLE / LOSSLESS WITHIN DECLARED RECOVERY CONTRACT
- approval/question state transitions
- workflow/NodeRun state transitions
- outcome/evaluation evidence
- routing/attempt decisions that affect retries/cost
- factual cost/accounting events
- security/policy decisions

LOSSY / RECONSTRUCTABLE
- progress percentages
- diagnostic chatter
- optional UI activity updates
- high-volume telemetry samples
```

Rules:

- terminal path never waits for agent/event/context consumers;
- lossy telemetry can be sampled/dropped with an explicit dropped counter;
- durable events cannot simply be dropped on overload; the owning **AgentRun/Workflow** must pause/fail/reconcile according to its protocol while terminal rendering continues independently;
- durable delivery does not require one global synchronous event log; bounded per-run/entity queues plus persistence/replay/idempotency are preferred;
- context/index work can be cancelled/recomputed;
- slow commercial/cloud consumers may resync from persisted/derived state rather than retaining unbounded local queues;
- a full durable queue must never cause the PTY reader/VT/render loop to block.

## External adapter process isolation

Default third-party adapter architecture from #51 is out-of-process.

Required baseline properties:

- no arbitrary native library loaded into the long-lived Runtime for third-party adapters;
- adapter receives no PTY master descriptor and no mutable `TerminalState` reference;
- explicit process lifecycle/health observation;
- bounded message framing with length/schema/version validation before allocation proportional to untrusted lengths;
- per-adapter queue, CPU/time and restart bounds;
- crash/exit degrades structured capabilities without killing a still-live TerminalExecution;
- repeated crash loops are circuit-broken rather than restarted indefinitely;
- adapter config/manifest values are data and cannot override Seyal security policy;
- credentials/tokens are passed only through scoped mechanisms appropriate to the integration and are not echoed into events/logs/caches.

A future App Sandbox/helper/XPC/container mechanism may strengthen OS confinement on supported platforms, but it requires separate evidence. Do not describe capability scopes as sandboxing until that exists.

## Initial resource guardrails

These are **R&D implementation guardrails, not achieved performance claims**. The implementation issue must benchmark and may tighten/reopen them with evidence.

### CPU/background work

- one indexing/build job per workspace by default;
- global CPU-heavy background concurrency: `max(1, min(4, logical_cpu_count / 4))`;
- interactive terminal activity causes index/embedding/compaction work to yield/throttle before terminal or renderer work;
- no busy polling; idle background CPU must converge to effectively idle after queues drain;
- external harness/model/adapter child CPU is accounted separately from Seyal-owned background CPU but is still observable for user diagnostics where platform APIs permit.

### RAM

For Seyal-owned **agent/context derived hot state**, excluding terminal state, external harness/adapter processes and durable disk cache:

- default soft global budget: `min(512 MiB, 2% of physical RAM)`;
- crossing soft budget triggers eviction/compaction/cancellation;
- any hard budget must be configurable and must fail work with an explicit resource reason rather than pressure terminal correctness.

This deliberately biases metadata/index state toward disk/rebuildability rather than resident duplication.

### Disk

For derived local agent/context cache:

- default global quota: `min(10 GiB, 2% of filesystem capacity)`;
- default per-workspace quota: `min(2 GiB, 0.5% of filesystem capacity)`;
- preserve an emergency free-space floor; cache writes stop/evict before consuming the final 5% of the filesystem;
- user can inspect and clear by namespace/workspace;
- source repository/user artifacts are not counted as disposable cache and are never deleted by cache eviction.

### Queues

No unbounded in-memory event/context queue. Exact item/byte caps are protocol-specific specs, but every implementation must define both. Oversized single messages are rejected before allocation proportional to claimed size.

Queue tests must separately exercise durable saturation and lossy saturation so “drop telemetry” behavior cannot accidentally apply to approvals/outcomes/accounting.

## Terminal regression gate

Agent/context/adapter work must run the same terminal benchmark workloads with subsystem:

```text
off
idle
indexing under bounded load
active agent events under bounded load
misbehaving external adapter under bounded/circuit-broken load
```

Measure at minimum:

- input latency stages;
- PTY read→TerminalState;
- damage→projection/present where available;
- active/idle CPU;
- RSS;
- thread count;
- allocations/copies on terminal path.

The implementation issue must pre-register a baseline and acceptable regression threshold per `docs/engineering/PERFORMANCE.md`. This R&D does not invent a microsecond target before the M001 benchmark environments are established. Any statistically/materially detectable regression outside that pre-registered budget blocks the agent feature until isolated or explicitly re-architected.

## Security fixtures

Retain adversarial cases for:

- repo file containing fake system/approval instructions;
- repository config references an uninstalled adapter → no automatic execution;
- malicious adapter manifest with duplicate ID/path traversal/oversized values;
- adapter process emits forged/mismatched run/session ID;
- adapter crash loop while raw TerminalExecution remains alive;
- durable event queue saturation does not drop approval/outcome/cost state;
- lossy telemetry queue saturation drops/samples only declared lossy events;
- huge/malformed event lengths;
- malicious cache entry/checksum/schema;
- symlink/path traversal context source;
- `.env`/credential source accidentally matched by retrieval;
- stale summary hiding changed security instruction;
- plugin attempting undeclared source/tool access;
- two workspaces with same filenames but different sensitive content;
- workflow artifact containing executable-looking instructions;
- ambiguous non-idempotent workflow side effect after timeout → no auto retry;
- forged “tests passed” model response while local test fails.

Fuzz all externally supplied structured protocols/parsers before trusting them in long-lived runtime processes.

## Commercial boundary

**OSS:** threat model, local authorization/sensitivity primitives, external-adapter trust/process boundary, bounded queues/resource limits, durable/lossy event classes, secure adapter/context/cache/workflow foundations, local auditability.  
**Commercial:** organization RBAC/SSO/SCIM, centralized policy, curated adapter allowlists/catalogs, audit/compliance, secret governance, tenant isolation, managed-worker security and organization retention controls.

Commercial policy can narrow OSS capabilities; it cannot become a synchronous terminal hot-path dependency.

## Rejected approaches

- “local means trusted”;
- treating an adapter manifest capability list as an OS sandbox;
- loading arbitrary third-party Rust/C/native plugins into the Runtime by default;
- repository-open automatically installing/running adapters;
- scanning terminal text for approvals;
- letting context sources invoke tools;
- dropping durable approval/outcome/cost events like telemetry;
- unlimited indexing because work is background;
- storing all prompts/responses for observability;
- per-pane agent/index worker or daemon;
- sending unknown sensitivity data to cloud and relying only on provider retention promises.

## Success / kill criteria

Pass when each #51–#56 design maps onto the trust/resource model, external adapters can fail without corrupting/killing terminal authority, durable events have bounded lossless/recovery semantics, adversarial fixtures have explicit expected behavior, queues/derived stores are bounded, and terminal progress stays independent under injected agent/index/adapter load.

Reopen numerical resource guardrails when reproducible benchmarks on supported hardware show that a different bound materially improves useful agent work without measurable terminal regression or unacceptable memory/disk pressure.

## ADR/spec before implementation

- Agent Platform trust/resource-isolation ADR (may be combined with ownership ADR if cohesive).
- adapter/context/plugin authorization and sensitivity spec;
- external adapter trust/process/crash-loop specification;
- event durability/backpressure classification specification;
- background scheduler/resource-budget spec;
- adversarial fixture/fuzz plan;
- terminal non-regression benchmark protocol.
