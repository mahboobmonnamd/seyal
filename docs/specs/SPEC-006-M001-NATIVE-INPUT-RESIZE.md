# SPEC-006 — M001 native input, focus/IME seam and authoritative resize

- **Status:** Proposed for M001 Pass 7
- **Date:** 2026-08-27
- **Issue:** #702
- **Architecture authority:** Foundation Architecture + ADR-001 + ADR-004 + ADR-005 + ADR-006
- **Depends on:** SPEC-001, SPEC-002, SPEC-003, SPEC-004, SPEC-005

## 1. Purpose

This specification defines the observable M001 Pass 7 contract that makes the permanent macOS Metal terminal surface interactive without moving terminal authority into the GUI.

The required input path is:

```text
NSEvent / AppKit text-input callbacks
→ native normalization
→ bounded typed client queue
→ compact local protocol
→ Runtime authorization
→ Runtime-owned terminal-key encoding
→ bounded Runtime input admission
→ PTY
→ shell/application
```

The required resize path is:

```text
usable native terminal viewport
→ rows/columns proposal
→ bounded client control queue
→ Runtime authorization + validation/prepare
→ fallible PTY winsize
→ canonical TerminalState resize commit
→ damage/projection
→ permanent Metal renderer
```

Pass 7 does not create a GUI VT parser, mirrored mode state, client-owned grid, second PTY, temporary text renderer or synchronous GUI acknowledgement dependency.

## 2. Non-negotiable invariants

1. `TerminalExecution` remains sole owner of the PTY, child lifecycle and canonical `TerminalState`.
2. Runtime remains the authority for attachment role, terminal-mode-sensitive key encoding, input admission, PTY writes and canonical resize commit.
3. AppKit owns only native event normalization plus ephemeral focus/IME composition state.
4. The client never consults disposable display state to decide terminal escape sequences.
5. Existing `Input` bytes represent already-committed literal input bytes. They are not a license for Swift to synthesize mode-sensitive terminal key sequences.
6. Semantic terminal keys cross the client/Runtime boundary as typed logical keys, not pre-encoded escape bytes.
7. Accepted input preserves FIFO ordering. New input is explicitly rejected on bounded backpressure rather than silently truncated, reordered or accepted into unbounded memory.
8. Resize never publishes canonical geometry before the PTY accepts the winsize transaction.
9. No input/resize path waits synchronously for rendering, display projection, Block semantics, persistence, agents, cloud, telemetry or licensing.
10. Input, marked text and terminal contents are secret-bearing data and are never emitted by latency instrumentation or normal diagnostic logs.

## 3. Scope

Pass 7 includes:

- first-responder/focus behavior for the permanent `MetalSurfaceView` terminal surface;
- AppKit native event normalization;
- committed UTF-8 text input;
- the exact M001 semantic terminal-key subset in section 6;
- Runtime-owned key encoding seam;
- Controller attachment for an interactive client;
- bounded, nonblocking client input/control queuing;
- native viewport → rows/columns calculation and authoritative resize submission;
- resize coalescing with ordering barriers;
- permanent AppKit IME/text-input-client seam;
- minimum accessibility/focus seam on the Metal surface;
- input/resize latency instrumentation and benchmarks;
- deterministic, integration, failure and fuzz coverage required by section 16.

Pass 7 does not claim full terminal application compatibility. The canonical M001 workload remains a normal interactive shell plus the accepted minimal alternate-screen fixture.

## 4. Native input classification

Every native key/text event is classified into exactly one of these categories before terminal submission:

```text
ApplicationCommand
CommittedText
SemanticTerminalKey
CompositionState
Unsupported
```

### 4.1 ApplicationCommand

An application/menu shortcut is handled by the native application layer and is never simultaneously submitted to the PTY.

M001 reserves the Command modifier for application/menu handling. `Command`-modified events must not become `Input` or `TerminalKey` frames merely because AppKit also exposes characters for the event.

Pass 7 does not define a configurable keybinding system.

### 4.2 CommittedText

Text committed by the AppKit text-input system is encoded as UTF-8 and submitted using the existing SPEC-004 `Input` message.

Committed text may originate from:

- ordinary printable keyboard input;
- keyboard-layout transformations;
- dead-key composition;
- an input method after composition commits.

The client does not reinterpret committed Unicode text into terminal key sequences.

### 4.3 SemanticTerminalKey

Terminal-control keys that are not ordinary committed text use the additive `TerminalKey` protocol extension in section 7. Runtime maps the logical key to bytes using canonical terminal semantics.

### 4.4 CompositionState

Marked/preedit text is client-local ephemeral state. It is not an `Input` message and never reaches Runtime/PTY until AppKit commits it through `insertText` or its equivalent.

### 4.5 Unsupported

An unsupported native event is not guessed into terminal bytes. It remains unhandled or follows ordinary native application behavior. Debug diagnostics may record only the event category/key identifier, never text content.

## 5. Event-routing order

The terminal surface must avoid duplicate delivery through `keyDown`, menu key equivalents and AppKit text interpretation.

The behavioral order is:

1. allow recognized application/menu commands to resolve as native application commands;
2. recognize the supported non-text terminal keys and supported Control-key combinations from the native event;
3. route remaining text-producing input through AppKit's text-input/IME machinery;
4. submit only committed text callbacks as `Input`;
5. preserve marked/preedit callbacks locally;
6. never submit one physical/native event by more than one route.

A production implementation may use `NSTextInputContext`, `interpretKeyEvents` or equivalent AppKit mechanisms, but it must satisfy this observable classification/order. In particular, it must not unconditionally send navigation/function keys through text interpretation if that causes those keys to leak as text/control characters.

## 6. Exact M001 keyboard matrix

Pass 7 intentionally implements a small permanent semantic-key layer that can be extended in M002 without changing AppKit ownership.

### 6.1 Supported M001

| Native intent | Wire representation | Runtime M001 result |
|---|---|---|
| committed printable/Unicode text | `Input` UTF-8 bytes | bytes unchanged |
| Return/Enter | `TerminalKey::Enter` | `CR` (`0x0D`) |
| Tab | `TerminalKey::Tab` | `HT` (`0x09`) |
| Backspace | `TerminalKey::Backspace` | `DEL` (`0x7F`) |
| Escape | `TerminalKey::Escape` | `ESC` (`0x1B`) |
| Up | `TerminalKey::ArrowUp` | normal-mode ANSI `ESC [ A` |
| Down | `TerminalKey::ArrowDown` | normal-mode ANSI `ESC [ B` |
| Right | `TerminalKey::ArrowRight` | normal-mode ANSI `ESC [ C` |
| Left | `TerminalKey::ArrowLeft` | normal-mode ANSI `ESC [ D` |
| Control + supported ASCII base | `TerminalKey::ControlAscii` + scalar | section 6.2 mapping |
| native key repeat for any supported semantic key | repeated semantic-key submissions | same ordering/encoding per occurrence |

The arrow-key encoding lives in Runtime even though M001 does not yet implement/advertise application-cursor mode. M002 may add DECCKM/application-keypad semantics to the Runtime encoder without changing the native event boundary. Pass 7 must not invent fake canonical mode state merely to demonstrate a mode toggle.

### 6.2 Control ASCII mapping

`ControlAscii` accepts only ASCII base scalars for which the conventional terminal control mapping is defined and tested:

```text
@ / Space → NUL 0x00
A..Z      → 0x01..0x1A
[          → ESC 0x1B
\          → FS  0x1C
]          → GS  0x1D
^          → RS  0x1E
_          → US  0x1F
?          → DEL 0x7F
```

ASCII letters are normalized case-insensitively for this mapping. No locale-dependent Unicode case conversion participates.

### 6.3 Explicitly deferred keyboard behavior

M001 does not claim:

- Option/Alt-as-Meta terminal policy;
- configurable Option-as-Alt behavior;
- application-cursor or application-keypad mode switching;
- Home/End/Insert/DeleteForward/PageUp/PageDown terminal sequences;
- Shift-Tab/backtab terminal sequence;
- function keys F1+;
- modified cursor/function-key xterm protocols;
- kitty keyboard protocol;
- arbitrary user keybindings/macros.

Unsupported combinations must not be silently rewritten into a different supported key.

## 7. SPEC-004 additive semantic-key protocol extension

Pass 7 adds a capability-gated message to the existing SPEC-004 protocol without changing framing version `1.0`.

### 7.1 Capability

Server capability bit 2 is:

```text
CAP_SEMANTIC_TERMINAL_KEY = 1 << 2
```

A Pass 7 interactive client requires this capability before sending `TerminalKey`. An older Runtime that does not advertise it remains valid for Pass 5/6 display but is not Pass 7 interactive-capable.

The client must not probe support by sending an unknown message first.

### 7.2 Message type

Message type 17 is:

```text
17  C→R  TerminalKey
```

It is legal only in `Attached` state for the attachment's current `Controller`. Observer use returns `PermissionDenied`; stale/foreign attachment IDs fail under existing SPEC-004 rules.

### 7.3 Payload

`TerminalKey` is exactly 24 bytes:

```text
u128 AttachmentId
u16  key_kind
u16  modifiers
u32  scalar
```

M001 `key_kind` values:

```text
1 Enter
2 Tab
3 Backspace
4 Escape
5 ArrowUp
6 ArrowDown
7 ArrowRight
8 ArrowLeft
9 ControlAscii
```

M001 `modifiers` is zero for kinds 1–8. `ControlAscii` requires exactly bit 0 (`CONTROL`) and no other bits. All unknown modifier bits are malformed. `scalar` must be zero for kinds 1–8 and must be one supported ASCII base scalar from section 6.2 for `ControlAscii`.

Malformed `TerminalKey` frames return `MalformedPayload` without terminal mutation. Unsupported but well-formed key kinds are not invented; unknown kind values are malformed in protocol 1.0.

### 7.4 Why `Input` remains

`Input` remains the efficient bulk path for already-committed bytes such as UTF-8 committed text. `TerminalKey` exists only where the client must preserve semantic intent so Runtime can own terminal encoding.

Pass 7 does not wrap every printable character into one message per scalar.

## 8. Runtime key encoding authority

Runtime decodes `TerminalKey` into a typed internal key intent and admits the resulting bytes through the same bounded input accounting used by SPEC-003.

The encoder consumes canonical terminal mode state when a supported key's encoding depends on a mode. M001 currently has no accepted application-cursor/keypad input mode, so arrows use the normal-mode sequences in section 6; future supported modes extend this encoder rather than adding GUI state.

The encode/admit operation must be all-or-nothing for one logical key: capacity is checked for the complete encoded byte sequence before any byte from that key is accepted. Backpressure never leaves a partial escape sequence accepted.

Only the Runtime reactor owner writes resulting bytes to the PTY, preserving SPEC-003 ordering and reservation accounting.

## 9. Interactive attachment and authority

The production Pass 7 terminal surface requests `Controller`, not `Observer`, for the execution it intends to control.

Rules:

- no client silently preempts an existing controller;
- `ControllerBusy` is an explicit noninteractive state;
- the Pass 7 surface must not appear to accept terminal typing while only Observer authority exists;
- no silent fallback may accept native input locally and discard it;
- reconnect obtains a new attachment/controller lease under normal SPEC-004 semantics; old `AttachmentId` values are stale;
- losing the connection or controller authority cancels marked composition locally and stops accepting terminal mutation until authority is re-established.

A future product may offer an intentional read-only observer UI; that is not the success path claimed by Pass 7.

## 10. Bounded client outbound queue

The display client evolves into a local terminal client with one ordered outbound control/input queue.

M001 client-side accepted-but-not-fully-written wire bytes are bounded to **262,144 bytes per connection**, including frame headers. This is separate from Runtime's SPEC-003 accepted-but-unwritten PTY budgets.

Requirements:

- FIFO for `Input`, `TerminalKey`, non-coalesced `Resize` and mandatory control messages;
- no per-key thread/task/process;
- no busy retry on `WouldBlock`;
- writable readiness is armed only while bytes remain;
- a partially written frame is immutable and completed before another frame begins;
- admission that would exceed the client budget returns explicit local backpressure before ownership is accepted;
- already accepted frames are never silently dropped to make room for newer input;
- secret-bearing payloads are never logged when backpressure occurs.

The queue may store encoded frames or typed entries, but it must not serialize through JSON or another general-purpose object format.

## 11. Resize geometry derivation

### 11.1 Native proposal only

AppKit computes a rows/columns **proposal** from the usable terminal viewport in logical points and the permanent renderer's cell metrics/insets.

Conceptually:

```text
usableWidth  = max(0, viewportWidth  - horizontalInsets)
usableHeight = max(0, viewportHeight - verticalInsets)
columns = floor(usableWidth  / cellWidth)
rows    = floor(usableHeight / cellHeight)
```

The implementation must use one authoritative renderer/layout cell-metric source rather than independently re-measuring fonts in resize code.

The proposal is submitted only when both dimensions are nonzero, within SPEC-004 maxima and differ from the last requested/committed relevant geometry.

### 11.2 Backing scale

A backing-scale change may invalidate renderer resources under SPEC-005, but it must not change Runtime rows/columns unless the usable logical geometry/cell layout actually yields different rows/columns.

GPU pixel dimensions are not terminal geometry authority.

### 11.3 Layout chrome

Pass 7 has one terminal surface. Future composer/Block chrome may change the usable terminal viewport only through the same rows/columns proposal path; it may not resize a hidden GUI grid independently.

## 12. Resize ordering, coalescing and transaction

Window live-resize may generate more native geometry events than Runtime should process individually.

The client may coalesce only a not-yet-started `Resize` that is the newest queued mutation and has no intervening `Input`, `TerminalKey` or other ordering barrier. In that case the older unsent geometry may be replaced by the newest geometry.

It must not:

- mutate a partially written resize frame;
- move a resize across accepted input/key frames;
- reorder input around a resize to improve coalescing;
- build an unbounded resize backlog.

Runtime applies a received resize exactly as the accepted transaction requires:

```text
validate Controller + AttachmentId + geometry
→ prepare all locally rejectable/infallible terminal resize inputs
→ apply fallible PTY winsize
→ if PTY succeeds, commit canonical TerminalState resize
→ canonical full damage
→ normal projection update
```

If PTY winsize fails, canonical rows/columns and damage generation remain unchanged. No success acknowledgement is required for terminal progress; the client observes accepted geometry through subsequent canonical display state. Semantic errors use the existing SPEC-004 `Error` path.

Repeated identical geometry is a no-op and must not create unnecessary canonical damage.

## 13. Focus and AppKit text-input / IME seam

The permanent Metal terminal surface is first-responder capable and owns the native terminal focus target. No `NSTextView`, SwiftUI text editor or parallel terminal text surface is introduced.

The terminal surface implements the AppKit text-input-client contract required for IME/dead-key composition, directly or through a dedicated helper owned by the surface.

Required behavior:

- marked/preedit text is retained only as bounded client presentation state;
- `setMarkedText`/equivalent never submits PTY input;
- only committed `insertText`/equivalent produces `Input` UTF-8 bytes;
- `unmarkText`, cancellation, connection/controller loss and relevant focus loss clear composition without sending it;
- IME-consumed key events do not also escape through the semantic-key route;
- candidate-window geometry uses the current disposable cursor/render geometry as a presentation anchor and does not become canonical terminal state;
- composition storage is bounded to the existing maximum `Input` payload; larger commits are rejected or chunked only through bounded existing input semantics without splitting UTF-8 code units incorrectly.

M001 does not require rich inline preedit rendering inside terminal history. The permanent seam must permit that later without replacing the terminal surface or routing marked text through the PTY.

## 14. Minimum accessibility seam

Pass 7 keeps the Metal terminal surface in the native accessibility tree with:

- stable accessibility identity;
- terminal-surface label/description;
- focusability/focused-state reporting;
- geometry consistent with the visible terminal surface.

M001 does not claim a complete screen-reader text-range/transcript implementation. Later accessibility text exposure must derive from authorized terminal/history presentation state; it must not create a second VT/grid authority.

## 15. Latency instrumentation and privacy

Pass 7 keeps latency measurement active at these boundaries:

1. native event receipt → client admission result;
2. client queue admission → successful socket-frame completion;
3. Runtime frame decode/admission → PTY write completion for accepted bytes;
4. native resize proposal → client admission;
5. Runtime resize receipt → PTY winsize completion → canonical commit;
6. canonical resize commit → first display generation carrying the new geometry, where measurable without adding acknowledgement to terminal progress.

Instrumentation records only monotonic duration, byte/count/geometry sizes, result category and aggregate counters/histograms. It never records text, encoded key bytes, marked text, terminal contents, environment values or secrets.

Production protocol messages do not gain tracing IDs solely for benchmarking. Cross-process end-to-end measurements use controlled benchmark/test harnesses; production hot paths retain local low-overhead boundary metrics.

Acceptance evidence reports p50/p95/p99/max for the measured workload and environment. Pass 7 must show no material regression to Pass 6 display/output performance when the terminal is idle or receiving output.

## 16. Required tests and validation

### 16.1 Native/AppKit deterministic tests

- first-responder acceptance and focus transitions;
- one-event/one-route classification;
- Command shortcut non-leak to PTY;
- committed ASCII and non-ASCII UTF-8;
- dead-key composition commit;
- Return, Tab, Backspace, Escape and arrows;
- every Control ASCII mapping in section 6.2;
- key-repeat ordering;
- unsupported modifiers/keys do not alias to supported behavior;
- IME mark/update/unmark/commit/cancel;
- focus/controller/connection loss during composition does not submit marked text;
- no duplicate delivery through AppKit text interpretation.

### 16.2 Protocol/Runtime tests

- `CAP_SEMANTIC_TERMINAL_KEY` negotiation;
- `TerminalKey` round-trip and exact 24-byte layout;
- malformed key kind/modifiers/scalar/reserved behavior;
- Observer rejection, Controller success, stale/foreign attachment rejection;
- whole-key admission/backpressure with no partial escape sequence;
- Runtime encoder expected M001 bytes;
- accepted FIFO order across `Input`, `TerminalKey` and `Resize` barriers;
- client 262,144-byte queue limit and recovery after writable progress;
- no unbounded allocation/busy retry under stalled Runtime socket.

The `TerminalKey` decoder is included in the existing local binary-protocol fuzz target and receives retained regression seeds for malformed kinds/modifiers/scalars and truncation.

### 16.3 Resize tests

- viewport/cell metric floor calculation and bounds;
- unchanged rows/columns produces no request;
- backing-scale-only invalidation does not mutate terminal geometry;
- rapid live-resize coalesces only adjacent unsent resize work;
- input/resize/input ordering remains exact;
- partially written resize is immutable;
- observer/invalid/stale geometry rejection;
- injected PTY winsize failure leaves canonical dimensions/generation unchanged;
- successful resize commits canonical geometry only after PTY success;
- resulting projection dimensions match committed canonical state;
- repeated resize/show/hide/focus cycles do not leak resources.

### 16.4 End-to-end acceptance

Using the real production path:

```text
native AppKit terminal surface
→ seyal-client
→ SPEC-004/006 local protocol
→ Runtime
→ TerminalExecution PTY
→ shell/application
→ Seyal VT/TerminalState
→ Candidate-D display
→ permanent Metal renderer
```

prove at minimum:

- focus terminal and type a simple shell command;
- Backspace edits before submission;
- Control-C reaches the shell/application as the expected control byte;
- arrows traverse the Runtime semantic-key encoder and reach PTY in M001 normal encoding;
- resizing the window changes PTY/canonical/projection geometry consistently;
- the accepted minimal alternate-screen fixture receives the same input/resize path;
- output continues correctly while input is backpressured/stalled on a separate test client;
- observer cannot mutate the execution.

### 16.5 Performance evidence

Benchmark at minimum:

- sparse typing;
- sustained synthetic key-repeat burst;
- 1 KiB / 16 KiB / 64 KiB committed-text submissions where legal;
- repeated live-resize from 80×24 through representative larger geometries;
- input while sustained terminal output is active;
- alternate-screen input/resize;
- idle terminal before/after Pass 7 to detect new polling/CPU cost.

Record CPU/RSS plus boundary latency distributions. No benchmark may depend on logging terminal/input contents.

## 17. Failure behavior

- Runtime unavailable/disconnected: stop terminal mutation acceptance, preserve UI responsiveness, clear marked composition and surface a non-secret diagnostic state.
- `ControllerBusy`: remain explicitly noninteractive; never preempt or silently drop apparently accepted typing.
- client queue full: reject new admission explicitly; do not block AppKit/main thread.
- socket `WouldBlock`: retain accepted FIFO bytes and wait for writable readiness.
- malformed server/client protocol: use SPEC-004 failure/cleanup semantics.
- PTY closed/execution finalized: reject input/resize and release controller/client resources idempotently.
- resize rejection: keep current rendered/canonical geometry until authoritative projection changes; do not locally pretend the terminal resized.
- renderer failure: does not change input/resize/terminal authority; canonical terminal progress remains independent.

## 18. Security and privacy

Input and IME text may contain passwords, tokens and secrets. Therefore:

- no normal/error/performance log contains input payloads, semantic encoded bytes, marked text or terminal contents;
- protocol validation happens before allocation/mutation beyond bounded receive buffers;
- only the authenticated attached Controller can submit `Input`, `TerminalKey` or `Resize`;
- stale `AttachmentId` values never regain authority after reconnect;
- malformed or unsupported key events fail closed before PTY mutation;
- accessibility/IME helpers do not scrape terminal text into trusted action authority.

## 19. Explicit non-goals

Pass 7 does not implement:

```text
Pass 8 Blocks / logical anchors / structured composer
Pass 9 full GUI close/crash/reopen proof
mouse reporting
clipboard / bracketed paste production semantics
Option-as-Alt / Meta configuration
full function/navigation-key matrix
application cursor/keypad modes
kitty keyboard protocol
full Unicode grapheme/emoji width
complete terminal screen-reader transcript API
full product-grade IME inline preedit UI
tabs / splits / workspaces chrome
agents / cloud / remote / mobile
commercial features
SwiftUI/NSTextView terminal rendering
```

These are deferred, not silently approximated.

## 20. Pass 7 definition of done

Pass 7 implementation is complete only when all of these are true:

- real AppKit key event → Runtime → PTY → shell/application path works on the permanent production terminal surface;
- committed text and semantic terminal keys follow this specification;
- Runtime owns all terminal-key escape encoding and the client contains no mirrored VT/mode authority;
- interactive surface holds explicit Controller authority or is visibly noninteractive;
- input/control queue is bounded, FIFO and readiness-driven with no main-thread busy wait;
- resize obeys propose → authorize/prepare → PTY winsize → canonical commit → damage/projection;
- resize storms remain bounded without crossing input-order barriers;
- focus/IME/accessibility seams exist on the Metal surface and marked text never leaks before commit;
- deterministic/native/protocol/integration/failure/fuzz tests pass;
- exact-head latency/CPU/RSS evidence is recorded and no material output/render regression is found;
- OSS remains independent of commercial code;
- no Pass 8+ behavior is included as scope creep;
- independent final review has no unresolved red/orange blocker.
