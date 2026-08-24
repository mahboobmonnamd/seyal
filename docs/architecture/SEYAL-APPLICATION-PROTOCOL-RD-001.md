# Seyal Application Protocol / SDK — R&D Direction

**Document:** SEYAL-APPLICATION-PROTOCOL-RD-001  
**Date:** 2026-08-24  
**Status:** Proposed / deferred  
**Implementation gate:** Do not implement a stable public protocol or SDK before M001 Pass 5 is complete, the required UI/workflow foundations exist, and at least three real integrations have validated the abstraction.

## 1. Decision summary

Seyal should eventually expose a public application integration contract, but it should **not** be framed as a generic "Shell API".

The shell is not one uniform API surface. Bash, zsh, fish and remote shells differ; terminal escape protocols solve only some presentation problems; terminal-multiplexer APIs mostly manipulate windows/panes rather than expressing semantic execution state.

Recommended direction:

```text
Seyal Application Protocol
+
Seyal SDKs
+
provider/workflow capability model
```

The contract should allow CLI tools and agent harnesses to progressively enhance themselves inside Seyal while remaining fully functional in ordinary terminals.

## 2. Goals

A CLI/application should be able to communicate structured intent/state such as:

- execution identity and lifecycle;
- progress/status;
- structured logs/events;
- artifacts and diffs;
- attention/questions/approvals;
- provider capabilities;
- workflow context;
- resource state;
- safe typed actions;
- metrics such as duration/token/cost when the application owns that data.

Example:

```text
my-deploy-tool deploy production
```

Outside Seyal:

```text
normal ANSI/TTY behavior
```

Inside Seyal, optionally:

```text
normal terminal execution
+
structured deployment status
+
artifact links
+
health/watch surfaces
+
attention/approval items
```

The CLI must never require Seyal in order to work.

## 3. Pros

### 3.1 Ecosystem leverage

Third-party developers can make their CLI tools Seyal-native without Seyal hard-coding every product.

### 3.2 Better than terminal scraping

Typed state is more reliable than parsing rendered terminal text, prompts or ANSI output.

### 3.3 Progressive enhancement

A single CLI remains portable across Seyal, SSH, CI and conventional terminals.

### 3.4 Shared model for first-party and third-party integrations

The same conceptual contract can serve:

- Kubernetes adapters;
- Git/CI integrations;
- Terraform/deployment tools;
- agent harnesses;
- custom internal company CLIs.

### 3.5 Strong OSS ecosystem fit

A portable, generic, local integration protocol is a good OSS boundary. Commercial consumers can layer managed policy, fleet orchestration, RBAC and organization-wide services on top without reversing the dependency.

## 4. Cons and risks

### 4.1 Premature standardization

The largest risk is designing the public API before real integrations reveal the correct abstractions.

Mitigation: keep the first interface internal/unstable and derive the public protocol only after multiple integrations.

### 4.2 Security

A process writing bytes to a PTY must not automatically gain privileges to manipulate other panes, run arbitrary commands, read secrets or approve production actions.

Mitigation:

- capability negotiation;
- authenticated/trusted integration channel;
- typed permissions;
- explicit action authorization;
- target scoping to workspace/execution/provider identity;
- no security-sensitive authority inferred from untrusted terminal output.

### 4.3 Compatibility burden

Once external developers depend on the SDK, schema changes become expensive.

Mitigation:

- versioned protocol;
- capability negotiation;
- backward-compatible additive evolution where possible;
- no stable ABI promise for in-process plugins unless explicitly chosen later.

### 4.4 Remote ambiguity

A CLI may execute locally, inside SSH, nested SSH, containers, tmux or remote runtime environments. The process's trust and ability to reach Seyal differs in each case.

Mitigation: treat transport and trust as separate design problems. A local direct channel should not imply that remote applications automatically receive the same authority.

### 4.5 Performance

High-volume event/log APIs can become another serialization/IPC hot path.

Mitigation:

- protocol is additive and asynchronous to terminal I/O;
- high-volume raw terminal bytes continue through PTY/VT path;
- structured event channels are bounded, rate-limited and optional;
- binary/framed transport may be used where justified; JSON should not be assumed for hot paths.

## 5. Relationship to shell integration

Shell integration protocols are useful for identifying prompt/command/output boundaries and related metadata. Seyal should support appropriate terminal/shell integration for Blocks, but that is not the same as the proposed Application Protocol.

Conceptually:

```text
Shell integration
→ shell/prompt/command boundary metadata

Seyal Application Protocol
→ application/workflow/domain state and typed actions
```

Both may coexist.

## 6. Proposed conceptual layers

```text
Application / CLI / Agent Harness
             │
             ▼
       Seyal SDK facade
             │
             ▼
   Capability negotiation
             │
             ▼
   Seyal Application Protocol
             │
   ┌─────────┼──────────┬───────────┐
   ▼         ▼          ▼           ▼
Execution  Artifact   Attention   Workflow/Resource
state      state      actions     state
```

This does not replace terminal execution:

```text
stdin/stdout/stderr/PTY
→ normal terminal path
```

The protocol is a side channel for structured semantics.

## 7. Capability model

Illustrative capability classes:

```text
execution.publish_status
execution.publish_progress
artifact.publish
artifact.update
attention.publish
attention.resolve
resource.publish_snapshot
resource.watch
workflow.publish_state
action.request
action.receive_result
metrics.publish
```

Capabilities should be scoped and negotiated. A client without a capability falls back gracefully.

Example handshake:

```text
Client → hello(protocol versions, requested capabilities, identity metadata)
Seyal  → accepted version + granted capabilities + limits
Client → typed events/actions within granted scope
```

## 8. Trust model direction

At least three trust classes should be considered:

### Local trusted integration

An application started directly by Seyal or explicitly connected through a protected local endpoint.

### Local untrusted terminal output

Bytes printed to the terminal. These may carry benign presentation metadata, but must not obtain privileged action authority merely by emitting escape sequences.

### Remote integration

Application running through SSH/container/remote host. Requires an explicit forwarding/authentication model; never inherit local privileges implicitly.

## 9. SDK direction

Potential language SDKs after protocol validation:

- Rust;
- Go;
- Python;
- TypeScript/Node;
- minimal C ABI only if real demand justifies it.

SDK responsibilities:

- transport discovery;
- version/capability negotiation;
- typed models;
- batching/backpressure;
- fallback/no-op behavior outside Seyal;
- safe lifecycle/cleanup.

The SDK must not force dependency on Seyal-specific business logic into a CLI's core execution path.

## 10. Adoption plan

### Phase A — no public API

Use internal provider/workflow contracts while building reference workflows.

### Phase B — integration evidence

Validate against at least:

1. Kubernetes workflow/provider.
2. Agentic-development harness/provider.
3. Git/CI, Terraform or another materially different integration.

Record where the common abstraction succeeds and where domain-specific escape hatches are needed.

### Phase C — protocol prototype

Create an experimental versioned protocol behind a feature flag. Test:

- local process discovery;
- capability negotiation;
- malformed/hostile clients;
- disconnect/reconnect;
- backpressure;
- version skew;
- remote/nested execution boundaries;
- SDK fallback outside Seyal.

### Phase D — public v1

Only publish v1 after:

- protocol semantics are stable across multiple integrations;
- threat model is reviewed;
- resource limits are measured;
- documentation/examples exist;
- compatibility policy is defined;
- no terminal hot-path dependency exists.

## 11. What not to do

Do not:

- define "one shell API" pretending bash/zsh/fish are equivalent;
- require CLIs to use Seyal to remain functional;
- parse arbitrary terminal output as trusted actions;
- expose unrestricted workspace/process control to any child process;
- commit to stable public schemas based on one integration;
- make PTY/VT/rendering synchronously depend on the application protocol;
- use this protocol as a replacement for correct terminal standards.

## 12. Implementation readiness gates

- [ ] M001 Pass 5 complete.
- [ ] UI/workflow foundation accepted and demonstrably available.
- [ ] At least three real integrations exercised an internal common model.
- [ ] Capability/security model defined.
- [ ] Local vs remote trust boundaries defined.
- [ ] Versioning/compatibility policy defined.
- [ ] Backpressure/resource limits measured.
- [ ] Outside-Seyal fallback proven.
- [ ] Terminal hot path remains entirely independent.

## 13. Recommended decision

**Recommend the concept, reject the original "Shell API" framing.**

The long-term opportunity is a Seyal Application Protocol/SDK for progressive enhancement of CLI and agent applications. The public contract should be extracted from proven workflow/provider integrations rather than invented upfront.
