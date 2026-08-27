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
→ desired rows/columns proposal
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
7. One accepted native input action is admitted atomically. It is never partially accepted, silently truncated, silently dropped or split differently depending on backpressure.
8. Accepted input preserves FIFO ordering. New input is explicitly rejected on bounded backpressure rather than blocking AppKit/main-thread progress or allocating unbounded memory.
9. Native resize convergence is driven by three distinct client facts: latest valid **desired** geometry, latest accepted-but-not-yet-authoritatively-observed **outstanding** target and latest authoritative **committed** projection geometry.
10. Resize never publishes canonical geometry before the PTY accepts the winsize transaction.
11. No input/resize path waits synchronously for rendering, display projection, Block semantics, persistence, agents, cloud, telemetry or licensing.
12. Input, marked text and terminal contents are secret-bearing data and are never emitted by latency instrumentation or normal diagnostic logs.

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
- desired/outstanding/committed resize reconciliation;
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

Committed text may originate from ordinary printable input, keyboard-layout transformations, dead-key composition or an input method after composition commits.

The client does not reinterpret committed Unicode text into terminal key sequences.

### 4.3 SemanticTerminalKey

Terminal-control keys that are not ordinary committed text use the additive `TerminalKey` protocol extension in section 7. Runtime maps the logical key to bytes using canonical terminal semantics.

### 4.4 CompositionState

Marked/preedit text is client-local ephemeral state. It is not an `Input` message and never reaches Runtime/PTY until AppKit commits it through `insertText` or its equivalent.

### 4.5 Unsupported

An unsupported native event is not guessed into terminal bytes. It remains unhandled or follows ordinary native application behavior. Debug diagnostics may record only the event category/key identifier, never text content.

## 5. Event-routing order

The terminal surface must avoid duplicate delivery through `keyDown`, menu key equivalents and AppKit text interpretation, while preserving IME control of composition keys.

The behavioral order is:

1. allow recognized application/menu commands to resolve as native application commands;
2. if marked/composition state is active, give the active AppKit text-input context first opportunity to consume the event; if consumed, stop terminal routing for that event;
3. when no active composition consumed the event, recognize the supported non-text terminal keys and supported Control-key combinations from the native event;
4. route remaining text-producing input through AppKit's text-input/IME machinery;
5. submit only committed text callbacks as `Input`;
6. preserve marked/preedit callbacks locally;
7. never submit one physical/native event by more than one route.

This ordering is required so Enter/Escape/arrows can commit, cancel or navigate an active IME candidate session instead of leaking to the PTY, while ordinary terminal navigation keys are not unconditionally consumed as AppKit editing selectors when no composition owns them.

A production implementation may use `NSTextInputContext`, `interpretKeyEvents` or equivalent AppKit mechanisms, but it must satisfy this observable classification/order. It must not unconditionally text-interpret navigation/function keys if that causes terminal keys to leak, disappear or be delivered twice.

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
| Control + supported ASCII base | `TerminalKey::ControlAscii` + normalized scalar | section 6.2 mapping |
| native key repeat for any supported semantic key | repeated semantic-key submissions | same ordering/encoding per occurrence |

The arrow-key encoding lives in Runtime even though M001 does not yet implement/advertise application-cursor mode. M002 may add DECCKM/application-keypad semantics to the Runtime encoder without changing the native event boundary. Pass 7 must not invent fake canonical mode state merely to demonstrate a mode toggle.

The current `seyal-m001` terminfo deliberately does not advertise cursor-key capabilities. Pass 7 tests that arrow intent crosses the Runtime encoder and reaches the PTY correctly; it does not use these arrows to claim ncurses/TUI key-discovery compatibility before the owning M002 capability work.

### 6.2 Control ASCII native normalization and Runtime mapping

`ControlAscii` is intentionally layout-aware but terminal-mode-independent. Swift/AppKit determines only the logical ASCII base scalar; Runtime still owns conversion of that scalar to the terminal control byte.

For a native event to become `ControlAscii` in M001:

1. active IME/composition handling from section 5 must not already have consumed the event;
2. the device-independent modifier set must contain `Control`;
3. `Shift` and `CapsLock` may additionally be present because they can affect the AppKit-produced character/case;
4. `Command`, `Option`, `Function`, `NumericPad` or any other semantic modifier makes the combination unsupported for `ControlAscii` in M001;
5. use that event's AppKit `charactersIgnoringModifiers` result as the layout-derived candidate, requiring exactly one Unicode scalar;
6. AppKit preserving the Shift effect in `charactersIgnoringModifiers` is deliberate here: for example a layout on which Shift+2 produces `@` may normalize Control+Shift+2 to the ASCII base `@`;
7. do not reconstruct a US keyboard from physical `keyCode`, do not remove Shift by hand and do not perform locale-dependent case conversion;
8. require the candidate to be ASCII and one of the bases below; ASCII `a..z` is normalized to `A..Z`, while ASCII uppercase is unchanged;
9. the wire `TerminalKey::ControlAscii.modifiers` contains only protocol `CONTROL`; native Shift/CapsLock are normalization inputs and are not terminal modifier state in M001.

The accepted Runtime mapping is exactly:

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

Examples are therefore defined by the active keyboard layout, not by US physical key positions. If a different layout produces a different scalar for the same hardware key, classification follows that produced scalar. A combination that does not yield one supported ASCII base is `Unsupported` and must not fall through into a different terminal-control interpretation.

This native normalization is testable as a pure boundary using `(modifier flags, charactersIgnoringModifiers)` fixtures, plus AppKit integration tests. Tests must include lowercase/uppercase letters, Shift-produced `@`, `^`, `_`, `?`, Space, the bracket/backslash family, CapsLock, and at least one synthetic non-US-layout case proving there is no hard-coded US key-position table.

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

Existing Pass 5/6 clients tolerate unknown `ServerHello.server_capabilities` bits and require only `CAP_BINARY_DISPLAY`; therefore advertising bit 2 is backward compatible with the current 1.0 client. The Pass 7 implementation must retain a regression test proving this compatibility.

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

Malformed `TerminalKey` frames return `MalformedPayload` without terminal mutation. Unknown kind values are malformed in protocol 1.0.

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

## 10. Bounded client outbound queue and exact input admission

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
- secret-bearing payloads are never logged when backpressure occurs;
- the queue may store encoded frames or typed entries, but it must not serialize through JSON or another general-purpose object format.

### 10.1 Committed-text atomicity

One non-empty AppKit committed-text callback is one logical M001 input action.

The exact behavior is:

1. convert the complete committed string to UTF-8 without lossy substitution;
2. if the result is empty, treat it as a no-op;
3. if the UTF-8 byte length exceeds SPEC-004's 65,536-byte `Input` limit, reject the **entire** commit locally as `CommitTooLarge`; M001 does not chunk it;
4. otherwise construct exactly one `Input` frame for that callback;
5. atomically admit the complete frame, including framing overhead, only if it fits the remaining 262,144-byte client queue budget;
6. if it does not fit, reject the **entire** commit locally as `ClientBackpressure` before any byte from that commit becomes owned by the queue;
7. never submit a prefix and never split a Unicode scalar or one AppKit committed callback across multiple `Input` frames in M001.

This removes the previous “rejected or chunked” ambiguity. Chunked paste/bulk-input semantics, if needed later, require an explicit later contract.

`TerminalKey` admission is likewise atomic for its complete frame. A rejected semantic key is not partially encoded or queued.

### 10.2 Visible non-silent rejection state

Because an AppKit commit has already occurred by the time the client attempts queue admission, local rejection must never be silent.

The native terminal surface owns a small non-canonical `InputAdmissionFailure` presentation state with at least these reason categories:

```text
ClientBackpressure
CommitTooLarge
LostController
Disconnected
```

On rejection:

- no rejected text/key payload is retained for automatic retry;
- no rejected payload is logged, copied into accessibility text or persisted;
- the terminal surface visibly reports a non-secret message such as “Input not sent — terminal client is busy; retry” or “Input not sent — committed text exceeds the M001 64 KiB limit”;
- the state is exposed accessibly without exposing the rejected content;
- AppKit/main-thread execution returns immediately;
- automatic replay is forbidden because rejection occurs before ownership and an implicit retry could duplicate later user intent.

For transient queue backpressure, the visible busy state may clear after writable progress restores admission capacity or after a subsequent input admission succeeds. Authority/disconnection failures clear only when the corresponding connection/controller state changes. Tests assert the reason state and visibility contract, not pixel styling.

## 11. Resize geometry derivation

### 11.1 Native proposal only

AppKit computes a rows/columns **desired proposal** from the usable terminal viewport in logical points and the permanent renderer's cell metrics/insets.

Before subtraction, division, `floor` or integer conversion, validate **every operand**:

```text
viewportWidth, viewportHeight,
horizontalInsets, verticalInsets,
cellWidth, cellHeight
```

All six values must be finite. Viewport dimensions and insets must be non-negative. Cell width/height must be strictly greater than zero.

Then compute:

```text
usableWidth  = viewportWidth  - horizontalInsets
usableHeight = viewportHeight - verticalInsets
```

Both derived usable dimensions must also be finite and strictly positive. Then compute `usableWidth / cellWidth` and `usableHeight / cellHeight`; each ratio must be finite and strictly positive before `floor` or numeric conversion.

Only after those checks:

```text
columns = clamp(floor(usableWidth  / cellWidth),  1, 512)
rows    = clamp(floor(usableHeight / cellHeight), 1, 256)
```

Clamping occurs while still in floating/numeric-safe form before conversion to the bounded integer wire type.

If any source operand is NaN or ±Infinity, any required sign constraint fails, any derived subtraction/division is non-finite, or either usable dimension is non-positive, there is **no valid desired geometry** for that layout sample and no new `Resize` is admitted from it.

A tiny but positive valid viewport therefore converges to at least `1×1`, while an extremely large but finite valid viewport is capped at `512×256` rather than retaining stale geometry.

The implementation must use one authoritative renderer/layout cell-metric source rather than independently re-measuring fonts in resize code.

### 11.2 Backing scale

A backing-scale change may invalidate renderer resources under SPEC-005, but it must not change Runtime rows/columns unless the usable logical geometry/cell layout actually yields different rows/columns.

GPU pixel dimensions are not terminal geometry authority.

### 11.3 Layout chrome

Pass 7 has one terminal surface. Future composer/Block chrome may change the usable terminal viewport only through the same rows/columns proposal path; it may not resize a hidden GUI grid independently.

## 12. Resize ordering, reconciliation, coalescing and transaction

Window live-resize may generate more native geometry events than Runtime should process individually. Correctness requires convergence even when native geometry returns to the currently committed size while an older different resize is still outstanding.

### 12.1 Client convergence state

The client tracks three independent values:

```text
desiredGeometry      latest valid geometry derived from current native layout
outstandingGeometry  latest Resize target accepted by the client queue but not yet
                     observed as canonical through authoritative projection
committedGeometry    latest geometry observed from authoritative display projection
```

`desiredGeometry` is presentation intent, not terminal authority. `committedGeometry` is observational knowledge of canonical state, not a second canonical grid. `outstandingGeometry` is only a client transport/reconciliation fact.

When a layout sample is invalid under section 11.1, `desiredGeometry` becomes unavailable for that sample and no invalid target is emitted. When valid layout returns, it is recomputed and reconciliation runs.

### 12.2 Required reconciliation algorithm

Reconciliation runs whenever any of these change:

- valid `desiredGeometry`;
- `outstandingGeometry` because of enqueue/coalescing, authoritative projection, resize error or connection/attachment reset;
- `committedGeometry` from authoritative projection;
- queue capacity/writability changes after a prior desired resize could not be admitted;
- Controller/connection authority becomes usable again.

Given a valid desired target `D`:

```text
if outstandingGeometry exists:
    if D == outstandingGeometry:
        no new Resize is needed
    else:
        try to enqueue/coalesce Resize(D) without crossing an ordering barrier
        if admission succeeds:
            outstandingGeometry = D
        if admission fails:
            retain D as desired and retry reconciliation on later capacity/state change
else:
    if D == committedGeometry:
        no new Resize is needed
    else:
        try to enqueue Resize(D)
        if admission succeeds:
            outstandingGeometry = D
        if admission fails:
            retain D as desired and retry reconciliation later
```

The suppression rule is therefore **not** “equal to committed OR outstanding.” A desired geometry equal to committed must still be admitted when a different outstanding target could later move Runtime away from that desired geometry.

Required regression example:

```text
committed = 80×24
outstanding = 100×30
desired changes back to 80×24
```

The client must queue/coalesce a restoring `80×24` target, or retain `80×24` as desired until it can be admitted. Runtime must eventually converge back to `80×24` even if no additional native resize event occurs.

When authoritative projection reports geometry `G`:

- set `committedGeometry = G`;
- if `outstandingGeometry == G`, clear `outstandingGeometry`;
- run reconciliation again because the latest desired geometry may differ.

A resize `Error`, disconnect or new attachment invalidates the transport meaning of the current outstanding target and immediately triggers reconciliation when authority/capacity permits. Desired geometry may be retained across a temporary disconnect, but it is never treated as committed until authoritative projection confirms geometry after attachment.

### 12.3 Ordering and coalescing

The client may coalesce only a not-yet-started `Resize` that is the newest queued mutation and has no intervening `Input`, `TerminalKey` or other ordering barrier. In that case the older unsent geometry may be replaced by the newest desired geometry, and `outstandingGeometry` is updated to that newest accepted target.

It must not:

- mutate a partially written resize frame;
- move a resize across accepted input/key frames;
- reorder input around a resize to improve coalescing;
- build an unbounded resize backlog;
- discard the latest desired geometry merely because an earlier geometry equals current committed state.

### 12.4 Runtime transaction

Runtime applies a received resize exactly as:

```text
validate Controller + AttachmentId + geometry
→ prepare all locally rejectable/infallible terminal resize inputs
→ apply fallible PTY winsize
→ if PTY succeeds, commit canonical TerminalState resize
→ canonical full damage
→ normal projection update
```

If PTY winsize fails, canonical rows/columns and damage generation remain unchanged. No success acknowledgement is required for terminal progress; the client observes accepted geometry through subsequent canonical display state. Semantic errors use the existing SPEC-004 `Error` path.

Repeated identical geometry that is both the current desired future target and already committed with no outstanding conflicting target is a no-op and must not create unnecessary wire work or canonical damage.

## 13. Focus and AppKit text-input / IME seam

The permanent Metal terminal surface is first-responder capable and owns the native terminal focus target. No `NSTextView`, SwiftUI text editor or parallel terminal text surface is introduced.

The terminal surface implements the AppKit text-input-client contract required for IME/dead-key composition, directly or through a dedicated helper owned by the surface.

Required behavior:

- marked/preedit text is retained only as bounded client presentation state;
- `setMarkedText`/equivalent never submits PTY input;
- only committed `insertText`/equivalent produces `Input` UTF-8 bytes;
- `unmarkText`, cancellation, connection/controller loss and relevant focus loss clear composition without sending it;
- while composition is active, IME-consumed Enter/Escape/arrows/control keys do not also escape through the semantic-key route;
- candidate-window geometry uses the current disposable cursor/render geometry as a presentation anchor and does not become canonical terminal state;
- marked/preedit storage is bounded to the existing maximum `Input` payload;
- a committed callback larger than 65,536 UTF-8 bytes is atomically rejected under section 10.1; it is not chunked in M001;
- queue-full rejection uses the visible non-secret admission-failure state in section 10.2 and never silently loses apparently accepted terminal input.

M001 does not require rich inline preedit rendering inside terminal history. The permanent seam must permit that later without replacing the terminal surface or routing marked text through the PTY.

## 14. Minimum accessibility seam

Pass 7 keeps the Metal terminal surface in the native accessibility tree with:

- stable accessibility identity;
- terminal-surface label/description;
- focusability/focused-state reporting;
- geometry consistent with the visible terminal surface;
- non-secret exposure of input-admission failure state without rejected input contents.

M001 does not claim a complete screen-reader text-range/transcript implementation. Later accessibility text exposure must derive from authorized terminal/history presentation state; it must not create a second VT/grid authority.

## 15. Latency instrumentation, budgets and privacy

Pass 7 keeps latency measurement active at these boundaries:

1. native event receipt → client admission result;
2. client queue admission → successful socket-frame completion;
3. Runtime frame decode/admission → PTY write completion for accepted bytes;
4. native resize proposal → client admission;
5. Runtime resize receipt → PTY winsize completion → canonical commit;
6. canonical resize commit → first display generation carrying the new geometry, where measurable without adding acknowledgement to terminal progress.

Instrumentation records only monotonic duration, byte/count/geometry sizes, result category and aggregate counters/histograms. It never records text, encoded key bytes, marked text, terminal contents, environment values or secrets.

Production protocol messages do not gain tracing IDs solely for benchmarking. Cross-process end-to-end measurements use controlled benchmark/test harnesses; production hot paths retain local low-overhead boundary metrics.

### 15.1 Pass 7 controlled-host performance gate

Before production implementation begins, the implementation Issue records the exact controlled Apple-Silicon host/OS/build baseline from current `master` and freezes its benchmark command/repetition/percentile method. The first implementation must meet these M001 engineering targets on that controlled host; they are acceptance targets, not universal device SLAs:

- sparse native event receipt → client admission: p99 **≤ 100 µs**;
- client admission → complete socket write when the socket is writable and uncontended: p99 **≤ 250 µs**;
- Runtime frame decode/admission → PTY write completion when PTY is writable and uncontended: p99 **≤ 250 µs**;
- controlled sparse native event receipt → PTY write completion: p99 **≤ 750 µs**;
- Runtime resize receipt → canonical commit at 120×40: p99 **≤ 1 ms**;
- Runtime resize receipt → canonical commit at the practical M001 maximum: p99 **≤ 2 ms**.

If measurement methodology cannot directly observe one cross-process composed boundary without perturbing the hot path, report the measurable component boundaries and a controlled harness-derived composition rather than adding per-event production tracing IDs.

Pass 7 must also demonstrate:

- **no persistent timer/poll loop** added for idle input/resize;
- idle CPU remains within measurement noise of the Pass 6 baseline;
- Pass 6 output/render p99 and active CPU show **no >10% regression** under the same controlled workload unless an independently reviewed measurement explains noise/host variance and the repeated-run median stays within 10%;
- steady-state client/Runtime RSS attributable to the Pass 7 idle path grows by **≤ 2 MiB** total on the controlled single-surface workload;
- accepted client input/control memory never exceeds the 262,144-byte wire-byte bound plus fixed queue/container overhead.

A target miss blocks Pass 7 unless the performance specification/target is explicitly re-reviewed with measured evidence; it must not be waived merely because functional tests pass.

Every new or renamed production hot-path function participating in input ingress, queue admission, Runtime dispatch/encoding, PTY write service or resize commit must be registered in `scripts/check-hot-path.py` as required by `docs/engineering/PERFORMANCE.md`.

## 16. Required tests and validation

### 16.1 Native/AppKit deterministic tests

- first-responder acceptance and focus transitions;
- one-event/one-route classification;
- Command shortcut non-leak to PTY;
- committed ASCII and non-ASCII UTF-8;
- one committed callback → exactly one atomic `Input` frame;
- >65,536-byte committed callback rejects completely with visible `CommitTooLarge` state and zero queued prefix;
- queue-full committed callback rejects completely with visible `ClientBackpressure` state and zero queued prefix;
- dead-key composition commit;
- Return, Tab, Backspace, Escape and arrows;
- every Control ASCII mapping in section 6.2;
- Control modifier-set fixtures for Control-only, Control+Shift and Control+CapsLock;
- Shift-produced `@`, `^`, `_`, `?` normalization;
- synthetic non-US-layout fixture proving scalar-derived rather than physical-US-keycode mapping;
- Command/Option/Function/NumericPad Control combinations are unsupported rather than aliased;
- key-repeat ordering;
- unsupported modifiers/keys do not alias to supported behavior;
- IME mark/update/unmark/commit/cancel;
- active-IME Enter/Escape/arrows/control keys are consumed by IME when appropriate and are not duplicated into terminal input;
- focus/controller/connection loss during composition does not submit marked text;
- no duplicate delivery through AppKit text interpretation.

### 16.2 Protocol/Runtime tests

- `CAP_SEMANTIC_TERMINAL_KEY` negotiation;
- a current Pass 5/6 client still accepts `ServerHello` with the new unknown-to-it capability bit set;
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

- valid viewport/cell-metric floor-and-clamp calculation;
- NaN, +Infinity and -Infinity independently injected into each of viewport width/height, horizontal/vertical insets and cell width/height produce no proposal;
- negative viewport/insets and zero/non-positive cell metrics produce no proposal;
- derived non-finite subtraction/division path produces no proposal before `floor`/conversion;
- tiny positive viewport converges to 1×1;
- huge finite viewport clamps to 512×256;
- invalid/non-positive usable viewport does not propose invalid geometry;
- no-outstanding desired==committed rows/columns produces no request;
- convergence regression: committed 80×24, outstanding 100×30, desired returns to 80×24, and 80×24 is still restored without a further native event;
- failed admission of a restoring desired resize retains desired state and retries after queue-capacity recovery;
- projection commit/outstanding transition always reruns reconciliation;
- resize Error/disconnect/reattach invalidates outstanding transport state and reconciliation can restore latest desired geometry;
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
- at least the supported Shift-produced Control ASCII cases follow section 6.2;
- arrows traverse the Runtime semantic-key encoder and reach PTY in M001 normal encoding without claiming ncurses capability;
- resizing the window changes PTY/canonical/projection geometry consistently;
- resize-away-then-return converges to the current desired geometry even when an older resize was outstanding;
- the accepted minimal alternate-screen fixture receives the same input/resize path;
- output continues correctly while input is backpressured/stalled on a separate test client;
- an intentionally filled client queue causes visible non-secret input rejection rather than silent loss or main-thread blocking;
- observer cannot mutate the execution.

### 16.5 Performance evidence

Benchmark at minimum:

- sparse typing;
- sustained synthetic key-repeat burst;
- 1 KiB / 16 KiB / 64 KiB committed-text submissions where legal;
- rejected >64 KiB committed-text path without payload logging/copy amplification beyond bounded conversion;
- repeated live-resize from 80×24 through representative larger geometries and M001 maximum;
- input while sustained terminal output is active;
- alternate-screen input/resize;
- idle terminal before/after Pass 7 to detect new polling/CPU cost.

Record exact commit SHA, hardware/OS/build mode, run/repetition count, percentile method, baseline/result, p50/p95/p99/max, CPU/RSS, queue depth/high-water, allocations/reallocations where instrumentable, socket/write counts where instrumentable and the section 15.1 pass/fail decision. No benchmark may depend on logging terminal/input contents.

## 17. Failure behavior

- Runtime unavailable/disconnected: stop terminal mutation acceptance, preserve UI responsiveness, clear marked composition and surface a non-secret diagnostic state.
- `ControllerBusy`: remain explicitly noninteractive; never preempt or silently drop apparently accepted typing.
- committed text >65,536 UTF-8 bytes: atomically reject the complete commit; do not chunk or submit a prefix; surface `CommitTooLarge` visibly/accessibly without retaining content.
- client queue full: atomically reject the new input action before ownership, surface `ClientBackpressure` visibly/accessibly, retain no rejected payload and do not block AppKit/main thread.
- rejected input is never automatically replayed; the user retries after the visible failure state because implicit replay can duplicate intent.
- socket `WouldBlock`: retain accepted FIFO bytes and wait for writable readiness.
- malformed server/client protocol: use SPEC-004 failure/cleanup semantics.
- PTY closed/execution finalized: reject input/resize and release controller/client resources idempotently.
- resize admission backpressure: retain latest valid desired geometry and reconcile after capacity changes; do not require a new native resize event.
- resize rejection: keep current rendered/canonical geometry, invalidate relevant outstanding transport state, rerun reconciliation against latest desired geometry and do not locally pretend the terminal resized.
- renderer failure: does not change input/resize/terminal authority; canonical terminal progress remains independent.

## 18. Security and privacy

Input and IME text may contain passwords, tokens and secrets. Therefore:

- no normal/error/performance log contains input payloads, semantic encoded bytes, marked text or terminal contents;
- protocol validation happens before allocation/mutation beyond bounded receive buffers;
- only the authenticated attached Controller can submit `Input`, `TerminalKey` or `Resize`;
- stale `AttachmentId` values never regain authority after reconnect;
- malformed or unsupported key events fail closed before PTY mutation;
- rejected committed text is not retained for hidden automatic retry, telemetry or diagnostics;
- `InputAdmissionFailure` state carries only non-secret reason/category data;
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
- Control-key native normalization is explicit, layout-aware and covered for Shift-produced symbols/modifier combinations without a physical-US-keycode table;
- committed-text callbacks are one-frame atomic in M001, and oversize/backpressure rejection is visible, accessible, non-secret and never silent;
- Runtime owns all terminal-key escape encoding and the client contains no mirrored VT/mode authority;
- interactive surface holds explicit Controller authority or is visibly noninteractive;
- input/control queue is bounded, FIFO and readiness-driven with no main-thread busy wait;
- resize obeys desired/outstanding/committed reconciliation plus propose → authorize/prepare → PTY winsize → canonical commit → damage/projection;
- resize storms remain bounded without crossing input-order barriers, all geometry operands are validated before numeric conversion and the final desired geometry converges without requiring another native resize event;
- focus/IME/accessibility seams exist on the Metal surface and marked text never leaks before commit;
- deterministic/native/protocol/integration/failure/fuzz tests pass;
- exact-head latency/CPU/RSS evidence meets section 15.1 and no material Pass 6 output/render regression is found;
- all new/renamed production hot-path functions are registered in the deterministic hot-path guardrail;
- OSS remains independent of commercial code;
- no Pass 8+ behavior is included as scope creep;
- independent final architecture/performance/security review has no unresolved blocking finding.
