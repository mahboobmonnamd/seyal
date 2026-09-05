# Milestone 001 — Production Foundation Vertical Slice

**Status:** Passes 1–9 Done; Pass 10 remaining (owning Issue #727). Pass 10 Phase 1 milestone-closure code/quality review is in progress from frozen review candidate `1005bc42397aac485b1aeff08cafd0f67790d969`. Independent final validation is still gated on review completion and final-head freeze. M001 itself is not closed.

**Authority:** This document is subordinate to the accepted Seyal foundation architecture, its rationale, and accepted ADRs. It narrows that architecture into an implementable M001 slice; it does not reopen accepted foundation decisions.

## 1. Goal

M001 proves that Seyal's permanent production architecture works end-to-end on macOS without a temporary VT, temporary renderer, GUI-owned terminal state, or late persistence migration.

The required output path is:

```text
launch headless Seyal Runtime
→ create TerminalExecution
→ real PTY
→ real shell
→ Seyal-owned VT
→ authoritative TerminalState
→ damage
→ derived local display projection
→ native macOS Seyal.app
→ Metal
→ pixels
```

The required input path is:

```text
NSEvent
→ native input normalization
→ bounded one-way runtime input path
→ canonical terminal mode/key handling
→ PTY
→ shell/application
```

The required persistence proof is:

```text
launch Seyal.app
→ terminal execution exists

close or crash Seyal.app
→ Runtime + PTY + shell continue

reopen Seyal.app
→ attach to the same TerminalExecution
```

Runtime-crash PTY survival is not part of M001.

---

## 2. Exact architecture slice

### 2.1 Runtime ownership

One persistent per-user Seyal Runtime is implemented in M001.

The Runtime owns:

- runtime identity;
- `TerminalExecution` registry;
- attach/detach state;
- execution lifecycle;
- PTY transport;
- child process lifecycle;
- local child launch/capability environment above the PTY layer;
- one authoritative `TerminalState` per `TerminalExecution`;
- VT parser/modes;
- primary and minimal alternate screen state;
- generation-based damage;
- minimum logical line identity;
- minimal Block metadata in Runtime/workspace metadata;
- local display-projection producer.

The GUI does not own or mirror a VT or grid.

### 2.2 TerminalExecution ownership

Each `TerminalExecution` owns exactly:

```text
ExecutionId
TerminalEndpoint = one real PTY
child lifecycle
TerminalState = one authoritative Seyal VT/state
attachment/projection state
```

`BlockTimeline` is **not** owned by `TerminalExecution`. It is Runtime/workspace metadata in the `seyal-workspace` logical boundary, keyed by `ExecutionId` and logical history anchors.

A `TerminalExecution` may emit bounded asynchronous execution/history signals consumed by Block metadata, but PTY → VT → `TerminalState` → damage progress never waits for Block mutation or semantic processing.

No Block, window, pane, renderer, or agent owns terminal infrastructure.

### 2.3 Native macOS host

M001 Seyal.app contains only what is necessary for the vertical slice:

- application lifecycle;
- one window;
- one terminal surface;
- keyboard input;
- focus;
- resize;
- IME/accessibility architecture seam;
- one minimal Block presentation;
- Metal renderer.

Final product chrome, workspace navigation, tabs, panes, inspectors, animations, attention UI, and rich Block UX are not M001 requirements.

### 2.4 Local attachment/projection

M001 uses the mechanism defined by `docs/architecture/ADR-001-LOCAL-DISPLAY-PROJECTION.md`:

```text
compact binary Unix-domain control/input
+
Runtime-written versioned shared-memory display projection
+
one-way generation wake/signal
```

The projection is renderer-facing, rebuildable, bounded, and read-only from the client's perspective.

No synchronous GUI↔Runtime acknowledgement is part of terminal progress.

---

## 3. In scope

M001 implements only:

- one per-user headless Runtime process;
- runtime identity and startup/attach discovery;
- one or more `TerminalExecution` objects sufficient to exercise 1/10/50/100 idle-execution measurements;
- one real macOS PTY per execution;
- one real shell child per execution;
- explicit Runtime-owned local terminal capability environment;
- bundled `seyal-m001` terminfo describing only the M001-tested capability subset;
- Seyal-owned incremental VT parser/state machine;
- explicit M001 VT subset in section 7;
- primary screen;
- minimal alternate-screen enter/leave path;
- resize path;
- stable logical line identity minimum;
- generation-based damage;
- local projection mechanism;
- first permanent Metal renderer path;
- native keyboard input path;
- minimum Block identity/state in Runtime/workspace metadata;
- GUI detach/reconnect;
- GUI crash survival;
- clean explicit execution termination;
- test/fuzz/benchmark/measurement harnesses;
- CI quality gates.

---

## 4. Explicit non-goals

M001 does **not** implement:

```text
full VT
production scrollback/reflow
million-line history
history compression/paging
multiple panes
tabs
multiple workspaces
agent orchestration
smart routing
agent subscriptions
cloud
mobile
remote internet attach
public embedding API
Teams
Enterprise administration
RBAC
SSO
billing
plugins
production Lua runtime
final configuration system
rich artifacts
final futuristic UI
runtime-crash PTY survival
reboot recovery
production history persistence
production layout persistence
stable public TERM/terminfo identity
SSH/remote terminfo installation or fallback
```

These are intentionally deferred, not forgotten.

---

## 5. Authoritative ownership matrix for M001

| Resource | Authoritative owner | M001 rule |
|---|---|---|
| Runtime identity | Seyal Runtime | one persistent per-user Runtime |
| `TerminalExecution` registry | Seyal Runtime | GUI references IDs only |
| PTY | `TerminalExecution` | exactly one per execution |
| child process | `TerminalExecution` | survives GUI detach/crash |
| local `TERM`/terminfo capability advertisement | Runtime/product composition | PTY layer remains policy-neutral; advertise only implemented/tested capabilities |
| VT parser/modes | `TerminalState` in Runtime | exactly one authority |
| primary/alternate screen | `TerminalState` in Runtime | GUI projection only |
| logical line identity | `TerminalState`/history seam | stable, minimal |
| damage generation | Runtime `TerminalState` | monotonic generation |
| Block timeline/metadata | `seyal-workspace` / Runtime workspace metadata keyed by `ExecutionId` | no PTY/grid ownership; async observation only |
| local projection writer | Runtime | single writer |
| local projection reader | Seyal.app | derived/read-only |
| shaping/glyph cache | renderer | client-side/native render domain |
| Metal surface | Seyal.app | visible surface only |
| input normalization | AppKit host | terminal mode encoding remains Runtime-authoritative |

---

## 6. Hot paths

### 6.1 Output

```text
PTY read
→ bounded Runtime I/O task/shard
→ Seyal VT parse/state mutation
→ damage generation
→ write derived projection generation
→ publish generation
→ one-way client wake
→ read visible projection
→ shaping/font fallback
→ glyph lookup/cache
→ draw preparation
→ Metal command encoding
→ present
```

Forbidden on this path:

- JSON;
- transcript serialization;
- synchronous persistence;
- synchronous Block semantics;
- synchronous agents;
- Lua;
- licensing/cloud/telemetry;
- per-cell Swift/Rust callbacks;
- renderer acknowledgement required for VT progress.

### 6.2 Input

```text
NSEvent
→ AppKit normalization
→ coarse native/Rust boundary
→ compact binary input/control message
→ Runtime
→ canonical terminal mode/key encoding
→ PTY write
```

The client must not guess terminal-mode-sensitive encoding from stale mirrored VT state.

### 6.3 Resize

Resize follows the accepted Foundation §5.4 transaction exactly:

```text
window geometry
→ rows/cols proposal
→ Runtime validates controller authority + geometry
→ Runtime prepares all locally rejectable/infallible resize inputs
→ apply fallible PTY winsize
→ commit canonical TerminalState resize/reflow
→ damage generation
→ new projection
```

The Runtime must not publish/reflow canonical geometry that the endpoint rejected. If a future canonical resize can fail after endpoint commit, the implementation must add explicit prepare/commit/rollback semantics rather than permit PTY/`TerminalState` divergence.

M001 has one controller, but the authority seam must not require redesign for future observers/controllers.

### 6.4 Local terminal capability environment (`TERM` / terminfo)

`TerminalEndpoint`/PTY creation remains environment-policy neutral. It does not invent `TERM`.

The Runtime/product composition that launches the local M001 shell owns terminal capability advertisement. It must not depend on an arbitrary parent process already having a correct `TERM`, because the headless Runtime/Seyal.app may be launched from Finder/launchd or another non-terminal environment.

M001 also must not advertise `xterm-256color` merely for compatibility: that terminfo entry describes a substantially broader behavior set than Seyal intentionally claims in M001.

For the M001 local vertical slice:

```text
bundled project-owned terminfo source
→ compile/package with Seyal development/runtime artifact
→ Runtime resolves the bundled terminfo database
→ spawned local shell receives:
     TERM=seyal-m001
     TERMINFO/TERMINFO_DIRS as required for that packaged database
```

The `seyal-m001` entry is deliberately milestone-scoped and advertises only capabilities that are implemented and covered by the M001 conformance/fixture contract. Adding a capability to that entry requires the corresponding terminal behavior to be implemented and tested first.

`seyal-m001` is **not** the promised long-term public terminal identity. A stable public identity (expected to be project-owned, potentially `xterm-seyal` for real-world ecosystem compatibility) is chosen only after Seyal has enough xterm-compatible behavior to justify that advertisement. SSH/remote propagation, automatic terminfo installation, and fallback to a widely installed entry are later compatibility work and are not pulled into M001.

---

## 7. M001 VT feature matrix

Rules:

1. Every `SUPPORTED M001` behavior requires tests first.
2. Unsupported sequences must be ignored/rejected according to a documented parser policy without being silently treated as correctly implemented semantics.
3. Parser framing must be permanent and extensible; deferred features add handlers/state, not a replacement parser.
4. Unknown/private sequences must never corrupt parser state or panic.
5. The bundled `seyal-m001` terminfo must not advertise a capability outside the supported/tested M001 contract.

### 7.1 Supported M001

| Area | Sequence/behavior | M001 classification |
|---|---|---|
| Text | printable UTF-8 scalar input | SUPPORTED M001 |
| Controls | CR, LF, BS, HT/TAB | SUPPORTED M001 |
| Cursor | CUU/CUD/CUF/CUB | SUPPORTED M001 |
| Cursor | CUP/HVP absolute positioning | SUPPORTED M001 |
| Cursor | CHA/VPA where needed by fixtures/shell output | SUPPORTED M001 |
| Erase | ED basic modes 0/1/2 | SUPPORTED M001 |
| Erase | EL modes 0/1/2 | SUPPORTED M001 |
| SGR | reset, bold/intensity seam, underline seam, inverse seam | SUPPORTED M001 |
| Color | default + ANSI 16 colors | SUPPORTED M001 |
| Color | 256-color indexed representation | SUPPORTED M001 |
| Color | truecolor RGB representation | SUPPORTED M001 |
| Cursor | DECTCEM cursor visibility | SUPPORTED M001 |
| Cursor | save/restore cursor required by shell fixtures | SUPPORTED M001 |
| Screen | primary screen | SUPPORTED M001 |
| Screen | minimal `CSI ?1049h/l` alternate-screen subset defined by SPEC-001; no broader xterm DECSC/DECRC save-slot compatibility claim | SUPPORTED M001 |
| Resize | rows/cols mutation + cursor/grid invariants | SUPPORTED M001 |
| Terminal modes | canonical mode storage needed by implemented input/screen behavior | SUPPORTED M001 |
| Parser | split/incremental UTF-8 and escape sequence delivery across arbitrary PTY read boundaries | SUPPORTED M001 |

### 7.2 Tested but deferred

These require parser tests proving safe recognition/state continuity, but not full semantics in M001:

| Area | Behavior | M001 classification |
|---|---|---|
| SGR | faint, blink, conceal, strike, advanced underline styles | TESTED BUT DEFERRED |
| Character editing | ICH/DCH/ECH | TESTED BUT DEFERRED |
| Line editing | IL/DL | TESTED BUT DEFERRED |
| Scrolling | scroll regions, SU/SD | TESTED BUT DEFERRED |
| Modes | origin mode, insert mode, application cursor/keypad expansion | TESTED BUT DEFERRED |
| Tabs | custom tab-stop set/clear | TESTED BUT DEFERRED |
| OSC | title/CWD/hyperlinks | TESTED BUT DEFERRED |
| Charset | legacy DEC charset switching | TESTED BUT DEFERRED |

For this category, tests verify that encountering the sequence does not corrupt subsequent supported parsing and that deferral is observable in diagnostics/fixtures where practical.

### 7.3 Unsupported / deferred

| Area | Behavior | M001 classification |
|---|---|---|
| Mouse | mouse reporting modes/protocols | UNSUPPORTED / DEFERRED |
| Images | sixel, Kitty graphics, iTerm image protocol | UNSUPPORTED / DEFERRED |
| OSC | clipboard OSC 52 | UNSUPPORTED / DEFERRED |
| Device queries | broad DA/DSR/DECRQSS support | UNSUPPORTED / DEFERRED |
| Reflow | production logical reflow | UNSUPPORTED / DEFERRED |
| Unicode | full grapheme-cluster/emoji-width correctness | UNSUPPORTED / DEFERRED beyond minimum rendering seam |
| Protected fields | DEC selective erase/protected areas | UNSUPPORTED / DEFERRED |
| Printing | printer/media-copy sequences | UNSUPPORTED / DEFERRED |
| Rare/private VT | unscoped vendor/private extensions | UNSUPPORTED / DEFERRED |

### 7.4 M001 VT acceptance workloads

The M001 subset must support a normal interactive shell prompt and simple commands such as:

```text
printf / echo output
pwd
ls with basic ANSI color
clear
multiline output
backspace/editing handled by shell line discipline/readline path
resize while shell is active
one explicit alternate-screen fixture/application that uses only the supported subset
```

The end-to-end shell fixture must prove that Runtime launch does not rely on inherited terminal metadata: the child sees `TERM=seyal-m001`, the bundled terminfo entry resolves successfully, and its advertised capabilities remain within the tested M001 matrix.

M001 does not claim Vim/Neovim/tmux/htop/Claude Code correctness. Those are M002 workloads.

---

## 8. Minimum Block model

M001 proves Blocks are architectural from day one without putting them in the terminal hot path.

Minimum model:

```text
BlockId
ExecutionId
kind
start: LogicalLineAnchor
state: Current | Completed
```

Optional M001 metadata may include timestamps only if it does not expand scope.

Rules:

- `BlockTimeline` and Block metadata are owned by `seyal-workspace` / Runtime workspace metadata, keyed by `ExecutionId`;
- `TerminalExecution` does not own Block semantic state;
- one Block never owns PTY, VT, grid, child process, renderer, or output copy;
- Block mutation may observe/coarsely bracket execution activity asynchronously;
- PTY → VT → `TerminalState` → damage progress never waits for Block mutation;
- shell integration and command intelligence are not required;
- absence of shell integration must still yield a correct raw terminal;
- renderer may visually expose one Block identity/shell region, but no rich card system is required.

---

## 9. History and line identity

M001 establishes stable logical identity, not production scrollback.

Minimum types/concepts:

```text
LineId       = monotonic/stable identity for a logical terminal line
LineRevision = optional generation/revision when content mutates
LogicalLineAnchor = LineId + anchor semantics sufficient for Block start
```

Requirements:

- Block anchors never use viewport row numbers as durable identity;
- resize may change physical row presentation without changing the identity contract;
- current screen/history storage may remain deliberately small/bounded in M001;
- APIs must allow later chunked scrollback/reflow without changing `BlockId`/`ExecutionId` ownership semantics.

Not required in M001:

- million-line scrollback;
- disk history;
- compression;
- paging;
- production reflow algorithm;
- history search.

---

## 10. Detach, reconnect, crash, and termination semantics

### 10.1 Normal app close

```text
close Seyal.app window / terminate GUI
→ detach client
→ do not terminate TerminalExecution
→ Runtime remains alive
→ PTY + child remain alive
```

### 10.2 GUI crash

Unexpected app death must have the same execution-lifetime result as detach:

```text
GUI disappears
→ Runtime detects connection loss
→ projection/client resources are reclaimed
→ TerminalExecution continues
```

### 10.3 Reconnect

```text
launch Seyal.app
→ discover/connect to existing per-user Runtime
→ enumerate/resolve target ExecutionId
→ attach
→ receive current full visible projection snapshot + generation
→ render
→ resume damage generations
```

Reconnect must not replay PTY bytes through a second parser in the GUI.

### 10.4 Explicit termination

An explicit terminate-execution action tells the Runtime to terminate the child/PTY according to clean lifecycle semantics and remove the execution after exit/reap.

Closing the view is not equivalent to terminate.

### 10.5 Runtime exit

For M001, stopping/killing the Runtime may terminate its PTYs/children. M001 does not claim live survival across Runtime failure.

---

## 11. First permanent renderer pipeline

Required pipeline:

```text
derived display projection
→ damage
→ visible cells/runs
→ shaping/font fallback
→ glyph cache/atlas
→ instance/draw preparation
→ Metal
→ present
```

### Rust responsibilities

Rust owns:

- renderer-facing projection schema/types;
- damage normalization/coalescing;
- visible cell/run extraction;
- style/color IDs;
- draw-oriented batching structures where they avoid native per-cell crossings;
- renderer deterministic test inputs;
- protocol/projection validation.

### Native macOS responsibilities

Swift/AppKit plus the smallest justified native Metal bridge owns:

- window/view lifecycle;
- display scale and surface size;
- font discovery/platform fallback hooks;
- glyph rasterization integration where platform APIs are required;
- Metal device/queue/pipeline/texture/buffer lifecycle;
- glyph atlas texture upload;
- command encoding;
- frame scheduling/presentation;
- native input, IME, accessibility seam.

### Boundary rule

Cross Rust/native boundaries with coarse arrays/runs/batches, never per cell.

M001 may keep shaping deliberately narrow, but the first renderer must use the permanent shaping/cache/Metal architecture rather than a temporary text renderer.

---

## 12. Engineering and test gates

M001 starts test-first. Production implementation is not accepted when tests are added only after the visible terminal works.

### 12.1 Rust unit tests

Required for:

- parser state transitions;
- UTF-8 chunk boundaries;
- supported controls/CSI/SGR;
- cursor/grid invariants;
- primary/alternate transitions;
- resize invariants;
- damage generation/coalescing;
- `LineId` stability;
- Block ownership/state;
- projection generation publication/validation.

### 12.2 Byte-level VT fixtures

Fixtures contain exact input bytes and expected canonical terminal state/damage.

Include:

- sequences split at every meaningful byte boundary;
- multiple sequences in one PTY read;
- malformed/truncated escape sequences;
- UTF-8 split across reads;
- unsupported/deferred sequences followed by supported text;
- alternate-screen enter/leave fixture;
- resize fixture.

### 12.3 Reference/conformance corpus

For every behavior classified `SUPPORTED M001`, maintain a retained reference/conformance corpus derived from authoritative terminal specifications or independently established terminal-behavior fixtures where practical.

Requirements:

- fixture source/provenance is recorded;
- supported behavior is checked against expected reference semantics where practical;
- regression fixtures are retained in the repository;
- disagreements between implementation and reference behavior are resolved explicitly rather than silently normalizing the implementation;
- deferred behavior remains deferred and is not pulled into M001 merely to increase conformance breadth;
- the bundled `seyal-m001` terminfo capability set is checked against the supported matrix so capability advertisement cannot outrun implementation.

### 12.4 Property tests

At minimum verify:

- arbitrary parser chunking produces the same result as contiguous input for supported sequences;
- cursor never leaves valid grid invariants after supported operations;
- invalid input never panics;
- damage generations are monotonic;
- projection reader never accepts an incomplete generation.

### 12.5 Fuzzing

Maintain fuzz targets for:

- VT byte parser;
- parser/state mutation boundary;
- local binary protocol decode;
- shared projection header/record validation;
- reconnect/resync state machine.

Primary gate: no crash, panic, memory unsafety, runaway allocation, or invariant violation for retained corpus/regression cases.

### 12.6 PTY integration tests

Required:

- spawn real shell through real PTY;
- read shell output through Seyal VT;
- write input through Runtime path;
- child exit/reap;
- winsize update;
- detach while child runs;
- reconnect to same `ExecutionId`;
- kill GUI test while child runs;
- explicit execution termination;
- Runtime-spawned local shell receives the explicit bundled M001 terminfo environment rather than relying on inherited `TERM`.

### 12.7 Renderer deterministic tests

Use deterministic projection fixtures to verify:

- damage → draw batch conversion;
- cursor visibility/position;
- style/color mapping;
- resize invalidation;
- glyph-cache identity policy;
- no draw work for unchanged/hidden surfaces.

Pixel/golden tests may be used for stable synthetic glyph/geometry cases where platform variation is controlled.

### 12.8 Local Runtime attachment security

Before Pass 5 is accepted, complete a focused threat/security review of the Unix-domain control and display-model transport path (ADR-001 Candidate D; the shared-memory projection alternative considered here was not selected for production — see ADR-001 §13.3 note below) covering at minimum:

- Unix-domain socket location, ownership and permissions;
- same-user client authentication/authorization;
- Runtime discovery without trusting attacker-controlled filesystem paths;
- controller versus observer authority;
- malformed or oversized control/protocol messages, including truncated ancillary data (`MSG_CTRUNC`);
- bounds/version/generation validation before reads;
- client crash and Runtime crash cleanup behavior;
- prevention of client mutation of canonical terminal state;
- bounded attachment/projection allocation against local denial-of-service;
- no terminal hot-path cloud/licensing/telemetry dependency.

This does not require enterprise RBAC, SSO, remote internet attach, or cloud security in M001.

### 12.9 Static/dynamic quality gates

At minimum:

- `cargo fmt --check`;
- Rust lint/static analysis with warnings treated deliberately;
- Swift formatting/lint direction appropriate to repository tooling;
- sanitizer runs where technically suitable for native/Rust integration tests;
- memory/error diagnostics for projection mapping/unmapping and app crash cases;
- deterministic dependency lockfiles;
- reproducible-build direction documented in CI/tooling;
- CI runs tests, fuzz smoke/regression corpus, and benchmark smoke checks where stable.

---

## 13. Benchmark and measurement plan

M001 records baselines; it must not label unmeasured aspirations as achieved performance.

### 13.1 Required measurements

Record environment metadata with every result: Mac model/chip, macOS version, build mode, commit SHA, terminal dimensions, font/scale, shell, workload, run count, and percentile method.

Measure:

- Runtime process startup;
- Seyal.app startup to attach/first present;
- keypress → Runtime/PTY write;
- PTY read → committed `TerminalState` generation;
- `TerminalState`/damage → published projection;
- published projection → presented frame;
- end-to-end PTY read → present;
- sustained output throughput;
- idle CPU;
- active CPU;
- RSS Runtime only;
- RSS app + 1 execution;
- RSS with 10 idle executions;
- RSS with 50 idle executions;
- RSS with 100 idle executions;
- thread count for Runtime/app at 1/10/50/100 executions;
- hidden/detached GPU resource usage;
- reconnect/full-snapshot latency and bytes;
- projection allocations/copies where instrumentable.

### 13.2 Existing architecture targets remain targets

Preserve, but do not claim as achieved:

- hidden/detached idle execution target `<= 256 KiB` Seyal-owned hot resident memory before scrollback payload;
- 100 hidden idle executions target `<= 25 MiB` Seyal-owned execution overhead;
- thread count should not scale linearly with execution count;
- hidden/detached executions should have no dedicated full GPU render surface.

M001 must publish measured results next to those targets.

### 13.3 Projection decision benchmark

Pass 5 compared:

```text
A. compact binary Unix-domain snapshot/delta transport
B. selected hybrid Unix-domain control + shared-memory projection
```

Use identical workloads. Record latency, CPU, RSS, copies/bytes, allocations, reconnect cost, and slow/dead client behavior.

**Resolved**: ADR-001 selected option A (Candidate D — compact binary UDS snapshot/delta transport) as the production architecture; option B remains in the tree only as isolated, non-default comparator/reference evidence. See ADR-001's "Measured evidence" section for the production-path benchmark results. No benchmark outcome moved authoritative VT into the GUI.

---

## 14. Dependency-ordered implementation passes

No next pass starts until the current pass is working, tested, demonstrable, and benchmarked where relevant.

**Frontier (2026-09-04):** Passes 1–9 are Done on `master`. Pass 10 (#727) is the remaining M001 gate. Required-exit lists below remain the historical pass contracts; they are not open implementation checklists for already-completed passes.

### Pass 1 — Repository/build/test foundation — Done

Implement only build/test skeletons and module boundaries justified by accepted ownership.

Required exits:

- Rust workspace/native app build from clean checkout;
- test commands documented;
- CI runs formatting/lints/unit tests;
- benchmark harness can record environment metadata;
- fuzz targets compile/run smoke corpus;
- no production terminal code beyond scaffolding.

### Pass 2 — Seyal VT parser/state minimum — Done

TDD the `SUPPORTED M001` parser/state behaviors without PTY dependency.

Required exits:

- byte fixtures pass;
- retained reference/conformance corpus exists for the `SUPPORTED M001` subset where authoritative/reference behavior is practical to encode;
- reference fixture provenance is recorded;
- supported behavior is checked against reference expectations and disagreements are resolved explicitly;
- property tests pass;
- parser fuzz target runs;
- primary/minimal alternate screen and damage generations are testable headlessly;
- unsupported/deferred behavior matrix is reflected by tests/diagnostics;
- conformance work does not pull deferred/full VT behavior into M001;
- no alternate VT dependency.

### Pass 3 — PTY + TerminalExecution — Done

Add real macOS PTY endpoint and child lifecycle around the same terminal engine.

Required exits:

- real shell starts;
- output is parsed by Seyal VT only;
- input reaches PTY;
- winsize changes work;
- exit/reap works;
- integration tests pass;
- PTY owner is `TerminalExecution`;
- PTY layer remains free of product `TERM`/terminfo policy.

### Pass 4 — Headless Runtime — Done

Move/host execution under the persistent per-user Runtime from the first runnable app architecture.

Required exits:

- Runtime launches independently of GUI;
- stable runtime identity;
- execution registry supports create/list/attach/detach/terminate minimum;
- one execution continues with no GUI attached;
- Runtime has bounded concurrency architecture, not thread/daemon per execution;
- Runtime owns local child capability-environment selection above the PTY layer;
- M001 local shell launch uses bundled `seyal-m001` terminfo rather than inherited/false standard terminal capability claims.

### Pass 5 — Local attachment/projection — Done

Implement ADR-001 and run the transport benchmark comparator.

Required exits:

- versioned local attach/control path;
- shared projection has single Runtime writer;
- generation publication/resync tests pass;
- killed/stalled app cannot backpressure PTY→VT;
- first attach/reconnect full snapshot works;
- socket-only vs hybrid measurements recorded;
- focused local attachment threat/security review is complete;
- local socket ownership/permissions and same-user authorization are tested;
- Runtime discovery does not trust attacker-controlled filesystem paths;
- malformed/oversized protocol messages are rejected safely;
- shared-memory mapping validates version, bounds and committed generation before consumption;
- shared-memory ownership, permission, lifetime and cleanup behavior is tested;
- stale/reused projection identifiers cannot grant unintended access;
- stalled/crashed clients cannot mutate canonical terminal state or exhaust unbounded Runtime resources;
- controller/observer authority is explicit even though M001 exercises one controller;
- selected mechanism remains justified or ADR amended with evidence.

### Pass 6 — Metal renderer — Done

Render deterministic projection fixtures and then live Runtime projection through the permanent Metal path.

Required exits:

- Metal is the first production terminal renderer;
- damage drives incremental draw preparation;
- shaping/font fallback/glyph-cache seams exist in permanent locations;
- no NSTextView/SwiftUI terminal renderer;
- no per-cell language crossing;
- hidden surface releases/drops dedicated GPU resources.

### Pass 7 — Native input + resize — Done

Connect AppKit input and resize to Runtime authority.

Required exits:

- key event → Runtime → PTY path works;
- mode-sensitive encoding owned by Runtime;
- resize follows validate/prepare → PTY winsize → canonical `TerminalState` commit → damage/projection;
- focus/IME/accessibility seams are present without replacing Metal surface;
- latency instrumentation is active.

### Pass 8 — Minimal Block + logical anchor — Done

Add minimum Block metadata over the existing execution/history seam.

Required exits:

- real `BlockId` references real `ExecutionId`;
- Block timeline authority is `seyal-workspace` / Runtime workspace metadata, not `TerminalExecution`;
- start anchor uses logical line identity, not viewport row;
- current/completed state is demonstrable;
- no second PTY/grid/output transcript;
- PTY→VT→damage path has no synchronous Block dependency.

### Pass 9 — Detach/reconnect + GUI crash survival — Done

Prove persistent Runtime ownership through real lifecycle behavior.

Required exits:

- close app, shell continues;
- reopen app, same `ExecutionId` attaches;
- kill/crash app process, shell continues;
- reconnect gets current snapshot without reparsing terminal history in GUI;
- projection resources from dead client are reclaimed;
- explicit terminate remains distinct from close/detach.

### Pass 10 — Conformance/performance/failure validation — In progress

Run the complete M001 suite and publish baselines. Owning Issue #727. Phase 1 code/quality review is authorized from frozen candidate `1005bc42397aac485b1aeff08cafd0f67790d969`; Phase 2 final validation remains gated on review completion and final-head freeze.

Required exits:

- all VT/PTY/projection/renderer/detach tests pass;
- retained VT reference/conformance regression corpus is clean;
- bundled `seyal-m001` terminfo resolves and advertises no unsupported capability;
- fuzz regression corpus clean;
- required measurements captured for 1/10/50/100 execution cases;
- failure tests include GUI death, malformed projection/protocol input, PTY child exit, reconnect/resync;
- focused local Runtime threat review and security tests pass;
- no M001 non-goal has leaked into required implementation;
- final demo procedure passes from clean build.

---

## 15. M001 acceptance gates

M001 passes only when every item below is demonstrated, not merely represented by interfaces.

These checkboxes remain the **Pass 10 final milestone-validation ledger**. Passes 1–9 production evidence is necessary but not sufficient; Pass 10 (#727) must still mark each criterion `PASS` on the final frozen M001 head. Do not check them from historical CI, merged PR state, or Issue assertions alone.

### Architecture

- [ ] exactly one authoritative VT/state per `TerminalExecution`;
- [ ] PTY owner is unambiguously `TerminalExecution` in Runtime;
- [ ] Block timeline authority is `seyal-workspace` / Runtime workspace metadata keyed by `ExecutionId`;
- [ ] `TerminalExecution` does not own Block semantic state;
- [ ] Block observation/mutation cannot block PTY → VT → damage progress;
- [ ] Runtime exists and is headless from M001;
- [ ] GUI contains no VT mirror;
- [ ] local projection mechanism matches ADR-001 or has been amended by measured evidence;
- [ ] no synchronous IPC request/response ping-pong in terminal hot paths;
- [ ] no display JSON/transcript serialization;
- [ ] no daemon/thread/render stack per execution by default;
- [ ] stable logical line identity exists independently of viewport rows;
- [ ] terminal capability advertisement is Runtime/product-owned above the PTY layer.

### Correctness

- [ ] every `SUPPORTED M001` VT behavior has tests written before/with implementation;
- [ ] retained reference/conformance fixtures cover the claimed M001 VT subset where practical;
- [ ] reference fixture provenance is recorded;
- [ ] no deferred behavior was promoted merely to satisfy corpus breadth;
- [ ] unsupported/deferred sequences are not silently represented as correct;
- [ ] real shell works through a real PTY;
- [ ] Runtime-spawned local shell uses resolvable bundled `seyal-m001` terminfo and does not rely on inherited `TERM`;
- [ ] bundled terminfo advertises no behavior outside the tested M001 matrix;
- [ ] resize follows the canonical endpoint-first transaction and PTY winsize is updated;
- [ ] minimal `?1049` alternate-screen subset works for the scoped fixture without claiming broader xterm save-slot compatibility;
- [ ] malformed/parser-fuzz inputs do not crash or violate invariants.

### Rendering/input

- [ ] Metal is the first production renderer;
- [ ] damage controls incremental presentation;
- [ ] native/Rust crossing is batched/coarse;
- [ ] keyboard input reaches PTY through Runtime authority;
- [ ] hidden/detached execution holds no dedicated full render surface.

### Persistence/lifecycle

- [ ] normal GUI close detaches without killing execution;
- [ ] GUI crash leaves Runtime/PTY/child alive;
- [ ] reopen attaches to the same `ExecutionId`;
- [ ] explicit terminate is separate from detach;
- [ ] no claim is made for Runtime-crash live-PTY survival.

### Security

- [ ] local socket ownership/permissions and same-user client authorization are tested;
- [ ] Runtime discovery is protected from attacker-controlled paths;
- [ ] protocol length/bounds/version validation is tested;
- [ ] shared-memory permissions/lifetime/generation validation is tested;
- [ ] malformed or hostile local-client input cannot corrupt Runtime terminal authority;
- [ ] attachment/projection resource usage is bounded against local denial-of-service;
- [ ] controller/observer authority is explicit;
- [ ] focused M001 local Runtime threat review is recorded.

### Engineering/performance

- [ ] CI gates formatting/lints/tests;
- [ ] byte fixtures/property tests/fuzz targets exist;
- [ ] PTY integration tests exist;
- [ ] renderer deterministic tests exist;
- [ ] detach/reconnect/crash tests exist;
- [ ] benchmark harness records required environment metadata;
- [ ] all required startup/latency/CPU/RSS/thread/GPU measurements are recorded;
- [ ] existing architecture targets are reported as target vs measured, not assumed achieved.

---

## 16. Demo procedure

The final M001 demo must be reproducible from a clean checkout/build.

1. Start the headless Seyal Runtime.
2. Launch Seyal.app and attach/create one `TerminalExecution`.
3. Show the real shell prompt rendered through Metal.
4. Verify the spawned local shell receives `TERM=seyal-m001` and resolves the bundled terminfo entry without relying on inherited terminal metadata.
5. Type commands and show input reaching the shell through Runtime.
6. Produce ANSI color and cursor/erase behavior from the supported matrix.
7. Resize the window and demonstrate validate/prepare → PTY winsize → canonical `TerminalState` commit behavior.
8. Run the scoped `?1049` alternate-screen fixture/application; leave alternate screen and recover primary screen state.
9. Show the real Block identity and logical start anchor from Runtime/workspace metadata without another PTY/grid.
10. Start a long-running shell command or counter.
11. Close Seyal.app; prove the Runtime/execution remains alive.
12. Reopen Seyal.app; attach to the same `ExecutionId`; show current state from projection snapshot/resync.
13. Repeat using forced GUI termination/crash; verify the execution still lives.
14. Explicitly terminate the execution and verify child reap/registry cleanup.
15. Run/present VT unit/reference/property tests, terminfo capability validation, PTY integration tests, projection tests, renderer tests, fuzz smoke/regression corpus, local attachment security tests, and CI status.
16. Present benchmark results for startup, latency stages, throughput, CPU, RSS, threads, hidden GPU resources, and the projection comparator.

---

## 17. Historical readiness check before implementation (superseded)

> **Superseded as a current implementation gate.** Architecture readiness for starting M001 passes was satisfied; Passes 1–9 are Done. The remaining open work is Pass 10 review/validation (#727), not re-opening Pass 1–9 implementation.

```text
[x] exactly one authoritative VT
[x] PTY owner is unambiguous
[x] BlockTimeline owner is Runtime/workspace metadata, not TerminalExecution
[x] Runtime exists from M001
[x] GUI close does not own shell lifetime
[x] Block does not own terminal infrastructure
[x] Metal is first production renderer
[x] no temporary VT
[x] no synchronous IPC ping-pong
[x] no display JSON
[x] local projection mechanism is decided
[x] VT M001 subset is explicit
[x] resize transaction is aligned with Foundation §5.4
[x] local TERM/terminfo ownership is explicit and capability-honest
[x] tests precede supported behavior
[x] retained reference/conformance corpus is required
[x] local attachment security gate is required
[x] benchmark harness exists in plan
[x] M001 is small enough to finish
```

# M001 READY FOR IMPLEMENTATION