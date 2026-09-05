# Seyal Engineering Quality Baseline

**Status:** Thin index for M002+ — **not** a second constitution.  
**Authority order:** Product & Engineering Constitution → accepted architecture/ADRs → specs → milestone → Ready Issue → this index → procedures linked below.

Use this document to find the existing gate that already owns a concern. Do not invent style preferences here. If a linked authority and this index disagree, the linked authority wins; amend that authority in its own Issue/PR.

## Canonical entry points

| Concern | Authority |
|---|---|
| Agent/workflow constitution | `AGENTS.md` |
| Development flow / one-Issue worktree/PR | `docs/engineering/DEVELOPMENT.md`, `docs/engineering/ISSUE-PROTOCOL.md` |
| Repository layout / module ownership | `docs/engineering/REPOSITORY-STRUCTURE.md` |
| Testing / TDD / native UI tests | `docs/engineering/TESTING.md` |
| Performance / hot-path / benches | `docs/engineering/PERFORMANCE.md` |
| Security / threat review | `docs/engineering/SECURITY.md` |
| CI / required checks | `docs/engineering/GITHUB-WORKFLOW.md` |
| OSS ↛ commercial boundary | `docs/engineering/OSS-COMMERCIAL-BOUNDARY.md` |
| Adversarial Runtime/lifecycle review | `docs/engineering/RUNTIME-ADVERSARIAL-REVIEW.md`, `AGENTS.md` § Adversarial lifecycle |
| Agent tooling / skills | `docs/engineering/AGENT-TOOLING.md` |

## Domain index (pointers only)

### Ownership and module cohesion

- One authoritative `TerminalState` per `TerminalExecution`; GUI never mirrors a second VT/grid authority — `AGENTS.md`.
- Prefer single-responsibility modules; ~500–700 handwritten lines is a cohesion review trigger; >1000 lines needs PR justification — `AGENTS.md` § Module cohesion.
- Structural refactors must not add hot-path IPC/copies/locks — `AGENTS.md`, `PERFORMANCE.md`.

### Public API / ABI / FFI

- Coarse C-compatible prepared-frame boundary; no per-cell language round-trips — `DEVELOPMENT.md`, `REPOSITORY-STRUCTURE.md`.
- Inventory `unsafe`/FFI with soundness, panic/unwind policy, and ownership proofs — Pass 10 code-quality protocol `docs/engineering/M001-PASS10-CODE-QUALITY-REVIEW.md` §5.1 (method retained for later milestones).

### Errors and panic policy

- Malformed terminal/protocol input must not panic, corrupt state, or allocate without bounds — `SECURITY.md`.
- FFI/C ABI linked into Swift: never unwind across the bridge (`RUSTFLAGS` panic=abort on native link path) — `scripts/build-macos.sh` / client FFI docs.

### Concurrency and lifecycle

- Orthogonal-state, termination, level-trigger progress, retry, inverse-regression, persistent-failure, and late-fix restart rules — `AGENTS.md`, `RUNTIME-ADVERSARIAL-REVIEW.md`.
- Green CI proves only represented behavior; unrepresented states need coverage or impossibility proof — `AGENTS.md`.

### Resources and hot path

- No avoidable synchronous IPC/JSON/serialization/agent/persistence/cloud/licensing/telemetry/Lua/Block semantics on canonical PTY→VT progress — `PERFORMANCE.md`, `scripts/check-hot-path.py`.
- Register new/renamed hot-path functions in the same PR; do not silently drop registry entries — `PERFORMANCE.md`.

### Swift / Metal

- Metal is the first production macOS terminal renderer; no temporary text renderer or temporary production VT path — `AGENTS.md`.
- Production presentation: DisplayLink → drawable → encode → present → commit; label GPU/presentation proxies honestly — `PERFORMANCE.md`, Pass 6 evidence.
- Resource lifetime: do not CPU-write textures/atlases while GPU samples them (in-flight deferral) — Pass 10 Metal residual #786/#789 lesson.

### TDD, fuzz, and benches

- Test/fixture first for core behavior; never weaken a valid test to pass implementation — `TESTING.md`, `ISSUE-PROTOCOL.md`.
- Fuzz: registry ≠ campaign; `ci-smoke` alone cannot score milestone fuzz `PASS`; need `nightly-campaign` / `controlled-campaign` or explicit `N/A` — `TESTING.md`, `M001-FUZZ-EVIDENCE.md`, Pass 10 §6.9.
- Legacy Candidate-B shared-projection comparator rows are not production fuzz coverage — `TESTING.md`.
- Foundation `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0` benches are class `CI` only; headed presentation proof requires DisplayLink=1 or equivalent headed samples — `PERFORMANCE.md`, Pass 10 validation §5.1.

### Security

- Threat review triggers and terminal-specific invariants — `SECURITY.md`.
- Same-user UDS trust, Observer/Controller auth, bounds/version validation — local-attachment / Runtime specs + `SECURITY.md`.

### Dependencies and CI

- Prefer removing unused deps; challenge duplicates that add hot-path or supply-chain cost — Pass 10 code-quality protocol § dependency review.
- Required PR checks and workflow pinning — `GITHUB-WORKFLOW.md`.
- Canonical commands: `make bootstrap|build|test|check|bench` — `AGENTS.md`, `DEVELOPMENT.md`.

### ADR triggers and docs

- ADR when authority/ownership, PTY lifecycle, VT semantics, renderer boundary, process/thread model, IPC/protocol, persistence, Block semantics, headless/embed, security boundary, public API/ABI, or OSS/commercial boundary changes — `ISSUE-PROTOCOL.md`.
- Never amend an ADR inside an implementation PR — `AGENTS.md`, `DEVELOPMENT.md`.
- Documentation impact re-assessed before Done; site docs validated with `make docs-check` / `make docs-build` when changed — `DEVELOPMENT.md`, `ISSUE-PROTOCOL.md`.

### OSS ↛ commercial

```text
seyal-commercial → pinned Seyal OSS
Seyal OSS ↛ proprietary code
```

- Formalized in `OSS-COMMERCIAL-BOUNDARY.md` and ADR-003. Public Seyal must not learn commercial SKUs/plans.

### Independent review

- Core/high-risk work requires independent review; implementers do not self-approve — `AGENTS.md`, `DEVELOPMENT.md`.
- Milestone closeout: independent Phase 2 validation on a frozen head; evidence honesty over substitution — Pass 10 validation protocol.

## M001 carry-forward lessons (proven)

These are distilled from Pass 10 disposition; they do not override the linked authorities above.

1. **Status drift** — Issue/PR/docs “Done” text is not evidence. Keep #727/#5 (and future milestone owners) aligned with criterion ledgers; correct premature closeout claims immediately (#784/#787).
2. **Cohesion deferrals** — Large-file / cohesion follow-ups (#765–#768 class) may be parked post-milestone only when explicitly dispositioned; do not silently grow god files.
3. **Candidate-B gating** — Comparator/legacy paths are not production acceptance evidence unless the governing pass explicitly says so.
4. **Issue-body hygiene** — Ready Issues must keep measurable acceptance and current dependency truth; stale blocked-by/Done checklists mislead agents.
5. **Hot-path registry** — New production hot paths must land with `check-hot-path.py` registration in the same PR (F-004 lesson).
6. **Fuzz grade honesty** — Do not cite CI smoke or registry syntax green as milestone fuzz `PASS`.
7. **Evidence class honesty** — Label `CI` vs `controlled-host` vs `PLATFORM_LIMITED`; never cite display-link-off CI as headed presentation proof; do not call socket-loss soak “GUI crash.”
8. **RSS gate honesty** — Machine gate is whatever the accepted calibration says (`CLIENT_RSS_KIB = 1536` for Pass 9/10); do not invent soft gates in closeout prose.
9. **Packaging vs product** — Diagnostic Release ad-hoc codesign (`SEYAL_CODESIGN_IDENTITY=-`) is for local/CI benches; distributable Release still needs an Apple-issued identity. Host automation timeouts are not product assertion failures when protocol substitution rules apply.
10. **Finding-set freeze** — Late milestone closeout: freeze the finding set; no Issue factory without mandatory FAIL/INCONCLUSIVE, amendment of a frozen finding, or a new BLOCKING product defect.

## How to use this baseline in M002+

1. Open the linked authority for the domain you are changing.
2. If the authority is silent or contradictory, stop and use `architecture-change` / `issue-refinement` — do not extend this index into novel law.
3. Carry the M001 lessons as review checklists, not as substitute acceptance criteria.

## Out of scope for this document

- Rewriting `AGENTS.md` or inventing a parallel constitution
- Product feature requirements (those stay in specs/milestones)
- Commercial-only gates (those stay in `seyal-commercial`)
