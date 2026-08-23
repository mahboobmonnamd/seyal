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

For the affected boundary identify assets, actors/trust levels, entry points, authorization, input validation, resource limits, lifetime/cleanup, failure modes, data exposure, privilege changes, and logging/audit needs.

Prefer least privilege, explicit ownership, bounded inputs/resources, versioned protocols, fail-closed authorization, and canonical state controlled by a single authority.

## Terminal-specific invariants

- Local clients cannot mutate canonical terminal state directly.
- Malformed terminal/protocol input cannot panic, corrupt state, or allocate without bounds.
- Slow/dead clients cannot backpressure PTY→VT progress.
- Secrets, clipboard and file-related features require explicit trust analysis before implementation.
- Enterprise/licensing/cloud checks never become synchronous dependencies of PTY/VT/render hot paths.

## M001 local IPC gate

`ADR-002-M001-READINESS-CORRECTIONS.md` and the M001 readiness amendment require a focused review before Pass 5 acceptance covering socket permissions/discovery, same-user authorization, controller/observer authority, protocol bounds, shared-memory ownership/lifetime/generation validation, stale identifiers, crash cleanup and denial-of-service limits.

## Reporting vulnerabilities

Do not ask users to disclose exploitable vulnerabilities, secrets, credentials or customer data in public Issues. The repository should configure a private security-reporting path before public launch. Until that owner-managed channel exists, security Issue forms are for non-sensitive security engineering/R&D only and must direct suspected vulnerabilities to the repository's private reporting mechanism once configured.
