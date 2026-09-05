# M001 Pass 10 — Final Milestone Validation Protocol

**Status:** Normative final-validation authority (historical) — Phase 1 findings disposition complete; Phase 2 final validation **Done / satisfied** on production freeze `d845c6ddbe86…` (#776) with harness tip `e2b76024de2c…` (#778) and evidence tip `c1246d43869a…` (#779); owning Issues #727 and #5 closed  
**Owning validation Issue:** #727 — closed Done  
**Parent M001 Issue:** #5 — closed Done  
**Pass 9 prerequisite:** #719 — closed Done  
**Phase 1 review candidate:** `1005bc42397aac485b1aeff08cafd0f67790d969`  
**Historical refinement base:** `efa365d48565fb09452b683577700a8e5e267fcb`

## 1. Purpose

Pass 10 is the final independent M001 milestone-validation gate.

It does not define new terminal behavior and therefore does not require a new M001 behavioral specification merely to exist. Its authority is the accepted M001 milestone plus the accepted architecture, ADRs and specifications from Passes 1–9.

The required result is a criterion-level verdict on one frozen final production head:

```text
accepted M001 authority
+ exact-head conformance
+ exact-head failure/security evidence
+ controlled performance/resource evidence
+ clean-checkout production demo
= M001 PASS only when every mandatory criterion passes
```

Use `.agents/skills/milestone-validation/SKILL.md` as the primary workflow.

## 2. Entry gate

### 2.1 Phase 1 code/quality review — findings disposition complete

Pass 9 no longer blocks Pass 10 Phase 1. The following entry conditions were satisfied and Phase 1 review executed:

- Pass 9 production Issue #719 is closed Done;
- final Pass 9 independent review has no unresolved blocker;
- Pass 9 calibrated reconnect/cleanup/resource budgets are accepted and retained;
- current `master` contains the accepted Pass 1–9 production lineage;
- Phase 1 review candidate was frozen at `1005bc42397aac485b1aeff08cafd0f67790d969`;
- there is no unresolved architecture/specification question that blocks review;
- development readiness for #727 returned Ready and Phase 1 was transitioned to IN PROGRESS by user direction;
- finding Issues #748–#760 are closed; IMPORTANT follow-ups #764–#768 are parked post-M001.

Phase 1 follows `docs/engineering/M001-PASS10-CODE-QUALITY-REVIEW.md`. Findings disposition is complete; claiming full Phase 1 COMPLETE still requires file-level ledger completeness per that protocol.

### 2.2 Phase 2 final validation — historically satisfied / Done

Independent final milestone-validation execution was forbidden until all of these were true (protocol rules retained):

- Phase 1 review ledger covers every M001-significant file/module;
- no `BLOCKING` review finding remains;
- every `IMPORTANT` finding is resolved or authoritatively assigned outside M001 with the correct future owner;
- all review domains affected by accepted fixes have been rerun;
- a final exact M001 validation head is frozen after review-driven changes;
- development readiness for the Phase 2 validation execution of #727 returns Ready.

Those Phase 2 entry conditions were met. Final validation is **Done / satisfied** on the frozen production head with PASS evidence in `docs/engineering/M001-PASS10-EVIDENCE.md`; #727 and #5 are closed. Soft RSS gate remains **768 KiB**.

M002 implementation must not be started as a dependency bypass while M001 has a failed, missing or inconclusive mandatory Pass 10 criterion.

## 3. Validation is not a catch-all implementation pass

Pass 10 owns validation orchestration, evidence aggregation and final milestone acceptance reporting. It does not own terminal product behavior.

If validation discovers a production defect, architecture gap, missing behavior, unsafe lifecycle state or material regression:

1. mark the affected Pass 10 criterion `FAIL` or `INCONCLUSIVE`;
2. create/refine a separate owning Issue in the responsible module/pass/domain;
3. resolve it through the normal Ready → implementation → review flow;
4. freeze the resulting new exact head;
5. rerun every invalidated Pass 10 criterion and its affected aggregate regressions.

Do not hide a product fix inside a benchmark, test harness or evidence document. Do not pull M002 compatibility into M001 simply to make validation broader.

Validation-only tooling may be added or corrected when a required production-equivalent evidence seam is missing, but that tooling must itself be independently reviewable and must not change production authority or hot-path behavior.

## 4. Verdict model

Every mandatory milestone criterion gets one explicit status:

| Verdict | Meaning |
|---|---|
| `PASS` | direct reproducible evidence satisfies the criterion |
| `FAIL` | evidence contradicts the criterion |
| `INCONCLUSIVE` | required evidence is absent, stale, incomparable or insufficient |
| `PLATFORM_LIMITED` | a requested measurement hit a demonstrated host/platform ceiling |
| `N/A` | only when the accepted milestone itself makes the criterion non-applicable |

`PLATFORM_LIMITED` is evidence classification, not an automatic pass. It satisfies an aggregate milestone gate only where the governing authority explicitly permits that limitation and all required floors/minimum cases pass.

Every mandatory M001 criterion must resolve to `PASS` before M001 is complete.

## 5. Evidence freshness and provenance

The final validation record must identify:

- frozen production code SHA;
- evidence-record/documentation SHA when different;
- Mac model/chip and architecture;
- macOS version/build;
- Rust/toolchain version;
- build mode;
- terminal geometry/font/backing scale where relevant;
- workload;
- run/sample count;
- percentile method;
- exact commands;
- CI/workflow run IDs where applicable;
- **evidence class** for every performance/presentation/fuzz claim: `CI`, `controlled-host`, or `PLATFORM_LIMITED` (and never silently mix them).

### 5.1 CI vs controlled-host vs `PLATFORM_LIMITED`

Label every Pass 10 performance, presentation, fuzz-campaign and resource claim with exactly one provenance class:

| Class | Meaning | May satisfy Pass 10 presentation / absolute perf criteria? |
|---|---|---|
| `CI` | GitHub-hosted Foundation / path-filtered workflow output | Only for contracts those jobs actually own (build, check, test smoke, registry smoke, unsigned bench harness). **Not** headed presentation or controlled absolute budgets. |
| `controlled-host` | Named Apple Silicon (or other) host under documented conditions | Yes, when methodology matches the governing pass/spec |
| `PLATFORM_LIMITED` | Host/platform ceiling prevented the requested measurement | Only where governing authority explicitly permits that limitation |

Honesty rules:

- Foundation `native-macos-smoke` sets `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0`. That job’s `make bench` must be labelled `CI` and must **not** be cited as Pass 6 / Pass 10 headed presentation proof.
- Headed presentation-proxy evidence requires `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1` (or an equivalent headed session that produces presentation-proxy samples) on a controlled host and must be labelled `controlled-host`.
- Path-filtered Pass 5 libFuzzer (~30s) and Foundation fuzz-registry smoke are `CI`. They do not substitute for required continuous/campaign fuzz evidence under §6.9.
- Pass 9 five-cohort production budget artifacts are `controlled-host`. `make check` only self-tests the validator; that self-test is not production budget proof.
- `macos-latest` / floating Xcode image drift is known CI nondeterminism; do not treat Foundation Metal/terminfo smoke as bit-reproducible controlled-host evidence.

Historical pass evidence may be reused only when repository history proves the relevant production code/behavior is unchanged and the original result has sufficient provenance. Historical green CI, merged PR state, checked Issues and author assertions are not milestone proof by themselves.

A documentation-only evidence commit may follow a measured code head only when the production/tooling delta is proven zero and both SHAs are retained.

Cross-host, changed-OS or changed-toolchain measurements may be useful context but must not be described as controlled same-host regressions or improvements.

A production code change during Pass 10 invalidates the exact-head verdict and requires targeted plus aggregate revalidation on the new frozen head.


## 6. Criterion matrix

The final evidence record must expand the complete `MILESTONE-001.md` acceptance gates. At minimum, cover every domain below.

### 6.1 Architecture and ownership

Prove from the current production tree and behavior:

- one authoritative VT/`TerminalState` per `TerminalExecution`;
- one terminal endpoint/PTY per execution;
- Runtime owns execution registry/composition/lifecycle and attachment/controller authority;
- `BlockTimeline` remains Runtime/workspace metadata keyed by `ExecutionId`;
- Blocks own no PTY, child, VT, grid, copied transcript or renderer state;
- client display and Metal state are derived/disposable only;
- no GUI PTY replay or second terminal parser/grid;
- no temporary text renderer or parallel production terminal engine;
- no renderer/Block/agent/persistence/cloud/licensing/telemetry dependency in canonical PTY → VT progress;
- Seyal OSS has no dependency on commercial code.

### 6.2 VT, terminal state and terminfo

Run and reconcile:

- all retained M001 VT unit/property/byte fixtures;
- retained reference/conformance corpus with provenance;
- arbitrary PTY read/chunk-boundary parser equivalence;
- primary screen and scoped `CSI ?1049h/l` alternate-screen behavior;
- safe continuity across deferred/unsupported sequences;
- malformed/parser-fuzz invariant coverage;
- real production shell launch with explicit `TERM=seyal-m001`;
- bundled terminfo resolution without relying on inherited terminal metadata;
- capability-by-capability audit proving terminfo advertises no unsupported M001 behavior.

Do not promote a deferred sequence to supported merely to increase corpus breadth.

### 6.3 PTY, child and Runtime lifecycle

Use real production PTYs and children to validate:

- shell spawn/read/write;
- child exit versus signal versus PTY EOF/HUP;
- explicit terminate and deterministic reap;
- final terminal output before lifecycle finalization;
- endpoint-first resize: validate/prepare → PTY winsize → canonical state commit;
- repeated lifecycle cleanup and resource return;
- adversarial orthogonal states where PTY condition, child lifetime, controller state and GUI connection differ;
- no thread/process/render stack per execution by default.

### 6.4 Candidate-D attachment/projection/client

Validate the selected production path rather than legacy comparator machinery:

- compact versioned binary UDS framing;
- endpoint discovery/path ownership and same-user trust rules;
- Observer/Controller authorization and stale-identity rejection;
- bounded input/control/presentation queues;
- snapshot/delta atomic commit;
- generation-gap resync/current-state reconstruction;
- slow/dead client isolation from canonical PTY/VT progress;
- same-execution fanout correctness;
- final display before lifecycle finalization;
- malformed/truncated/oversized frames;
- unexpected/multiple/truncated ancillary descriptor handling;
- no transcript serialization or client-owned VT reconstruction.

Legacy shared-projection comparator/reference coverage is not production Candidate-D evidence.

### 6.5 Permanent Metal renderer and native interaction

Validate:

- deterministic renderer preparation and damage handling;
- permanent AppKit/Metal/CAMetalLayer production topology;
- coarse native/Rust boundary;
- hidden/occluded/detached surface resource release/reconstruction;
- preparation, submission and asynchronous GPU-completion failure state machines;
- bounded retry/exhaustion/recovery behavior;
- native keyboard → bounded client → Runtime-owned encoding/admission → PTY;
- Controller authority and visible bounded input failure behavior;
- correlated resize plus applied-generation projection fence;
- first-responder recovery;
- dead-key and real IME mark→commit/cancel behavior;
- IME candidate rectangle geometry;
- accessibility/VoiceOver smoke on the recreated permanent surface;
- no input, marked-text or terminal-secret disclosure in diagnostics/accessibility.

Native UI evidence must use the real macOS app/XCTest/XCUIAutomation where applicable. Browser automation is not a substitute.

### 6.6 Minimal Block metadata

Validate the M001 Block contract only:

- bounded Runtime/workspace-owned Block metadata keyed by `ExecutionId`;
- stable `BlockId` and primary logical-line anchor;
- anchor stability across scroll, resize, alternate screen, resync and detach/reconnect;
- monotonic `Current → Completed` lifecycle ordering;
- final display → Block Completed → Lifecycle Finalized ordering where negotiated;
- Block failure/quarantine cannot stall terminal progress;
- bounded BlockTimeline capacity and exact retirement;
- disposable client Block cache rebuilt from Runtime authority;
- no command scraping, copied transcript or M003 rich/composer semantics required for M001.

### 6.7 Pass 9 detach/reconnect/crash continuity

Re-run the final accepted Pass 9 contract on the final M001 head:

- normal app/window detach;
- abrupt GUI/socket death;
- same live Runtime and `ExecutionId` continuity;
- fresh `AttachmentId` and correct Controller recovery;
- detached terminal output;
- detached child exit and finalization;
- current Candidate-D state reconstruction without PTY replay;
- renderer/input/resize/IME/accessibility reconstruction;
- Pass 8 Block identity/anchor continuity;
- stale attachment/request identity rejection;
- endpoint discovery/startup races;
- repeated graceful/abrupt lifecycle resource cleanup against accepted Pass 9 budgets.

M001 does not claim Runtime-crash or reboot survival of a live PTY.

### 6.8 Failure and adversarial-state matrix

At minimum cover:

- malformed VT/protocol/projection/Block/reconnect inputs;
- PTY child exit, signal, EOF/HUP and winsize failure;
- slow, killed and disconnected clients;
- disconnect during input backpressure;
- disconnect during outstanding resize;
- disconnect during snapshot/display chunking;
- disconnect during Block/finalization work;
- endpoint bind/connect/stale-socket races;
- renderer preparation/submission/GPU-completion failures;
- persistent/N-times failures, not only one-shot failures;
- no hidden retry/timer/spin loop after persistent failure;
- exact logical cleanup and expected FD/thread/socket/attachment/controller/renderer/Block resource return.

Every level-triggered readiness handler must make progress, drain, disarm or throttle. Persistent no-progress wake loops are blocking.

### 6.9 Fuzz and retained regression corpus

Audit the registry against the final production surfaces, then run every applicable active target. Required coverage includes at least:

- VT byte parser;
- parser/state mutation;
- local binary protocol decode;
- Candidate-D display decode/client state;
- attachment/reconnect/resync state machine;
- Pass 7 protocol decoders (`TerminalKey`, resize, composer, history/timeline);
- BlockState decode;
- any additional final Pass 9 decoder/state-machine surface required by its accepted implementation contract (or an explicit `N/A` with architecture proof in `docs/engineering/M001-FUZZ-EVIDENCE.md`).

The registry being syntactically green is not enough. A required production surface without real fuzz coverage is `INCONCLUSIVE` and blocks milestone completion.

Foundation fuzz-registry smoke is continuous CI proof of registry/adapter integrity only. Path-filtered ~30s libFuzzer jobs are targeted PR evidence and must be labelled `CI`. They are not continuous campaign coverage and must not be cited alone as Pass 10 “fuzz clean.” See `fuzz/README.md` and `docs/engineering/GITHUB-WORKFLOW.md`.

**Evidence grade (mandatory):** PR CI campaigns at the `ci-smoke` floor (typically ≤30s) **cannot alone** score this criterion `PASS`. Milestone `PASS` requires `nightly-campaign` or `controlled-campaign` provenance on the exact validation head for every applicable active production target, as defined in `docs/engineering/M001-FUZZ-EVIDENCE.md`, or an explicit `N/A` with architecture proof for each remaining gap. Legacy Candidate-B shared-projection comparator rows are not production §6.9 coverage.

### 6.10 Security and privacy

Perform a fresh focused M001 threat review covering:

- PTY/process boundary;
- per-user Runtime endpoint location, ownership and permissions;
- same-user authentication and connection-bound attachment identity;
- Observer/Controller authorization;
- malformed/bounded protocol/projection inputs;
- unexpected ancillary descriptors;
- slow-client/resource abuse;
- reconnect/stale identity;
- renderer/native input/IME/accessibility privacy;
- Block metadata bounds/privacy;
- logs, benchmarks and diagnostics;
- no commercial/cloud/licensing requirement in terminal fundamentals.

When a security boundary changed after its historical review, rerun targeted security tests rather than accepting provenance only.

### 6.11 Performance, memory and resources

Performance validation must use accepted per-pass budgets, final accepted Pass 9 calibration-derived budgets and the existing M001 targets. Do not invent thresholds after observing Pass 10 results.

Publish/reconcile at least:

1. Runtime startup and 1/10/50/100 live-execution resource measurements where platform limits permit.
2. Candidate-D real PTY → VT → canonical damage → UDS → client-cache measurements.
3. Same-execution fanout 1/2/4/8/16.
4. Representative 80×24, 120×40, 200×60 and practical-maximum geometries where feasible.
5. Sparse, ordinary, token-streaming, sustained high-output, burst, scrolling, TUI-like partial/full redraw and alternate-screen workloads.
6. Output-to-client-state latency p50/p95/p99, source throughput and fanout throughput separately.
7. Runtime/client CPU and RSS, allocations/reallocations/bytes where instrumentable, copies/writes/syscalls, queue high-water, coalescing/resync and cleanup.
8. Renderer committed-state→prepare→command-commit→GPU-completion and headed presentation-proxy evidence.
9. Hidden/detached GPU resource use.
10. Pass 7 input/resize latency and resource evidence with comparable Pass 6 regression checks.
11. Pass 8 metadata latency/resource evidence and paired same-host regression attribution.
12. Pass 9 cleanup/reconnect/native-ready/repeated-cycle/RSS evidence against its frozen budgets.
13. Thread, FD and idle CPU behavior for hidden/detached states.
14. M001 architecture targets shown explicitly as `target vs measured` rather than assumed achieved.

Host PTY ceilings must be retained as `PLATFORM_LIMITED` evidence. Never silently lower a requested population or reinterpret a host allocator limit as a Seyal capacity limit.

Any historical >5% explanation / >10% blocking regression policy is used only where the governing accepted pass/spec makes it applicable and the measurement method is comparable.

### 6.12 Clean-checkout final demo

The final demo must use production topology from a clean checkout of the frozen head:

1. `make bootstrap`
2. `make build`
3. `make test`
4. `make check`
5. `make bench`
6. run all required production fuzz/security campaigns;
7. start the headless Runtime;
8. launch Seyal.app and attach/create a real `TerminalExecution`;
9. show the real shell prompt rendered through Metal;
10. prove the child receives and resolves bundled `seyal-m001` terminfo;
11. type commands through Runtime authority;
12. demonstrate supported ANSI color/cursor/erase behavior;
13. resize through the endpoint-first canonical transaction;
14. enter/leave scoped `?1049` alternate screen and recover primary state;
15. show real Runtime/workspace Block identity and logical anchor;
16. start long-running output, close GUI and prove execution continues;
17. reopen to the same execution and current state;
18. repeat with forced GUI termination;
19. prove input/resize/IME/accessibility again after reconnect;
20. explicitly terminate and prove reap/registry/resource cleanup;
21. present the criterion-level conformance/fuzz/failure/security/performance evidence.

Fake terminal cells, a direct renderer fixture, an in-process substitute Runtime or browser automation cannot replace the real end-to-end demo.

### 6.13 Non-goals and authority consistency

Audit the final tree and evidence to prove M001 has not silently absorbed:

- full VT/M002 compatibility;
- production scrollback/reflow/million-line history;
- tabs/splits/full workspace navigation UI;
- agents/orchestration;
- remote/cloud/mobile;
- Runtime-crash PTY keeper/reboot recovery;
- production history/layout persistence;
- M003 rich command Blocks/composer;
- public plugin/API/enterprise/commercial functionality.

Also audit stale Issue/spec/evidence status wording. Correct it only where current evidence supports the correction. A merged PR or checked checkbox is not proof by itself.

## 7. Performance measurement rules

Every controlled claim records exact environment and methodology before interpreting results.

Do not:

- increase timeouts to hide a product bottleneck without root-cause evidence;
- change workloads to create a favorable regression result;
- treat shared CI absolute latency/CPU/RSS as controlled product evidence;
- cite Foundation `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0` benches as headed presentation / Pass 6 presentation proof;
- describe GPU completion proxy as physical display scanout;
- combine source PTY throughput with N-viewer aggregate socket throughput;
- use a different host/OS/toolchain as a same-host A/B comparison;
- add production instrumentation that changes the terminal hot path merely to measure it.

If a required boundary cannot be measured without material perturbation, document the closest production-equivalent boundary and its limitation explicitly.

## 8. Final evidence artifact

Pass 10 execution should retain a final evidence document, expected as:

```text
docs/engineering/M001-PASS10-EVIDENCE.md
```

Do not pre-populate it with claimed results during refinement.

The final evidence artifact should contain:

- frozen code/evidence SHAs;
- criterion ledger with verdict + exact evidence pointer;
- commands and environment;
- conformance/fuzz/failure/security results;
- performance/resource tables;
- platform limitations;
- clean-demo evidence;
- non-goal audit;
- independent-review verdict;
- final M001 PASS/FAIL conclusion.

Raw benchmark/fuzz/native artifacts may remain CI/release artifacts where appropriate, with durable IDs/links referenced from the evidence record.

## 9. Documentation impact

Before M001 closure, re-assess:

- User Guide: only behavior genuinely supported/exposed by the final M001 slice;
- Developer Guide: Runtime/PTY/VT/projection/renderer/input/Block/reconnect architecture and clean validation workflow;
- engineering authority: exact final evidence, accepted budgets and status/provenance consistency;
- screenshots/video: optional final stable demo evidence, never a substitute for tests.

## 10. Completion rule

Pass 10 is complete only when every mandatory M001 acceptance criterion is `PASS` on the final accepted head, all required controlled evidence is retained, the clean production demo succeeds, security/performance/architecture independent review has no blocker, and no M001 non-goal or duplicate production authority has leaked into the final tree.

If even one mandatory criterion is `FAIL` or `INCONCLUSIVE`, M001 remains open.
