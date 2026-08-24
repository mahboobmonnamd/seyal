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

This ordering is about integration clarity, not a product endorsement or model-quality ranking.

## Adapter conformance suite

Every adapter must be tested against retained fixtures/probes for:

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

- Harness process is untrusted code with user-granted capabilities.
- Structured events are data, never instructions to Seyal's terminal engine.
- Approval IDs must bind to exact run/session/action and expire; never approve by matching terminal text.
- Adapter authentication material is outside event payloads/logs/caches.
- MCP/tool capabilities are separately authorized; discovering a tool does not authorize execution.
- External hook output is bounded and schema validated.

## Performance

No harness adapter callback, event parser, hook, telemetry sink or agent process is on PTY→VT→TerminalState→damage→render. Event ingestion uses bounded asynchronous queues with overload policy; dropping noncritical telemetry is preferable to terminal backpressure. Critical control events may pause the **AgentRun**, never terminal rendering.

## OSS/commercial ownership

**OSS:** protocol, capability negotiation, adapter SDK, conformance suite and useful first-party local adapters.  
**Commercial:** managed lifecycle across organization fleets, hosted adapters/workers, organization policy/credentials/analytics. Commercial code consumes the OSS protocol.

## Success / kill criteria

Pass when two materially different harnesses can map lifecycle, session identity, cancellation, approvals and structured events without vendor fields leaking into core identities, while raw TUI remains untouched.

Reject/rework if integration requires:

- parsing arbitrary terminal text for correctness-critical events;
- a second PTY/session to represent one logical run;
- blocking terminal I/O on adapter work;
- claiming unsupported resume/usage/cost semantics.

## ADR/spec required before implementation

- Agent Platform Ownership/Lifecycle ADR (shared after #51–#57 stabilize).
- `HarnessAdapter` capability protocol specification with conformance semantics.
- First Codex adapter behavior spec and threat review.
