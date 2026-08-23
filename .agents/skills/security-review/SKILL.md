---
name: security-review
description: Threat-oriented review for Seyal process execution, PTY, IPC, SSH, clipboard, OSC, files, secrets, agents, remote clients and enterprise policy boundaries.
---

# Security review

Read `docs/engineering/SECURITY.md`, the active Issue/spec, and relevant architecture/ADRs.

1. Identify assets, actors/trust levels and entry points.
2. Map authorization/ownership for every mutating operation.
3. Check input length/bounds/version validation and malformed-input behavior.
4. Check permissions, privilege transitions, resource limits, lifetime and cleanup.
5. Check confidentiality of terminal contents, files, environment, clipboard and secrets.
6. Check denial-of-service paths and whether slow/dead clients can backpressure terminal progress.
7. Check that canonical PTY/VT/state ownership cannot be mutated by clients/agents/policy layers.
8. Add targeted security tests/fuzz/failure cases and document residual risk.
9. Escalate architecture changes through the `architecture-change` skill.

For M001 local attachment, explicitly cover the socket/shared-memory security gate in the canonical `docs/milestones/MILESTONE-001.md`.
