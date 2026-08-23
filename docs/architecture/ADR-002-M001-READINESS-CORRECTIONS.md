# ADR-002 — M001 Readiness Corrections

**Status:** Accepted

**Date:** 2026-08-23

**Scope:** Documentation consistency and M001 readiness only. This ADR does not redesign Seyal or expand M001 product scope.

**Refines:** `SEYAL-ARCH-FOUNDATION-RD-001.md`, `docs/milestones/MILESTONE-001.md`

**Related rationale:** `R-006`, `R-029`, `R-034`, `R-045`, `R-047`, `R-048`, `R-049`

## Why this ADR exists

The independent M001 readiness review found no restart-class architecture blocker, but it found several wording inconsistencies that should be resolved before implementation so ownership and acceptance gates cannot be interpreted in two ways.

This ADR is authoritative where the older foundation or M001 wording conflicts with the corrections below.

## 1. Foundation acceptance state

The foundation architecture is **Accepted**.

The merge of the foundation architecture PR constitutes acceptance. Any remaining `Proposed for acceptance` wording in `SEYAL-ARCH-FOUNDATION-RD-001.md` is stale metadata and must not be interpreted as an unresolved architecture decision.

M001 implementation must treat the foundation architecture, its rationale, accepted ADRs, and M001 specification as the active authority chain.

## 2. BlockTimeline ownership

### Decision

`BlockTimeline` is authoritative **Runtime/workspace metadata**, keyed by stable `ExecutionId` and owned by the `seyal-workspace` logical boundary.

`TerminalExecution` owns terminal execution infrastructure and canonical terminal semantics. It does **not** own Block semantic authority.

Canonical relationship:

```text
Seyal Runtime
  ├─ Execution registry
  │    └─ TerminalExecution
  │         ├─ TerminalEndpoint / PTY
  │         ├─ child lifecycle
  │         └─ TerminalState
  │              ├─ VT parser / modes
  │              ├─ primary / alternate screen
  │              ├─ logical history identity
  │              ├─ reflow
  │              └─ damage
  │
  └─ workspace metadata
       └─ BlockTimeline
            └─ references ExecutionId + LogicalLineAnchor
```

### Consequences

- A Block never owns PTY, VT, grid, renderer, child process, or copied terminal output.
- `TerminalExecution` may emit tiny asynchronous execution/history signals that workspace metadata consumes.
- PTY → VT → TerminalState → damage progress never waits for Block mutation or semantic processing.
- Block identity and lifecycle may outlive a particular client presentation.
- Flow, Raw and TUI remain views of the same `ExecutionId`.

### Superseded wording

Any older statement saying that `TerminalExecution` "owns BlockTimeline" or includes `BlockTimeline` in its exact ownership list is superseded by this ADR.

The existing ownership matrices that place Block metadata in Runtime/workspace metadata represent the intended architecture.

## 3. Configuration and Lua scope in M001

Foundation decisions remain:

```text
TOML = canonical static configuration direction
Lua  = optional programmable extension through cold typed patches/actions
```

However, **production configuration and Lua implementation are not part of M001**.

M001 may create only the minimum typed seams needed to avoid architectural leakage, and only when naturally required by implementation. It must not implement:

- a production Lua VM;
- general Lua automation;
- the final configuration system;
- config provenance UI;
- policy composition unrelated to the M001 terminal slice.

The explicit M001 non-goals remain authoritative over broader foundation roadmap language.

## 4. VT conformance gate

M001 must not claim supported terminal behavior only from self-authored unit tests.

For every behavior classified `SUPPORTED M001`, Pass 2 must maintain a retained reference/conformance corpus derived from authoritative terminal specifications or independently established terminal-behavior fixtures where practical.

The gate is intentionally bounded to the M001 support matrix. It does not require implementing full VT in M001.

Required properties:

- source/provenance of reference fixtures is recorded;
- supported behavior is checked against expected external/reference semantics where practical;
- regression fixtures are retained in the repository;
- a disagreement between implementation and reference behavior is resolved explicitly rather than silently normalizing the implementation;
- deferred behavior remains deferred and must not expand M001 scope merely to increase conformance coverage.

## 5. Local Runtime IPC/shared-memory security gate

Before M001 Pass 5 is accepted, the local attachment design must receive a focused threat/security review covering at minimum:

- Unix-domain socket location, ownership and permissions;
- same-user client authentication/authorization;
- Runtime discovery without trusting attacker-controlled paths;
- controller versus observer authority;
- malformed or oversized control/protocol messages;
- shared-memory object ownership, permissions, lifetime and cleanup;
- stale/reused shared-memory identifiers;
- bounds/version/generation validation before reads;
- client crash and Runtime crash cleanup behavior;
- prevention of client mutation of canonical terminal state;
- denial-of-service limits for attachment/projection allocation;
- no terminal hot-path licensing/cloud/telemetry dependency.

This is a security validation gate, not a requirement to implement enterprise identity, RBAC, SSO or remote-internet security in M001.

## 6. M001 implementation authority

For M001, read authority in this order:

```text
Seyal Project Instructions / Product & Engineering Constitution
→ accepted foundation architecture
→ foundation rationale
→ accepted foundation ADRs, including this ADR
→ MILESTONE-001.md
→ M001 readiness amendments/gates
→ implementation
```

If an older sentence conflicts with this ADR, this ADR wins for the specific correction above. Unrelated foundation decisions remain unchanged.

## 7. Decisions explicitly not reopened

This correction does not reopen:

- per-user headless Runtime from M001;
- one authoritative `TerminalState` per `TerminalExecution`;
- one real PTY per independent terminal execution;
- no GUI VT/grid mirror;
- Metal as first production macOS terminal renderer;
- no temporary VT or temporary text renderer;
- derived local display projection;
- no synchronous IPC ping-pong in terminal hot paths;
- no process/thread/daemon/render stack per pane by default;
- Blocks as metadata/presentation over canonical execution/history;
- genuine alternate-screen/TUI behavior;
- agents outside terminal authority;
- journaling/history not being live-PTY survival;
- OSS terminal fundamentals remaining license-independent.

## Final effect

After this ADR is merged, the documentation contradictions identified by the pre-M001 architecture review are considered resolved. No additional architecture cycle is required before M001 implementation.