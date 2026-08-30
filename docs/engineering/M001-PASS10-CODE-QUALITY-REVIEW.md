# M001 Pass 10 — Milestone Closure Code and Quality Review

**Status:** Refinement authority; execution blocked until Pass 9 is complete  
**Owning Issue:** #727  
**Parent M001 Issue:** #5  
**Companion protocol:** `docs/engineering/M001-PASS10-VALIDATION.md`

## 1. Purpose

M001 is not complete merely because Passes 1–9 behave correctly and the Pass 10 validation suite is green.

Before final milestone validation, Pass 10 requires an exhaustive milestone-closure review of the complete M001 production slice. The review exists to prove that the implementation is maintainable, secure, architecture-correct, resource-disciplined, testable, documented and suitable as the foundation for later milestones.

The required closure sequence is:

```text
Pass 1–9 complete
→ freeze review candidate
→ exhaustive module/file/code review
→ route every blocking finding to an owning Issue/PR
→ merge accepted fixes/cleanup
→ repeat affected reviews until clean
→ freeze final M001 head
→ run independent Pass 10 validation
→ M001 PASS only if review and validation both pass
```

This is deliberately stricter than a conventional PR review. Green CI, historical approvals, issue checkboxes, previous review comments and PR descriptions are inputs, not proof.

## 2. Scope rule: review everything; fix through traceable ownership

Pass 10 must inspect the entire code, build, test, benchmark, documentation and architecture surface that participates in or governs M001.

Pass 10 is still not a single catch-all implementation PR. When review identifies a production defect, security weakness, architecture violation, dead code, dependency problem, documentation contradiction or material maintainability issue:

1. record the finding with file/module and evidence;
2. classify severity and affected M001 criterion;
3. create/refine a separate owning Issue where a change is required;
4. implement and review the correction in a focused PR;
5. update architecture/docs in the same owning change when that is the correct authority;
6. freeze the resulting new candidate head;
7. rerun the affected code-review domains and all invalidated Pass 10 validation evidence.

Small documentation-only corrections may be grouped where they have one clear ownership purpose. Do not mix unrelated production cleanup into one giant milestone PR.

No finding may be waived solely because changing it is inconvenient late in the milestone.

## 3. Review granularity and evidence ledger

Review must be **module by module, file by file and line by line for production-significant code**.

Maintain a review ledger that records, at minimum:

- repository path;
- module/subsystem;
- production/test/build/docs classification;
- reviewer/pass identity;
- architecture authority checked;
- correctness result;
- concurrency/resource result;
- security/privacy result;
- performance/hot-path result;
- dead-code/API/dependency result;
- test adequacy result;
- documentation/diagram impact;
- findings and owning Issue/PR;
- final status: `PASS`, `BLOCKED`, `N/A`.

A directory-level statement such as “runtime reviewed” is insufficient when production files inside it have not been individually covered.

Generated files and third-party vendored material may be classified separately, but the generation process, pinned source/version, license and integration boundary still require review.

## 4. Repository and dependency inventory

Before code review, establish the exact final repository surface:

- Rust workspace crates and features;
- Swift/AppKit/Metal sources and targets;
- FFI/native boundary files;
- build scripts and generated artifacts;
- test/fixture/helper crates and native test targets;
- benchmark harnesses;
- fuzz targets and corpora;
- shell scripts and Make targets;
- terminfo sources/install path;
- CI/workflow files;
- documentation, ADRs, specifications and architecture diagrams;
- all direct and transitive dependencies relevant to M001.

For every dependency, verify that it is necessary, appropriately scoped, licensed, maintained enough for the risk it carries, and does not violate the OSS/commercial boundary.

Remove unused dependencies through focused cleanup changes. Challenge dependencies that duplicate functionality already owned by Seyal or introduce avoidable hot-path, security or supply-chain cost.

## 5. Rust production-code review

Review every M001 Rust production module, including the final locations of PTY/child lifecycle, Runtime/execution authority, VT parser/state/grid, terminal state, Candidate-D protocol/projection/attachment, Block metadata, renderer preparation/damage data, reconnect state and native-facing interfaces.

For each file and function, inspect:

- ownership and lifetime correctness;
- single-authority invariants;
- state-transition validity;
- boundary validation;
- integer/size conversions and overflow behavior;
- error propagation and cleanup;
- allocation/copy/clone behavior;
- data-structure complexity and capacity bounds;
- lock/atomic/channel usage;
- blocking operations and syscall placement;
- repeated work in hot paths;
- hidden synchronization or thread hops;
- avoidable serialization/deserialization;
- duplicate state or cached authority;
- API visibility and encapsulation;
- naming and semantic clarity;
- comments for non-obvious invariants rather than commentary that repeats code.

### 5.1 `unsafe` and FFI

Inventory every `unsafe` block/function/impl and every FFI boundary. For each one require:

- why safe Rust cannot express the operation reasonably;
- explicit safety invariants;
- ownership/lifetime assumptions;
- null/alignment/length requirements;
- thread-affinity requirements;
- panic/unwind behavior across the boundary;
- caller/callee responsibility for allocation and release;
- tests or assertions covering misuse/failure where feasible.

Unjustified `unsafe` is blocking.

### 5.2 Panic and placeholder audit

Review production use of:

- `unwrap` / `expect`;
- `panic!`;
- `assert!` used for external/runtime conditions;
- `todo!` / `unimplemented!` / `unreachable!`;
- ignored `Result` / error values;
- silent fallback paths.

Each occurrence must either be proven impossible by a local invariant or converted into bounded, observable error handling. A comment alone is not proof.

## 6. Swift/AppKit/Metal production-code review

Review every native M001 production file line by line, including application lifecycle, Runtime attachment, FFI, CAMetalLayer/Metal renderer integration, keyboard/input, resize, IME, accessibility and reconnect reconstruction.

Inspect:

- AppKit main-thread requirements;
- ownership/retain-cycle behavior;
- Swift/Rust memory ownership across FFI;
- buffer length and lifetime correctness;
- `Unsafe*` pointer usage;
- asynchronous callback lifetime and cancellation;
- Metal resource lifetime and command-buffer completion handling;
- hidden/occluded/detached resource release;
- window/view/layer reconstruction after reconnect;
- input authority and backpressure behavior;
- resize correlation/generation fencing;
- IME mark/commit/cancel state;
- accessibility privacy and stale-state behavior;
- error surfaces rather than silent fallback;
- absence of temporary text-rendering or duplicate terminal authority.

Any native code that can synchronously stall canonical terminal progress must be justified or removed from that dependency path.

## 7. Concurrency, scheduling and resource review

Perform a cross-module concurrency review rather than reviewing locks only in isolation.

Prove:

- lock ordering cannot deadlock;
- terminal hot progress does not depend on UI/renderer/agent/persistence/cloud/licensing work;
- channels/queues are bounded where untrusted or bursty producers exist;
- backpressure cannot wedge PTY/VT progress;
- level-triggered readiness always drains, makes progress, disarms or throttles;
- retry/timer paths have bounded behavior and cannot spin indefinitely;
- cancellation/disconnect races do not leak authority/resources;
- atomics have documented ordering where non-trivial;
- shared state has one owner and derived caches are disposable;
- thread count does not scale accidentally per cell/block/view or other inappropriate unit;
- FD/socket/process/Metal/resource ownership has deterministic release paths.

Run race-sensitive/adversarial tests appropriate to the platform and architecture. Where a Rust concurrency model is intentionally lock-free or uses unsafe synchronization, demand stronger invariant documentation and targeted tests.

## 8. Security and privacy code review

In addition to the Pass 10 threat validation, perform source review for:

- UDS path construction, ownership and permissions;
- same-user authentication assumptions;
- Observer/Controller authorization enforcement;
- attachment/request identity binding and stale identity rejection;
- binary frame lengths, counts and integer arithmetic;
- malformed/truncated/oversized message handling;
- ancillary descriptor validation;
- PTY command/environment handling;
- path traversal or unsafe filesystem operations;
- shell/script injection opportunities in build/test/runtime helpers;
- secret/terminal/input/IME leakage through logs, diagnostics, assertions or accessibility;
- denial-of-service through allocations, queue growth, resync loops or fanout;
- unsafe deserialization/state reconstruction;
- accidental network/cloud dependency in local terminal fundamentals;
- dependency/supply-chain exposure;
- OSS depending on or importing commercial-only code.

Security findings are blocking until resolved or explicitly proven outside the M001 attack surface by current architecture authority.

## 9. Performance and hot-path source review

Do not rely only on benchmarks. Review source for architectural performance hazards.

For PTY → bytes → VT → canonical state → damage → projection → renderer preparation, identify and challenge:

- avoidable allocations/reallocations;
- avoidable clones/copies;
- whole-grid or whole-buffer work where damage/local work is sufficient;
- JSON/text serialization in production hot paths;
- excessive syscalls;
- synchronous IPC round trips;
- lock contention and coarse critical sections;
- O(n²) or unbounded work;
- per-cell/per-glyph language-boundary callbacks;
- unnecessary process/thread hops;
- unnecessary timers/polling;
- repeated Unicode/width/layout calculations that can safely be retained or batched;
- memory retained by hidden/detached clients;
- unbounded scroll/projection/Block/cache structures inside the M001 scope.

Any optimization must preserve correctness and single-authority architecture. Do not weaken correctness to win a benchmark.

## 10. Dead code, duplication and API-surface cleanup

Pass 10 must actively look for and remove obsolete milestone scaffolding where removal is safe and evidenced.

Audit:

- unused functions/types/modules;
- obsolete feature flags and `cfg` branches;
- deprecated adapters/prototypes/comparators no longer needed by retained tests or architecture evidence;
- duplicate implementations of the same state/protocol logic;
- unused public exports;
- test-only helpers accidentally compiled into production;
- stale benchmark paths;
- unused dependencies;
- obsolete scripts/Make targets;
- TODO/FIXME/HACK markers that conceal required M001 work;
- commented-out code and abandoned compatibility paths.

Do **not** delete retained conformance fixtures, security regression tests, benchmark baselines or historical architecture evidence merely because production no longer calls them. Classify evidence separately from dead production code.

Public visibility should be the minimum necessary. A symbol being used by tests alone is not sufficient reason for a broad production API when a narrower test seam is possible.

## 11. Code structure and enterprise maintainability

Review whether the final code is understandable and safely changeable by engineers who did not author it.

Check:

- modules have one coherent responsibility;
- files/functions are not excessively large or mixing unrelated concerns;
- abstractions remove real duplication/complexity rather than adding speculative layers;
- important invariants are encoded in types/state machines where practical;
- names match canonical Seyal concepts;
- errors carry actionable context without secrets;
- platform-specific behavior is isolated at the appropriate boundary;
- portable Rust logic is not unnecessarily implemented in Swift;
- macOS code does not introduce a premature cross-platform GUI abstraction;
- no speculative architecture exists for distant milestones;
- testability is designed into boundaries without creating alternate production engines.

Large-file size is a review trigger, not an arbitrary numeric rejection rule. Split when cohesion, reviewability, testing or ownership improves; do not fragment code merely to meet a line count.

## 12. Tests, fuzzing and benchmark-code quality

Review the test code itself for false confidence.

Check:

- tests assert meaningful behavior rather than implementation trivia;
- fixtures cannot leak child processes, PTYs, sockets, threads or temp files;
- timeouts are not masking deadlocks or performance regressions;
- failure tests prove the intended failure boundary;
- tests use real PTYs/production topology when the contract requires it;
- mocks/fakes cannot accidentally satisfy production acceptance criteria;
- fuzz targets reach current production decoders/state machines;
- retained corpora remain relevant;
- benchmark setup does not measure setup noise as product cost or hide product work outside the measured interval;
- benchmark failure/host limits are classified correctly;
- flaky tests are fixed at root cause rather than retried into green.

Coverage percentage alone is not an acceptance metric. Risk and state-transition coverage matter more.

## 13. Build, CI, reproducibility and supply-chain review

Audit:

- `Cargo.lock` and toolchain policy;
- Swift/Xcode project determinism where applicable;
- build scripts and code generation;
- warnings/lints and denied classes;
- feature combinations that can compile unsupported authority paths;
- deterministic/reproducible build expectations;
- CI parity with documented local commands;
- security-sensitive dependency updates and provenance;
- dependency licenses and OSS compatibility;
- workflow permissions and untrusted-input handling;
- artifacts/logs for accidental secret or terminal-content retention;
- quality gates that can be bypassed or silently skipped.

A green workflow with a skipped mandatory gate is not a Pass 10 success.

## 14. Architecture reconciliation and diagrams

Pass 10 must reconcile the implementation against the canonical architecture documentation.

At minimum verify diagrams/documentation for:

```text
Runtime
→ TerminalExecution
→ PTY/child
→ byte stream
→ authoritative VT/TerminalState/grid
→ damage/projection
→ Candidate-D client state
→ Metal renderer
```

and the orthogonal relationships for:

- Workspace/BlockTimeline metadata;
- controller/observer attachments;
- detach/reconnect lifecycle;
- native input and resize authority;
- Swift/Rust/Metal ownership boundary;
- OSS/commercial boundary where architecture documentation describes it.

If diagrams are absent, materially stale, contradict code, show duplicate authority, retain replaced architecture or imply unsupported milestone behavior, they must be corrected before M001 closure.

Architecture diagrams are descriptive evidence, not authority over accepted ADRs/specifications. If code and accepted authority disagree, do not simply redraw the diagram to match the code—resolve the architecture conflict first.

## 15. Documentation cleanup

Audit all M001-relevant documentation for:

- stale pass/frontier/status wording;
- obsolete paths/names from earlier architecture iterations;
- duplicate/conflicting ADR/spec statements;
- superseded prototypes presented as production;
- outdated commands;
- incorrect diagrams;
- dead links;
- unsupported claims of performance, compatibility, persistence or enterprise behavior;
- docs that accidentally imply commercial dependencies in OSS;
- temporary refinement notes that no longer provide audit value.

Remove unnecessary documentation only when it has no retained architectural, decision, conformance, security, benchmark or audit value. Historical ADRs/evidence should normally be retained and clearly superseded rather than erased.

User and developer documentation must describe only behavior actually available in the accepted final M001 slice.

## 16. OSS/commercial boundary audit

Prove from manifests, imports, build scripts, CI and docs that:

- `seyal` can build/test/run independently of commercial code;
- OSS terminal fundamentals do not depend on licensing, cloud, collaboration or enterprise services;
- no commercial source is copied/imported into OSS through generated code or build-time paths;
- commercial integration consumes OSS in the permitted direction only.

Any reverse dependency is a blocking architecture violation.

## 17. Review passes and independence

Require at least two distinct closure perspectives before final milestone acceptance:

1. **Implementation-quality review:** exhaustive file/module/code inspection and cleanup ledger.
2. **Independent milestone validation:** execute `M001-PASS10-VALIDATION.md` against the resulting frozen head without inheriting the implementation review's conclusions.

For high-risk areas—PTY/child lifecycle, unsafe/FFI, protocol/authz, concurrency, Metal resource lifetime and reconnect—perform a second focused review after fixes even if the first reviewer authored none of the changes.

The final reviewer must establish evidence from the current tree rather than copy prior verdicts.

## 18. Severity and closure rules

Classify findings at least as:

- **BLOCKING:** correctness, architecture, security, data/resource leak, material performance regression, missing mandatory tests/evidence, unsafe undefined behavior risk, OSS/commercial violation, or documentation/diagram contradiction capable of misleading implementation authority.
- **IMPORTANT:** maintainability, dead code/dependency, test weakness, documentation drift or design debt that should be resolved before M001 closure unless explicitly demonstrated to belong to a later milestone.
- **NON-BLOCKING:** small clarity/style improvements with no meaningful milestone risk.

Pass 10 cannot be declared complete while any BLOCKING finding remains open.

IMPORTANT findings require resolution before closure unless #727 records concrete authority proving why the work is outside M001 and assigns it to the correct later milestone. “We can clean it later” is not sufficient.

## 19. Required final artifacts

Before the final Pass 10 validation verdict, retain:

- complete review ledger;
- finding → Issue/PR resolution map;
- unsafe/FFI inventory;
- dependency and public-API cleanup result;
- dead-code/duplicate-path cleanup result;
- security source-review result;
- concurrency/resource review result;
- hot-path source-review result;
- test/fuzz/benchmark quality review result;
- architecture/diagram reconciliation result;
- documentation cleanup result;
- OSS/commercial boundary result;
- final frozen production SHA.

These may be sections of the final Pass 10 evidence document or separately retained evidence files with durable links.

## 20. Definition of complete

The M001 code-quality closure review passes only when:

- every production-significant file/module has an explicit review-ledger result;
- all BLOCKING findings are resolved and re-reviewed;
- all IMPORTANT findings are resolved or correctly assigned outside M001 with evidence;
- no unexplained dead production path, dependency, public API, unsafe block or duplicate authority remains;
- architecture diagrams and documentation match accepted authority and final implementation;
- security, concurrency, resource and hot-path source reviews have no unresolved blocker;
- OSS remains independent of commercial code;
- the candidate head is frozen after the last accepted cleanup/fix;
- independent final validation is then run against that exact head.

Only the combination of **clean milestone-closure code review + successful independent final validation** permits M001 to close.