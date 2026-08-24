# Seyal Workflow Extension Platform — R&D Direction

**Document:** SEYAL-WORKFLOW-EXTENSION-PLATFORM-RD-001  
**Date:** 2026-08-24  
**Status:** Proposed / deferred  
**Implementation gate:** Do not implement before M001 Pass 5 is complete **and** the required workflow UI primitives are accepted and demonstrably available. This document does not authorize runtime/UI implementation.

## 1. Decision summary

Seyal should pursue a workflow-extension platform that composes terminal execution, operational state, agents, artifacts, approvals and structured inspectors around real tasks.

The target is **not** "a library of prebuilt dashboards" and not "automatic pane rearrangement whenever a familiar CLI command is typed".

The stronger model is:

```text
real user task
→ workflow activation
→ existing TerminalExecution remains authoritative
→ structured providers/adapters observe or query relevant systems
→ Seyal composes task-focused operational surfaces
→ user retains a normal shell throughout
```

Recommended initial validation domains:

1. Kubernetes production incident / debugging.
2. Agentic software development.
3. A third structurally different domain, preferably Git/CI or Terraform, before stabilizing any public extension contract.

## 2. Why this is valuable

### 2.1 Product advantages

- Converts Seyal from a rectangular terminal surface into an execution workspace for real operational tasks.
- Reduces context switching among terminal tabs, dashboards, log viewers, agent UIs, Git tools and approval systems.
- Helps beginners by presenting the same diagnostic context senior engineers typically assemble manually.
- Helps senior engineers by making repeated incident/debugging layouts faster and more consistent without hiding the underlying shell.
- Makes Seyal's existing pane model, Blocks, inspectors, artifacts and Attention Stack work together rather than as unrelated features.
- Creates a natural extension ecosystem: first-party and third-party providers can expose structured state without owning terminal execution.
- Fits the OSS product principle: local, generic, portable workflow primitives can live in OSS while managed org-wide automation/policy can remain external/commercial.

### 2.2 Engineering advantages

- Operational surfaces do not require one PTY per visual region.
- The terminal hot path stays independent of workflow computation.
- Structured providers can expose typed state rather than requiring fragile parsing of rendered terminal text.
- A provider can fail, restart or disappear without invalidating the underlying `TerminalExecution`.
- The same provider model can serve human workflows and agent workflows.

## 3. Main risks and disadvantages

### 3.1 Scope explosion

Kubernetes, Terraform, Docker, Git, CI, databases and agents all have deep domain-specific behavior. A naive "support everything" program would distract from terminal correctness and UI completion.

Mitigation:

- validate with only 2–3 reference workflows;
- keep the extension model unstable until repeated patterns are proven;
- reject integrations that require special-case terminal ownership.

### 3.2 Becoming a worse version of existing specialist tools

K9s, native cloud consoles, GitHub/GitLab UIs, Terraform tooling and agent UIs already solve parts of these problems well.

Seyal should not compete by cloning their feature depth.

The unique value must be **cross-tool task composition around live execution**, for example:

```text
Kubernetes incident
= shell + unhealthy workloads + selected logs + events + relevant resource state + attention/approval
```

rather than "K9s rebuilt inside Seyal".

### 3.3 Incorrect automatic behavior

Automatically changing layout or intercepting normal `kubectl`, `git`, `terraform` or agent commands can surprise expert users and break muscle memory.

Mitigation:

- normal CLI commands always retain normal shell semantics;
- workflow activation is explicit or user-confirmed;
- command recognition may suggest a workflow but must not silently hijack execution;
- raw terminal behavior remains available when integration metadata is absent or untrusted.

### 3.4 Performance and hot-path contamination

Watchers, log streams, semantic extraction and remote APIs can become expensive.

Mitigation:

- no workflow/provider work synchronously gates PTY read, VT mutation, damage or rendering;
- watchers are bounded and cancellable;
- hidden surfaces release expensive render resources;
- backpressure policy is explicit;
- providers publish snapshots/deltas to workspace-owned state asynchronously.

### 3.5 Security and trust

Infrastructure workflows may expose production credentials, cluster state, logs, secrets, approvals and privileged actions.

Mitigation:

- read and action capabilities are separate;
- typed actions require capability authorization;
- high-risk actions surface explicit intent and target context;
- providers do not gain arbitrary access to other executions/workspaces;
- secret/password interaction remains terminal-native when semantic handling is unsafe;
- remote and enterprise policy can constrain provider capabilities without entering terminal hot paths.

### 3.6 Extension compatibility burden

Once a public provider SDK is stable, Seyal inherits compatibility obligations.

Mitigation:

- do not publish a stable provider ABI/API after only one integration;
- validate internal interfaces across Kubernetes + agentic development + one third domain;
- version capability contracts explicitly;
- prefer message/domain compatibility over in-process ABI stability.

## 4. Recommended user experience model

### 4.1 Explicit workflow entry points

Examples:

```text
Command palette:
  Kubernetes: Investigate unhealthy workload
  Kubernetes: Watch rollout
  Git/CI: Investigate failing build
  Agent: Open development control surface
```

CLI entry points may also exist later:

```text
seyal workflow k8s-incident
seyal workflow agent-dev
```

Normal commands remain normal:

```text
kubectl get pods
terraform plan
git status
```

Seyal may observe context and offer a non-disruptive suggestion such as:

```text
CrashLoopBackOff detected — Open Incident Workspace
```

but it should not rearrange the workspace without explicit user action or a previously configured rule.

### 4.2 Kubernetes reference workflow

Suggested composition:

```text
Workspace / Tab
├─ Main TerminalPane
│    └─ real TerminalExecution
├─ OperationalPane: unhealthy workloads
├─ OperationalPane: selected workload logs/events
├─ InspectorPane: resource / node / rollout details
└─ Attention Stack: action/approval/failure items
```

Important rule: only surfaces that truly require interactive terminal semantics receive a PTY.

### 4.3 Agentic development reference workflow

Possible surfaces:

- agent/run status;
- current task/work item;
- changed files and diff summary;
- test/check status;
- token/context/cost metrics when reported by the provider;
- subagent or parallel-task status;
- worktree/repository ownership;
- tool/action activity;
- approvals/questions through Attention;
- main shell remains normal and directly usable.

This should consume the provider-neutral agent model rather than hard-coding one vendor UI.

## 5. Proposed architecture direction

```text
                    Workspace
                       │
              WorkflowInstance
                       │
      ┌────────────────┼────────────────┐
      │                │                │
Provider/Adapter   Provider/Adapter   Provider/Adapter
(Kubernetes)       (Git/CI)          (Agent Harness)
      │                │                │
      └────────────── typed events/state ─────────────┘
                       │
              Workspace Domain State
                       │
       ┌───────────────┼─────────────────┐
       ▼               ▼                 ▼
OperationalPane   InspectorPane      Attention

TerminalExecution remains separately authoritative:
PTY → VT → TerminalState → damage → renderer
```

### 5.1 Ownership rules

- `TerminalExecution` continues to own PTY, child lifecycle and authoritative terminal state.
- `WorkflowInstance` owns workflow lifecycle/state, not terminal infrastructure.
- Providers own only their integration-specific handles/cache/watch lifecycle.
- Workspace/domain state owns normalized workflow/provider state needed for presentation and recovery.
- UI panes are projections of workspace/provider/terminal state.
- Providers never synchronously gate terminal output or input.

### 5.2 Provider capability examples

A future internal interface might expose capabilities such as:

```text
discover_context
snapshot
watch
query
stream_logs
publish_artifact
publish_attention
request_action
execute_typed_action
health
shutdown
```

This is deliberately illustrative, not a stable API.

## 6. Adoption strategy

### Phase A — document only

Current phase.

- record product value, constraints and risks;
- align with terminal/runtime/UI ownership;
- do not add workflow runtime code;
- do not add public plugin APIs;
- do not alter current milestone scope.

### Phase B — post-gate reference workflows

After the implementation gate is satisfied:

1. Build Kubernetes incident workflow using an internal unstable provider interface.
2. Build agentic-development workflow using the same core model.
3. Measure CPU, memory, watch/log backpressure and UX overhead.
4. Verify failure of a provider cannot affect terminal responsiveness.
5. Add a third integration to test abstraction quality.

### Phase C — extract extension model

Only after multiple integrations converge:

- define stable domain types and lifecycle;
- define capability negotiation;
- define isolation model;
- decide in-process vs out-of-process extension packaging;
- define compatibility/versioning;
- publish SDK/documentation only when the seam is proven.

## 7. Implementation readiness gates

Implementation must not start until all are true:

- [ ] M001 Pass 5 is complete.
- [ ] Required UI foundation for panes/inspectors/attention is accepted and demonstrably usable.
- [ ] Workflow work is placed in a later milestone rather than expanding M001.
- [ ] Terminal hot-path independence is preserved by design.
- [ ] Provider state ownership and lifecycle are defined.
- [ ] Security/threat model covers local and remote providers.
- [ ] Resource/backpressure limits are defined.
- [ ] Kubernetes and agent-development UX mocks/specs exist.
- [ ] Public API/SDK stabilization is explicitly out of scope for the first workflow implementation.

## 8. Recommended decision

**Proceed with R&D and later implementation.**

The concept is strategically strong because it uses Seyal's unique combination of persistent terminal execution, structured workspace state, agents, Blocks, inspectors and attention. The main failure mode would be turning Seyal into a bundle of domain dashboards or allowing provider logic to pollute terminal correctness/performance.

The first implementation should therefore validate the workflow model, not attempt to become a complete DevOps platform.
