# M001 Pass 10 — Code and Quality Review Ledger

**Owning Issue:** #727  
**Starting review candidate:** `1005bc42397aac485b1aeff08cafd0f67790d969`  
**Status:** PHASE 1 COMPLETE — findings resolved on frozen final validation head `e8431f01c797b57d7b6ee6a9be65706f77c7d789` (2026-09-05); Phase 2 validation authorized  
**Normative protocol:** `docs/engineering/M001-PASS10-CODE-QUALITY-REVIEW.md`

This ledger records the exhaustive M001 milestone-closure code and quality review required before independent final Pass 10 validation.

It is evidence, not a replacement for architecture/specification authority. Historical green CI, prior PR approval, checked Issue boxes and author assertions are inputs only; every production-significant file/module must receive a current review result.

## Review rules

For every production-significant path, record:

- repository path;
- subsystem/module;
- production/test/build/docs classification;
- architecture authority checked;
- correctness result;
- concurrency/resource result;
- security/privacy result;
- performance/hot-path result;
- dead-code/API/dependency result;
- test adequacy result;
- documentation/diagram impact;
- findings and owning Issue/PR;
- final status: `PASS`, `BLOCKED`, or `N/A`.

Findings are classified `BLOCKING`, `IMPORTANT`, or `NON-BLOCKING` under the normative Pass 10 protocol. Required fixes are routed through focused owning Issues/PRs; they are not hidden inside this ledger.

## Frozen-candidate provenance

The Phase 1 review starts from current `master`:

`1005bc42397aac485b1aeff08cafd0f67790d969`

This head contains accepted Passes 1–9 and Pass 9 release qualification. It is the **review candidate**, not yet the final milestone-validation head. Any accepted production change during Pass 10 invalidates affected review evidence and requires a new reviewed/frozen head before final validation.

`master` tip later advanced to `bb6d9cd` via governance-only #747 (ADR amendment PR separation) with **zero** production terminal/runtime/native delta vs the review candidate.

## Review inventory

The review must cover the complete M001-significant surface, including at minimum:

1. Rust production crates and workspace manifests.
2. PTY/child lifecycle and Runtime ownership.
3. VT parser, terminal state/grid, damage and terminfo authority.
4. Candidate-D binary protocol, attachment/controller, projection/display cache and reconnect state.
5. Runtime/workspace Block metadata.
6. Renderer preparation/damage and Rust/native FFI boundaries.
7. Swift/AppKit/Metal application, renderer, input, resize, IME, accessibility and reconnect lifecycle.
8. `unsafe`, FFI, panic/placeholder/error-handling inventory.
9. Cross-module concurrency, queues, locks/atomics, retries, timers and resource cleanup.
10. Security/privacy and local IPC/process boundaries.
11. Hot-path allocation/copy/serialization/syscall/thread-hop review.
12. Tests, fixtures, fuzz targets/corpora and benchmark methodology.
13. Build scripts, toolchains, CI/workflow permissions and reproducibility.
14. Architecture diagrams, ADR/spec consistency, developer/user documentation and stale status wording.
15. OSS/commercial dependency-direction proof.

The repository tree for the starting candidate is the inventory source; directory-level statements are insufficient where production files remain individually unreviewed.

## Phase 1 domain results

| Agent | Domain | Result |
|---|---|---|
| Agent1 | Issues | Stale #706/#725/#730/#654/#73; #663 later work; #5/#727 truth OK → #752 |
| Agent2 | Rust core | Single authority HOLDS; IMPORTANT cohesion/fanout → #759 |
| Agent3 | FFI/unsafe | **BLOCKING** FFI ABI/borrow/panic gaps → #760 |
| Agent4 | macOS/Metal | APPROVE_FOR_VERIFICATION (Low only) |
| Agent5 | Concurrency | **BLOCKING** `TerminationFailed` dead-end → #756; disconnect-during matrix → #757 |
| Agent6 | Perf | Registered hot paths OK; #663 not M001 blocker; display/Metal registry gap → #754 |
| Agent7 | Security | No med/high/critical; OSS↛commercial proven |
| Agent8 | Fuzz/bench | §6.9 INCONCLUSIVE risk without campaign parity → #758 |
| Agent9 | CI | No silent Foundation skip; honesty/repro gaps → #755 |
| Agent10 | Docs | Status/scaffolding drift → #748/#753; Baseline path recommended |

## Findings

### F-001 — stale Pass 9-blocked status in normative Pass 10 protocols

- **Severity:** IMPORTANT
- **Status:** RESOLVED — merged #751
- **Owning Issue:** #748
- **PR:** #751
- **Paths:**
  - `docs/engineering/M001-PASS10-CODE-QUALITY-REVIEW.md`
  - `docs/engineering/M001-PASS10-VALIDATION.md`
- **Evidence:** both documents retain current-status/readiness wording that says Pass 10 is blocked by incomplete Pass 9 even though #719 is closed Done and #5/#727 now reflect the accepted Pass 1–9 lineage.
- **Required resolution:** reconcile only current status/readiness/frontier wording without weakening either review or validation contract. Final validation must remain gated on completion of this Phase 1 review, required fixes/re-review and final exact-head freeze.

### F-002 — reconcile stale Pass 7/9/UI open Issues

- **Severity:** IMPORTANT
- **Status:** RESOLVED — Issue-authority hygiene complete (no production code)
- **Owning Issue:** #752
- **PR:** none required for production; ledger disposition note on #749
- **Evidence (pre-fix):** open Issues (#706/#725/#730/#654/#73) still claimed unfinished Pass 1–9 work even though production landed and #5/#719 record those passes Done; #663 remains later-milestone work; PR #742 still Refs #654 for new UI chrome.
- **Disposition (2026-09-04):**
  - Closed completed/historical with evidence comments: #706 (PR #707 → `4490d89…`), #725 (#719 Done + `docs/evidence/pass9-production-budget-calibration.md`), #730 (PR #731 → `1a1bb43…`), #654 (PR #655 → `c9311ae…`), #73 (PR #74 → `3d98e84…` + `M001-FIRST-UI-DESIGN.md`).
  - Clarified and retitled #663 as post-multipane / later-milestone; kept open; not an M001 Pass 10 blocker; M001 not expanded.
  - Created owning Issue #773 for remaining left-context press-preview work; rehomed open PR #742 from #654 → #773.
- **Required resolution:** ~~close or retarget stale Issues~~ **done.** Active M001 blockers remain #5 and #727 (plus other open Pass 10 finding Issues).

### F-003 — remaining Pass 9 / scaffolding status drift

- **Severity:** IMPORTANT
- **Status:** RESOLVED — merged #763
- **Owning Issue:** #753
- **PR:** #763
- **Related:** extends F-001 / #748 (Pass 10 protocols handled on #751)
- **Evidence:** SPEC/MILESTONE/distribution/scaffolding docs still present Pass 9 / early-pass readiness as current blockers or present-tense unfinished scaffolding after #719 Done.
- **Required resolution:** reconcile remaining status/readiness wording to current #5/#719/#727 truth without weakening Ready/validation contracts.

### F-004 — register display/Metal production hot paths

- **Severity:** IMPORTANT
- **Status:** RESOLVED — merged #761
- **Owning Issue:** #754
- **PR:** #761
- **Evidence:** `scripts/check-hot-path.py` passes for registered VT/Runtime/input functions, but Candidate-D display encode/publish and Metal prepare/present remain outside `HOT_FUNCTIONS` despite `PERFORMANCE.md` requiring production-authoritative renderer/projection registration.
- **Required resolution:** register the missing display/Metal production hot paths; do not treat #663 as an M001 blocker.

### F-005 — CI reproducibility honesty and supply-chain pins

- **Severity:** IMPORTANT
- **Status:** RESOLVED — merged #769
- **Owning Issue:** #755
- **PR:** #769
- **Evidence:** Foundation Quality does not silently skip required jobs, but green CI remains incomplete Pass 10 proof (DisplayLink bench env honesty, floating Actions tags vs SHA-pin policy, docs npm lockfile gaps).
- **Required resolution:** make CI claims honest and pin/lock supply-chain inputs per `GITHUB-WORKFLOW.md` policy.

### F-006 — TerminationFailed dead-end and PrimaryExitPending unbound retry

- **Severity:** BLOCKING
- **Status:** RESOLVED — merged #774
- **Owning Issue:** #756
- **PR:** #774
- **Paths:**
  - `crates/seyal-runtime/src/runtime.rs`
  - related exec/fault-injection and adversarial Runtime tests
- **Evidence:** after forced-reap deadline, lifecycle becomes `TerminationFailed` and remains a permanent silent registry sink; `PrimaryExitPending` retries lack an explicit convergence bound.
- **Required resolution:** restore recoverable or explicitly finalized termination after `TerminationFailed`; bound/converge `PrimaryExitPending`; add adversarial/fault-injection coverage. **Blocks Phase 2 validation.**

### F-007 — disconnect-during adversarial matrix gaps

- **Severity:** IMPORTANT
- **Status:** RESOLVED — merged #770
- **Owning Issue:** #757
- **PR:** #770
- **Evidence:** Pass 10 §6.8 requires disconnect-during evidence for input backpressure, outstanding resize, snapshot/display chunking, and Block finalization; dedicated cells are missing.
- **Required resolution:** add deterministic disconnect-during adversarial coverage for the four required matrix cells.

### F-008 — fuzz registry/campaign parity and Pass 7/9 surfaces

- **Severity:** IMPORTANT
- **Status:** RESOLVED — merged #772
- **Owning Issue:** #758
- **PR:** #772
- **Evidence:** `fuzz/targets.toml` covers Pass 2/5/8 only; Pass 7 protocol decoders unfuzzed; registry ≠ libFuzzer campaigns; §6.9 evidence risks INCONCLUSIVE without parity.
- **Required resolution:** restore registry/campaign parity and cover required Pass 7/9 fuzz surfaces for Pass 10 §6.9.

### F-009 — oversized Runtime/client modules and display publish bookkeeping

- **Severity:** IMPORTANT
- **Status:** RESOLVED — publish fix #771; module split → #765–#768
- **Owning Issue:** #759
- **PR:** #771
- **Evidence:** handwritten production modules exceed the AGENTS.md >1000-line justification threshold without a Pass 10 decomposition plan; display publish bookkeeping cohesion gaps reported with module-size findings.
- **Required resolution:** land display publish bookkeeping fix and record an authoritative module-cohesion disposition/deferral compatible with Pass 10 closure rules.

### F-010 — Rust↔Swift FFI ABI, borrow lifetime, and panic policy

- **Severity:** BLOCKING
- **Status:** RESOLVED — merged #762
- **Owning Issue:** #760
- **PR:** #762
- **Evidence:** escapable borrowed `NativePreparedFrame` pointers, incomplete ABI/lifetime enforcement, and panic-crossing-FFI policy gaps vs Pass 10 §5.1.
- **Required resolution:** enforce consume-before-next-poll / ABI / panic contracts with misuse coverage. **Blocks Phase 2 validation.**

## File-level review ledger

| Path | Subsystem | Class | Architecture | Correctness | Concurrency/resources | Security/privacy | Performance/hot path | Dead code/API/deps | Tests | Docs/diagram | Findings | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `docs/engineering/M001-PASS10-CODE-QUALITY-REVIEW.md` | Pass 10 authority | docs | checked | current-status wording stale | N/A | N/A | N/A | N/A | protocol exists | contradiction with current #5/#719/#727 state | F-001 / #748 → #751 | PASS |
| `docs/engineering/M001-PASS10-VALIDATION.md` | Pass 10 authority | docs | checked | current Ready-gate wording stale | N/A | N/A | N/A | N/A | validation protocol exists | contradiction with current #5/#719/#727 state | F-001 / #748 → #751 | PASS |
| SPEC / MILESTONE / distribution / scaffolding status docs | Pass authority | docs | checked | remaining Pass 9 / scaffolding drift | N/A | N/A | N/A | N/A | N/A | status truth vs #719 Done | F-003 / #753 → #763 | PASS |
| `scripts/check-hot-path.py` + display/Metal hot paths | Perf registry | build/prod | checked | registry incomplete for display/Metal | N/A | N/A | registry gap | N/A | check passes on registered set | PERFORMANCE.md coverage | F-004 / #754 → #761 | PASS |
| `.github/workflows/*` + docs npm pins | CI / supply chain | build | checked | honesty/repro gaps | N/A | pin policy | N/A | floating tags / lockfile | Foundation does not silent-skip | workflow honesty | F-005 / #755 → #769 | PASS |
| `crates/seyal-runtime/src/runtime.rs` (termination) | Runtime lifecycle | production | checked | TerminationFailed dead-end | BLOCKING resource sink | N/A | N/A | N/A | forced-reap fault coverage missing | AGENTS/SPEC termination | F-006 / #756 → #774 | PASS |
| disconnect-during adversarial matrix | Runtime / attachment | test | checked | §6.8 cells incomplete | disconnect races | N/A | N/A | N/A | matrix gaps | Pass 10 §6.8 | F-007 / #757 → #770 | PASS |
| `fuzz/targets.toml` + campaigns | Fuzz | test | checked | Pass 7/9 / parity gaps | N/A | trust-boundary fuzz missing | N/A | registry≠campaigns | §6.9 risk INCONCLUSIVE | Pass 10 §6.9 | F-008 / #758 → #772 | PASS |
| Runtime/client oversized modules + display publish | Runtime / client | production | checked | cohesion / bookkeeping | N/A | N/A | N/A | >1000-line modules | N/A | AGENTS.md size gate | F-009 / #759 → #771 | PASS |
| Rust↔Swift FFI / prepared-frame boundary | FFI | production | checked | ABI/borrow/panic gaps | borrow lifetime | UB risk | N/A | API misuse surface | misuse coverage incomplete | Pass 10 §5.1 | F-010 / #760 → #762 | PASS |

Issue-authority finding #752 has no production path row; disposition is tracked only in the resolution map below.

## Resolution map

| Finding | Severity | Owning Issue | PR | Re-review | Final disposition |
|---|---|---|---|---|---|
| F-001 stale Pass 9-blocked Pass 10 protocol wording | IMPORTANT | #748 | #751 | docs/authority spot-check on `e8431f0` | **RESOLVED** — merged; protocols no longer claim Pass 9 block |
| F-002 stale Pass 7/9/UI Issues | IMPORTANT | #752 | none (Issue-authority) | Issue/status truth | **RESOLVED** — closed #706/#725/#730/#654/#73; #663 later-milestone; PR #742 → #773 |
| F-003 remaining Pass 9 / scaffolding status drift | IMPORTANT | #753 | #763 | docs/authority spot-check | **RESOLVED** — merged |
| F-004 display/Metal hot-path registry | IMPORTANT | #754 | #761 | `scripts/check-hot-path.py` | **RESOLVED** — merged |
| F-005 CI reproducibility honesty / pins | IMPORTANT | #755 | #769 | workflow/docs honesty | **RESOLVED** — merged; floating Xcode image → post-M001 #764 |
| F-006 TerminationFailed / PrimaryExitPending | BLOCKING | #756 | #774 | concurrency/lifecycle tests | **RESOLVED** — merged; adversarial recovery/escalation/re-arm coverage landed |
| F-007 disconnect-during adversarial matrix | IMPORTANT | #757 | #770 | `pass10_disconnect_during` | **RESOLVED** — merged; non-vacuous finalization cell retained |
| F-008 fuzz registry/campaign parity | IMPORTANT | #758 | #772 | fuzz smoke + §6.9 honesty | **RESOLVED** — merged; CI smoke ≠ campaign PASS documented |
| F-009 modules / display publish bookkeeping | IMPORTANT | #759 | #771 | runtime publish + cohesion | **RESOLVED** — publish bookkeeping merged; oversized-module split → post-M001 #765–#768 |
| F-010 FFI ABI / borrow / panic policy | BLOCKING | #760 | #762 | FFI misuse + Swift ABI offsets | **RESOLVED** — merged |

Ledger kickoff PR: #749 (Refs #727).

## Phase 1 re-review notes (2026-09-05)

Re-reviewed the domains touched by merged finding PRs on exact head `e8431f01c797b57d7b6ee6a9be65706f77c7d789`:

- **Concurrency/lifecycle (F-006):** #774 restores recoverable `TerminationFailed` and bounds `PrimaryExitPending`; adversarial tests present.
- **FFI/unsafe (F-010):** #762 locks ABI, owned frame cells, panic=abort policy; live-handle misuse + Swift field-offset tests present.
- **Adversarial disconnect (F-007):** #770 matrix cells present including Block-finalization disconnect with unread-bytes proof.
- **Fuzz/§6.9 (F-008):** #772 registry/campaign parity + evidence-grade honesty; PR CI alone cannot score §6.9 PASS.
- **Docs/CI/hot-path/publish (F-001/F-003/F-004/F-005/F-009):** corresponding PRs merged; IMPORTANT module-size follow-ups explicitly parked post-M001 (#765–#768, #764).

No open `BLOCKING` Pass 10 finding Issues remain. Phase 2 final validation is authorized on the frozen head below.

## Frozen final M001 validation head

```text
e8431f01c797b57d7b6ee6a9be65706f77c7d789
```

Branch tip at freeze: `master` / `e8431f0` (“Close Pass 10 fuzz registry/campaign parity and Pass 7/9 surfaces (#772)”).

Any later production delta invalidates affected Phase 2 evidence and requires a new freeze + revalidation.

## Completion condition

Phase 1 of this ledger is complete: every `BLOCKING` finding is resolved and re-reviewed, every `IMPORTANT` finding is resolved or authoritatively assigned outside M001 (#764–#768), and the final M001 validation head is frozen above.

Phase 2 independent validation proceeds under `docs/engineering/M001-PASS10-VALIDATION.md` with evidence in `docs/engineering/M001-PASS10-EVIDENCE.md`.
