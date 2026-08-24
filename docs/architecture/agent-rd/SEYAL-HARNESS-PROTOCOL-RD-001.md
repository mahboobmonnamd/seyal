# Seyal Harness Protocol R&D

**Status:** Proposed  
**Issue:** #51  
**Dependency:** shared domain vocabulary in #48 / PR #60  
**Scope:** Research/design only; no production adapter.

## Evidence reviewed

Current upstream integration surfaces were checked rather than inferred from terminal behavior:

- Claude Code CLI, hooks and MCP documentation: https://code.claude.com/docs/en/cli-reference , https://code.claude.com/docs/en/hooks , https://code.claude.com/docs/en/mcp
- OpenAI Codex CLI and App Server: https://developers.openai.com/codex/cli/reference , https://developers.openai.com/codex/app-server/
- Gemini CLI ACP, CLI/session and hooks documentation: https://geminicli.com/docs/cli/acp-mode/ , https://geminicli.com/docs/cli/cli-reference/ , https://geminicli.com/docs/hooks/reference/
- OpenCode server and permissions: https://opencode.ai/docs/server/ , https://opencode.ai/docs/permissions/

All capabilities are version-sensitive and must be discovered/probed by an adapter rather than assumed from this snapshot.

## Comparative findings

| Area | Claude Code | Codex | Gemini CLI | OpenCode |
|---|---|---|---|---|
| Raw interactive terminal | Strong TUI | Strong TUI | Strong TUI | Strong TUI |
| Structured control | stream JSON/non-interactive + hooks | App Server JSON-RPC | ACP JSON-RPC + stream JSON | HTTP/OpenAPI + SSE |
| Resume/session identity | CLI session resume/session ID | thread/session lifecycle via App Server/CLI | project-scoped resumable session UUIDs | session IDs/API |
| Cancel/abort | process/session control + hook lifecycle | structured turn/thread interruption | ACP `cancel` | `/session/:id/abort` |
| Approvals | permission modes + PermissionRequest/hooks | explicit approval server requests | ACP session mode/policy/hooks | permission endpoints |
| Tools/MCP | MCP + tool hooks | MCP/config + structured item events | MCP through ACP + hooks | provider/tool/permission API |
| Artifacts/diffs | derive from structured tool/output + repo | structured items plus repo state | structured output/hooks + repo | explicit session diff API |
| Model selection | CLI/config | CLI/App Server config | ACP model change/CLI config | provider/model config API |
| Usage/cost | structured result/telemetry where exposed | usage events/metadata where exposed | telemetry/JSON where exposed | provider/session metadata; adapter must verify |

## Architecture decision

Seyal uses **dual-surface adapters**:

```text
                         ┌─ raw TerminalExecution ─→ real TUI/user interaction
HarnessAdapter ──────────┤
                         └─ structured control/event surface ─→ Seyal RunEvents
```

The raw surface preserves terminal correctness. The structured surface provides semantic lifecycle, approvals, tool calls, usage and artifacts when upstream supports them.

The two surfaces must reference the **same upstream logical session** when a harness supports that safely. Seyal must never launch a duplicate agent merely to gain structure.

### Rejected: PTY scraping as the semantic protocol

Parsing arbitrary terminal text for approvals, tool calls or completion is fragile across versions/themes/locales, breaks TUIs, and can mistake untrusted model output for control state. Terminal output may be displayed/indexed under normal terminal/content policy, but semantic events require structured upstream evidence or are marked unavailable.

### Rejected: lowest-common-denominator adapter

A single interface with only start/input/stop would discard important approval, usage, diff and session features. Seyal instead negotiates namespaced capability versions from the shared `CapabilitySet`.

## Adapter ecosystem and extension model

Seyal OSS must support **both** first-party adapters maintained in this repository and independently developed third-party/private adapters. Open source contribution is one distribution path, not a requirement for integration.

```text
Seyal OSS
  ├─ Harness Adapter Protocol + SDK
  ├─ Adapter manifest schema
  ├─ discovery / compatibility negotiation
  ├─ conformance suite
  ├─ first-party adapters
  │    ├─ Codex
  │    ├─ Claude Code
  │    └─ ...
  └─ external adapter processes
       ├─ community adapter
       └─ private organization adapter
```

### First-party adapters

Important broadly used harnesses should have reviewed first-party OSS adapters where maintenance cost is justified. They may be compiled with Seyal when doing so is the simplest and safest deployment, but they still conform to the same capability and event contracts exposed to external adapters.

### External adapters are out-of-process by default

The default third-party extension boundary is an **adapter executable/process**, not an arbitrary Rust dynamic library loaded into the long-lived Seyal Runtime.

Reasons:

- Rust has no stable plugin ABI suitable for uncoordinated third-party binaries;
- an unsafe/incompatible in-process adapter could corrupt or crash the Runtime that owns live executions;
- process isolation gives clear lifecycle, crash and resource boundaries;
- external adapters may be implemented in languages other than Rust;
- adapter upgrade/restart can be independent of Seyal release cadence.

The adapter control/event channel is a bounded, versioned, typed IPC protocol. Exact wire encoding is deferred to the implementation spec; it must not place JSON/serialization or external adapter execution on PTY → VT → TerminalState → render progress.

An external adapter never receives the PTY master FD or becomes terminal-state authority. If a harness needs a raw TUI, Seyal owns the real `TerminalExecution`; the adapter references the same logical harness session through opaque session identifiers/capabilities.

### Adapter manifest

Every externally discovered adapter supplies a non-executable manifest containing at least:

```text
adapter_id
adapter_version
protocol_version_range
entrypoint
publisher_identity? / signature_metadata?
supported_platforms
claimed_capabilities
required Seyal capability scopes
upstream harness compatibility/probe rules
configuration schema reference
```

The manifest is descriptive, not proof. Runtime capability support remains `declared | probed | observed`.

### Discovery and installation

Initial design supports explicit user/system installation locations and explicit configuration. Repository content may request or recommend an adapter by ID/version, but **opening a repository must never execute an uninstalled/untrusted adapter automatically**.

A future registry/package index is optional convenience, not protocol authority. The protocol and conformance suite remain usable for adapters distributed by GitHub release, package manager, organization tooling or local path.

Adapter selection order must be deterministic and inspectable. Duplicate adapter IDs from multiple locations are an error unless an explicit configured precedence resolves them.

### Trust and authorization

Installing/enabling an external adapter is an explicit trust decision. `required Seyal capability scopes` describe what Seyal will authorize through its own APIs; they do **not** claim that the operating system sandbox restricts everything the adapter process can access as the user.

Keep these concepts separate:

```text
Seyal capability authorization
≠ adapter process OS sandbox
≠ harness/provider authorization
```

A stronger macOS sandbox/container policy may be added after platform-specific R&D, but the protocol must remain secure when the adapter is merely an untrusted peer process with only bounded Seyal IPC access.

### Adapter failure semantics

External adapter exit/crash must not kill a still-live harness `TerminalExecution`. The Runtime marks structured capability loss, drains/reconciles durable control state, and follows the normal capability-dependent recovery path. Restarting an adapter must not create a new `Attempt` unless actual work is retried.

No adapter can claim that its own restart restores a lost PTY/process.

## Capability families

Minimum protocol vocabulary:

```text
lifecycle.discover/start/resume/cancel
session.identity/reconnect
presentation.raw-terminal
stream.structured-events
interaction.approval/question
operation.tool-call
artifact.file/diff
config.model/provider/permission
usage.tokens/cache-tokens/cost
integration.mcp/hooks
```

Each capability records:

- semantic version;
- `declared | probed | observed` evidence;
- adapter/upstream version;
- limitations (for example resume only after process exit, or model switch experimental).

Unknown capability versions fail closed for control operations but remain inspectable.

## Lifecycle mapping

```text
Seyal AgentRun
  CREATED
    → STARTING
    → RUNNING
    → WAITING_ATTENTION? ↔ RUNNING
    → CANCELLING?
    → TERMINATED

TERMINATED does not imply successful Outcome.
```

An adapter losing its structured channel produces `structured_channel_lost`; it does not invent a run failure if the underlying TerminalExecution/process is still alive. Recovery options are capability-dependent:

1. reconnect to the existing structured server/session;
2. resume the upstream session after process loss;
3. continue raw-only with reduced capabilities, explicitly surfaced;
4. mark interrupted when no safe recovery exists.

A GUI reconnect is not an agent retry.

## First adapter recommendation

Implement after R&D in this order:

1. **Codex App Server adapter vertical** — strongest explicit bidirectional JSON-RPC integration surface for threads/turns/events/approvals while retaining Codex TUI separately for raw use.
2. **Claude Code adapter** — high-value second adapter using supported structured CLI/hooks while preserving real TUI semantics.
3. **Gemini ACP adapter** — ACP is strategically attractive because it is a standardized JSON-RPC client protocol; stabilize against the upstream capability/version surface before making it a foundation dependency.
4. **OpenCode adapter** — valuable OpenAPI/SSE reference implementation and conformance test for a server-shaped harness.

After two first-party adapters prove the contract, add one small **out-of-tree reference adapter fixture** to prove the public SDK/discovery process does not depend on private in-repository APIs.

This ordering is about integration clarity, not a product endorsement or model-quality ranking.

## Adapter conformance suite

Every adapter must be tested against retained fixtures/probes for:

- manifest/schema/protocol-version validation;
- deterministic discovery and duplicate-ID handling;
- untrusted/uninstalled repository adapter request does not auto-execute;
- adapter crash/restart while underlying TerminalExecution remains alive;
- bounded/oversized IPC behavior;
- version/capability discovery;
- launch with explicit working directory/environment policy;
- upstream session ID capture;
- resume without creating a new Seyal Attempt;
- cancel/abort and terminal child cleanup semantics;
- structured tool start/end and malformed/unknown events;
- approval allow/deny and timeout;
- artifact/diff references;
- model/provider selection when advertised;
- MCP availability when advertised;
- token/cache/cost fields only when upstream actually reports them;
- structured-channel loss while the raw process remains alive;
- duplicate/out-of-order events and idempotent replay;
- upstream version with an unknown capability.

## Security

- Harness and external adapter processes are untrusted code with explicitly granted Seyal capabilities.
- Structured events are data, never instructions to Seyal's terminal engine.
- Approval IDs must bind to exact run/session/action and expire; never approve by matching terminal text.
- Adapter authentication material is outside event payloads/logs/caches.
- MCP/tool capabilities are separately authorized; discovering a tool does not authorize execution.
- External hook/adapter output is bounded and schema validated before allocation proportional to claimed payload size.
- Repository configuration may reference adapter IDs but cannot silently install/enable/execute code.
- Adapter manifest permissions are not represented as an OS sandbox guarantee.

## Performance

No harness adapter callback, event parser, hook, telemetry sink or agent process is on PTY→VT→TerminalState→damage→render. Event ingestion uses bounded asynchronous queues with overload policy; dropping noncritical telemetry is preferable to terminal backpressure. Durable control/accounting events use bounded persistence/replay/backpressure semantics at the **AgentRun**, never terminal rendering.

## OSS/commercial ownership

**OSS:** protocol, capability negotiation, public adapter SDK, manifest/discovery rules, external-adapter process contract, conformance suite and useful first-party local adapters.  
**Commercial:** managed lifecycle across organization fleets, curated/approved adapter catalogs, organization policy/credentials/analytics, hosted adapters/workers. Commercial code consumes the OSS protocol.

A private organization adapter can consume the OSS SDK without becoming commercial-repository code or requiring upstream inclusion.

## Success / kill criteria

Pass when:

- two materially different first-party harnesses can map lifecycle, session identity, cancellation, approvals and structured events without vendor fields leaking into core identities;
- one out-of-tree adapter can be discovered and pass conformance without linking to Seyal Runtime internals;
- an adapter crash does not kill a live terminal execution;
- untrusted repository content cannot cause arbitrary adapter execution;
- raw TUI remains untouched.

Reject/rework if integration requires:

- parsing arbitrary terminal text for correctness-critical events;
- a second PTY/session to represent one logical run;
- loading arbitrary third-party native code into the authoritative Runtime;
- blocking terminal I/O on adapter work;
- claiming unsupported resume/usage/cost semantics.

## ADR/spec required before implementation

- Agent Platform Ownership/Lifecycle ADR (shared after #51–#57 stabilize).
- `HarnessAdapter` capability + external adapter process/discovery specification with conformance semantics.
- External adapter trust/authorization/threat-model specification.
- First Codex adapter behavior spec and threat review.
