# Milestone 001 — Readiness Amendment 001

**Status:** Required for M001 implementation

**Date:** 2026-08-23

**Authority:** Subordinate to the accepted Seyal foundation architecture and `ADR-002-M001-READINESS-CORRECTIONS.md`. This file adds readiness gates only; it does not expand M001 product scope.

## 1. Block ownership clarification

For M001, `BlockTimeline` and Block metadata are Runtime/workspace metadata keyed by `ExecutionId`.

`TerminalExecution` owns:

```text
ExecutionId
TerminalEndpoint / PTY
child lifecycle
TerminalState
attachment/projection state
```

It does not own Block semantic authority.

Pass 8 must therefore implement the minimum Block model in the `seyal-workspace` logical boundary or equivalent Runtime/workspace metadata boundary, observing execution/history asynchronously.

The PTY → VT → TerminalState → damage path must never synchronously depend on Block mutation.

## 2. Configuration/Lua scope clarification

The broader foundation direction for typed TOML and cold Lua extension remains valid, but M001 does not implement the production configuration or Lua systems.

Only a minimal type/boundary seam may be introduced if required naturally by the M001 implementation. Do not add production Lua, config provenance, policy composition, or general customization during M001.

## 3. Pass 2 — additional VT conformance exit gate

Add to Pass 2 required exits:

- a retained reference/conformance corpus exists for the `SUPPORTED M001` VT subset where authoritative/reference behavior is practical to encode;
- fixture provenance is recorded;
- supported behavior is checked against those reference expectations;
- disagreements are resolved explicitly;
- conformance work does not pull deferred/full VT behavior into M001.

## 4. Pass 5 — additional local security exit gate

Add to Pass 5 required exits:

- focused threat/security review of the Unix-domain control path and shared-memory projection is complete;
- local socket ownership/permissions and same-user authorization are tested;
- Runtime discovery does not trust attacker-controlled filesystem paths;
- malformed/oversized protocol messages are rejected safely;
- shared-memory mapping validates version, bounds and committed generation before consumption;
- shared-memory ownership, permission, lifetime and cleanup behavior is tested;
- stale/reused projection identifiers cannot grant unintended access;
- stalled/crashed clients cannot mutate canonical terminal state or exhaust unbounded Runtime resources;
- controller/observer authority is explicit even though M001 exercises one controller;
- no cloud/licensing/telemetry dependency is introduced into terminal progress.

This does not require enterprise RBAC, SSO, remote internet attach or cloud security in M001.

## 5. Acceptance-gate additions

M001 cannot pass unless all of the following are demonstrated:

### Architecture

- [ ] BlockTimeline authority is Runtime/workspace metadata keyed by `ExecutionId`;
- [ ] `TerminalExecution` does not own Block semantic state;
- [ ] Block observation/mutation cannot block PTY → VT → damage progress.

### Correctness

- [ ] retained reference/conformance fixtures cover the claimed M001 VT subset where practical;
- [ ] reference fixture provenance is recorded;
- [ ] no deferred behavior was promoted merely to satisfy corpus breadth.

### Security

- [ ] local socket ownership/permissions and client authorization are tested;
- [ ] protocol length/bounds/version validation is tested;
- [ ] shared-memory permissions/lifetime/generation validation is tested;
- [ ] malformed or hostile local-client input cannot corrupt Runtime terminal authority;
- [ ] attachment/projection resource usage is bounded against local denial-of-service;
- [ ] focused M001 local Runtime threat review is recorded.

## 6. Scope remains unchanged

This amendment does not add:

```text
full VT
production scrollback/reflow
million-line history
agents/orchestration
cloud/mobile
public embedding API
production Lua
final configuration system
enterprise RBAC/SSO
runtime-crash PTY survival
reboot recovery
```

M001 remains the same production-shaped vertical slice. These gates exist only to remove ambiguity and prevent correctness/security debt from entering the foundation.