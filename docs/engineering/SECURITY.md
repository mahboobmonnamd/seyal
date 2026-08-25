# Security engineering

Security review is required when an Issue crosses trust, process, data or policy boundaries. Security work must not be postponed as generic “later hardening” when the boundary is introduced.

## Review triggers

Use `.agents/skills/security-review/SKILL.md` for changes involving:

- process execution and child lifecycle;
- PTY/ConPTY;
- Unix sockets, shared memory or IPC;
- SSH/remote attach;
- clipboard/OSC/file access;
- terminal-provided links/images/control sequences;
- secrets/credentials/environment data;
- agents/tool execution;
- remote clients;
- enterprise identity/policy/audit boundaries;
- public API/ABI/plugin surfaces.

## Threat review questions

For the affected boundary identify assets, actors/trust levels, entry points, authorization, input validation, resource limits, lifetime/cleanup, failure modes, data exposure, privilege changes and logging/audit needs.

Prefer least privilege, explicit ownership, bounded inputs/resources, versioned protocols, fail-closed authorization and one canonical state authority.

## Terminal-specific invariants

- Local clients cannot mutate canonical terminal state directly.
- Malformed terminal/protocol input cannot panic, corrupt state or allocate without bounds.
- Slow/dead clients cannot backpressure PTY -> VT progress.
- Secrets, clipboard and file-related features require explicit trust analysis before implementation.
- Enterprise/licensing/cloud checks never become synchronous dependencies of PTY/VT/render hot paths.
- A client RenderState cache is derived/disposable and never terminal authority.
- No client runs a second authoritative VT engine to reconstruct server state.

## M001 local IPC gate

ADR-001 now selects compact binary UDS for control/input/lifecycle **and** ordinary terminal-model snapshot/delta presentation. The earlier per-attachment shared-memory grid remains transitional comparator/reference code in PR #106 and is not the intended final production text/grid path.

Pass 5 treats a same-effective-UID peer as part of the same OS-user trust domain; this is not a sandbox boundary. Opening `control.sock` grants no execution authority. An attachment is bound to the authenticated connection that created it, `AttachmentId` is not a bearer capability, observers cannot input/resize, and only one controller may exist for an execution.

The per-user runtime directory is owner-only and `control.sock` is explicitly `0600`. Symlink/non-socket/insecure discovery paths fail closed, and a connectable active endpoint is never unlinked as stale. Local socket descriptors are close-on-exec.

## Binary presentation security

The selected production path must keep presentation data explicitly encoded and versioned. Rust struct layout, pointers, parser internals, mutable canonical grid memory, renderer/GPU objects and AppKit types are forbidden on the wire.

Every client-supplied frame remains bounded before allocation. Integer length/offset/count calculations are checked before use. Unknown/unsupported versions fail according to the protocol contract; malformed framing is connection-fatal where required.

Presentation payloads are server-derived from canonical terminal state. They must not gain input/control authority merely because they are carried on the same UDS connection.

A client generation/cache value is never authoritative. Generation gaps cause bounded resynchronization to current canonical state; the Runtime never replays PTY bytes into a client-owned terminal engine.

## Slow-client and denial-of-service rule

Mandatory control/lifecycle queues remain bounded. Presentation state is coalescible/replaceable and must not accumulate unbounded generation history.

If a client cannot keep up, Runtime may discard obsolete presentation work, mark that client for resync and/or disconnect it according to the protocol contract. One slow or malicious client must not delay another client or PTY -> VT progress.

## Ancillary descriptor handling

Removing the production text-grid shared-memory path does not make ancillary handling optional. Client-to-Runtime M001 frames do not carry descriptors. Production receive logic must continue to use ancillary-aware bounded receive semantics so unexpected `SCM_RIGHTS`, multiple descriptors, malformed ancillary records or truncated ancillary control are detected and rejected rather than silently ignored.

Every complete unexpected descriptor recovered from a rejected message must be closed during bounded cleanup.

The existing shared-projection comparator/tests may still exercise descriptor transfer while that reference code remains in the branch. Such descriptor authority must remain isolated from the selected production text/grid path.

## Future bulk-object boundary

Future large immutable images/graphics/media may use a separate shared-buffer transport such as POSIX shared memory or IOSurface if later evidence justifies it. M001 does not implement that plane.

When introduced, the bulk path requires a fresh explicit threat review covering:

- object identity and authority;
- descriptor/handle ownership;
- read/write protections;
- size and aggregate memory budgets;
- lifecycle/revocation;
- stale handles;
- cross-client isolation;
- malformed metadata;
- GPU/IOSurface access rules;
- remote-versus-local transport differences.

Do not reuse the old per-attachment grid ABI as a de facto graphics protocol without that review.

## Pass-5 security validation

Before PR #106 may merge, verify at least:

- same-UID peer authentication;
- owner-only endpoint permissions and safe stale-socket handling;
- observer/controller authorization;
- connection-bound attachment identity and stolen-ID rejection;
- invalid/nonexistent/finalized execution behavior;
- malformed/oversized/truncated binary frames;
- unexpected/multiple/truncated ancillary descriptor handling;
- bounded receive/control/presentation queues;
- generation-gap/resync behavior;
- slow/dead/killed client isolation;
- reconnect without PTY replay or client VT authority;
- cleanup/resource return to baseline;
- final canonical output delivered before execution teardown;
- fuzzing of production snapshot/delta decode and generation/resync state machine with bounded resources.

Existing shared-memory reader/fd/mapping tests remain valid comparator/reference coverage while that code exists, but they do not substitute for testing the selected production binary presentation path.

## Reporting vulnerabilities

Do not ask users to disclose exploitable vulnerabilities, secrets, credentials or customer data in public Issues. The repository should configure a private security-reporting path before public launch. Until that owner-managed channel exists, security Issue forms are for non-sensitive security engineering/R&D only and must direct suspected vulnerabilities to the repository's private reporting mechanism once configured.
