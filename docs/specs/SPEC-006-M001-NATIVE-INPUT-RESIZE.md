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
9. Native resize convergence is driven by distinct desired, outstanding and committed client facts; correlated Runtime failures may invalidate only the resize request that actually failed.
10. A Runtime resize error never causes an immediate resend loop. Runtime-reported failures are retry-gated by an external recovery event defined in section 12.5.
11. Resize never publishes canonical geometry before the PTY accepts the winsize transaction.
12. `NSTextInputClient` exposes only a bounded ephemeral composition document. Terminal/history text is never returned through text-input APIs and never becomes a second editable text model.
13. No input/resize path waits synchronously for rendering, display projection, Block semantics, persistence, agents, cloud, telemetry or licensing.
14. Input, marked text and terminal contents are secret-bearing data and are never emitted by latency instrumentation or normal diagnostic logs.

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
- correlated resize-error handling and retry gating;
- resize coalescing with ordering barriers;
- permanent AppKit `NSTextInputClient` composition seam with explicit UTF-16 range semantics;
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

Marked/preedit text is client-local ephemeral state. It is not terminal history, is not an `Input` message and never reaches Runtime/PTY until the AppKit text-input contract commits it through `insertText` or `unmarkText` as defined in section 13.

### 4.5 Unsupported

An unsupported native event is not guessed into terminal bytes. It remains unhandled or follows ordinary native application behavior. Debug diagnostics may record only the event category/key identifier, never text content.

## 5. Event-routing order

The terminal surface must avoid duplicate delivery through `keyDown`, menu key equivalents and AppKit text interpretation, while preserving IME control of composition keys.

The behavioral order is:

1. allow recognized application/menu commands to resolve as native application commands;
2. if marked/composition state is active, give the active AppKit text-input context first opportunity to consume the event; if consumed, stop terminal routing for that event;
3. when no active composition consumed the event, recognize the supported non-text terminal keys and supported Control-key combinations from the native event;
4. route remaining text-producing input through AppKit's text-input/IME machinery;
5. submit only text that the `NSTextInputClient` contract commits as `Input`;
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
UnsupportedReplacementRange
CompositionTooLarge
```

On rejection:

- no rejected text/key payload is retained for automatic retry;
- no rejected payload is logged, copied into accessibility text or persisted;
- the terminal surface visibly reports a non-secret reason;
- the state is exposed accessibly without exposing the rejected content;
- AppKit/main-thread execution returns immediately;
- automatic replay is forbidden because rejection occurs before ownership and an implicit retry could duplicate later user intent.

For transient client-queue backpressure, the visible busy state may clear after writable progress restores local admission capacity or after a subsequent input admission succeeds. Authority/disconnection failures clear only when the corresponding connection/controller state changes. Tests assert the reason state and visibility contract, not pixel styling.

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

A tiny but positive valid viewport therefore converges to at least `1×1`, while an extremely large but finite viewport is capped at `512×256` rather than retaining stale geometry.

The implementation must use one authoritative renderer/layout cell-metric source rather than independently re-measuring fonts in resize code.

### 11.2 Backing scale

A backing-scale change may invalidate renderer resources under SPEC-005, but it must not change Runtime rows/columns unless the usable logical geometry/cell layout actually yields different rows/columns.

GPU pixel dimensions are not terminal geometry authority.

### 11.3 Layout chrome

Pass 7 has one terminal surface. Future composer/Block chrome may change the usable terminal viewport only through the same rows/columns proposal path; it may not resize a hidden GUI grid independently.

## 12. Resize ordering, reconciliation, correlation and retry policy

Window live-resize may generate more native geometry events than Runtime should process individually. Correctness requires convergence without allowing an older Runtime error to invalidate a newer resize or allowing persistent PTY failure to cause an internal resend loop.

### 12.1 Client convergence state

The client tracks:

```text
desiredGeometry          latest valid geometry derived from current native layout
outstandingGeometry      latest Resize target accepted by the client queue but not yet
                         observed as canonical through authoritative projection
outstandingResizeSeq     sequence of that latest transmitted/queued target when assigned
committedGeometry        latest geometry observed from authoritative display projection
resizeRetrySuppression   optional failed target + failure class + recovery epoch
```

`desiredGeometry` is presentation intent, not terminal authority. `committedGeometry` is observational knowledge of canonical state, not a second canonical grid. Outstanding/retry fields are bounded client transport facts only.

When a layout sample is invalid under section 11.1, no invalid target is emitted. When valid layout returns, desired geometry is recomputed.

### 12.2 Correlated Resize errors without changing framing 1.0

SPEC-004 defines a connection-local `Resize` sequence for message type 10.

Rules:

- sequence value `0` is reserved and never assigned to a normal Resize;
- client and Runtime each initialize `nextResizeSequence = 1` for a new connection;
- the client consumes a sequence only when a not-yet-started Resize frame becomes immutable and begins socket transmission; local coalescing before that point consumes no sequence;
- Runtime consumes the next sequence when it receives the corresponding complete structurally valid Resize frame, before semantic authorization/geometry/PTy application;
- because frames are FIFO, partially written frames are immutable and a broken connection resets both sides, the two counters remain aligned for a conforming client;
- for any semantic `Error` caused by that structurally valid Resize, `Error.offending_message_type = 10` and `Error.detail_code = resizeSequence`;
- `detail_code = 0` means no trustworthy Resize correlation exists and a Pass 7 client must fail closed rather than guess which request failed;
- sequence wrap is forbidden. Before `u32::MAX` would wrap to zero, the client stops admitting new Resize frames and requires reconnect/reattach; Runtime likewise never silently wraps.

The sequence is protocol bookkeeping only. It is not a terminal generation, trace ID, telemetry identifier or persistence identity.

Runtime must enqueue a Resize semantic `Error` on mandatory control output before continuing with later client mutations whose success could otherwise make the error ambiguous. Mandatory errors are never presentation-superseded.

### 12.3 Required reconciliation algorithm

Reconciliation runs when desired/committed/outstanding state changes, when local queue capacity recovers after a **local admission failure**, or when a recovery event explicitly permitted by section 12.5 occurs.

Given valid desired target `D` and no retry suppression that blocks `D`:

```text
if outstandingGeometry exists:
    if D == outstandingGeometry:
        no new Resize is needed
    else:
        try to enqueue/coalesce Resize(D) without crossing an ordering barrier
        if admission succeeds:
            outstandingGeometry = D
        if admission fails locally:
            retain D as desired and retry only on later local-capacity/state progress
else:
    if D == committedGeometry:
        no new Resize is needed
    else:
        try to enqueue Resize(D)
        if admission succeeds:
            outstandingGeometry = D
        if admission fails locally:
            retain D as desired and retry only on later local-capacity/state progress
```

The suppression rule is therefore **not** “equal to committed OR outstanding.” A desired geometry equal to committed must still be admitted when a different outstanding target could later move Runtime away from that desired geometry.

Required regression:

```text
committed = 80×24
outstanding = 100×30
desired changes back to 80×24
```

The client must queue/coalesce a restoring `80×24` target, or retain it as desired until local admission is possible. Runtime must eventually converge back to `80×24` if no Runtime failure suppresses that target.

When authoritative projection reports geometry `G`:

- set `committedGeometry = G`;
- if `outstandingGeometry == G`, clear the corresponding outstanding target/sequence;
- clear a stale visible resize-failure state if the current desired geometry is now authoritatively committed;
- rerun reconciliation, except that section 12.5 suppression still blocks automatic resend of a failed same target.

### 12.4 Ordering and coalescing

The client may coalesce only a not-yet-started `Resize` that is the newest queued mutation and has no intervening `Input`, `TerminalKey` or other ordering barrier. The unsent target may be replaced by the newest desired geometry. No Resize sequence is consumed until the final coalesced frame actually begins transmission.

It must not:

- mutate a partially written resize frame;
- move a resize across accepted input/key frames;
- reorder input around a resize to improve coalescing;
- build an unbounded resize backlog;
- discard the latest desired geometry merely because an earlier geometry equals current committed state.

### 12.5 Runtime-error classification and retry gate

A Runtime `Error` for `Resize` is **not** a retry trigger.

First correlate it using `detail_code`:

- `detail_code == 0`, a sequence greater than the last sent sequence, a duplicate impossible sequence, or an unknown error code is a protocol/compatibility failure. Surface a non-secret `ResizeProtocolFailure`, stop automatic resize submission and require explicit reconnect/recovery.
- an error sequence older than the latest outstanding Resize must never clear or replace the newer outstanding target;
- an error matching the latest outstanding sequence may clear only that matching outstanding transport fact.

Error classes are:

**Authority / connection state**

```text
InvalidState
InvalidExecution
InvalidAttachment
StaleIdentity
PermissionDenied
ControllerBusy
UnsupportedVersion
UnknownMessage
MalformedPayload
```

These disable resize mutation for the affected connection/attachment. Do not resend because of socket writability, queue capacity, projection updates or the error itself. Retry is allowed only after the corresponding authority/connection/reattach transition or an explicit recovery action establishes a new usable mutation state.

**Request / operational failure**

```text
CapacityExceeded
Backpressure
InvalidGeometry
DisplayUnavailable
InternalFailure
```

These surface a non-secret `ResizeApplyFailure` with the error category and failed target but never terminal contents. If the failed sequence is the latest outstanding request for the current desired target, install `resizeRetrySuppression` for that target. The same target is not automatically resent because outstanding state cleared, projection changed, socket became writable, local queue capacity changed or Runtime continued producing output.

A suppressed target may be retried only after one of:

1. a **new meaningful native-layout epoch** caused by viewport dimensions, insets or authoritative cell metrics changing and producing a fresh valid layout sample;
2. an explicit user/system recovery action such as “retry resize”;
3. reconnect/reattach or controller-authority recovery when that recovery is relevant to the failure class.

Each recovery event permits at most one new admission attempt for the currently desired target. A repeated Runtime failure reinstalls suppression. There is no timer-based retry, exponential-retry loop, busy retry or error→reconcile→error recursion.

Local client-queue admission failure is intentionally different: no request reached Runtime, so the latest desired geometry may retry once local queue capacity progresses, as section 12.3 states.

If an older correlated Resize fails while a newer Resize is outstanding, the older failure may be recorded as a bounded diagnostic counter/state but must not invalidate, resend or suppress the newer request. Connection/authority-class failures still apply globally because they invalidate the mutation authority itself.

### 12.6 Runtime transaction

Runtime applies a received resize exactly as:

```text
assign connection-local Resize sequence
→ validate Controller + AttachmentId + geometry
→ prepare all locally rejectable/infallible terminal resize inputs
→ apply fallible PTY winsize
→ if PTY succeeds, commit canonical TerminalState resize
→ canonical full damage
→ normal projection update
```

If PTY winsize fails, canonical rows/columns and damage generation remain unchanged and Runtime emits the correlated `InternalFailure` Resize error. No success acknowledgement is required for terminal progress; the client observes canonical geometry through display state.

Repeated identical geometry that is the current desired target and already committed with no conflicting outstanding target or active recovery request is a no-op.

## 13. Exact AppKit `NSTextInputClient` composition seam

The permanent Metal terminal surface is first-responder capable and owns the native terminal focus target. No `NSTextView`, SwiftUI text editor or parallel terminal text surface is introduced.

### 13.1 Composition-only document

For text-input protocol purposes, M001 exposes exactly one bounded ephemeral `CompositionDocument`:

```text
text                 current marked/preedit text only
selection            selection/caret inside that text only
utf16Length          NSString/NSAttributedString UTF-16 code-unit length
maxUtf8Bytes         65,536
```

It never contains:

- committed terminal input;
- visible terminal rows/cells;
- scrollback/history;
- shell prompt text;
- renderer cache text;
- semantic transcript/Blocks;
- clipboard contents.

All `NSRange` values in the `NSTextInputClient` seam are interpreted in UTF-16 code units relative to this ephemeral document, matching Foundation/AppKit string indexing. Range arithmetic is overflow checked before use.

Attributes from an incoming `NSAttributedString` are not retained in M001; only its `.string` content is kept. `validAttributesForMarkedText()` therefore returns `[]`.

The complete composition document must remain `<= 65,536` UTF-8 bytes. An update that would exceed the bound or contains invalid range arithmetic fails closed: do not mutate the prior document, surface non-secret `CompositionTooLarge`/invalid-composition state, then discard the input-method conversion session on a safe AppKit boundary without submitting marked text.

### 13.2 Required protocol methods

`hasMarkedText()`

- returns `true` iff the composition document UTF-16 length is greater than zero.

`markedRange()`

- when marked text exists with UTF-16 length `N`, returns `{0, N}`;
- otherwise returns `{NSNotFound, 0}` exactly.

`selectedRange()`

- while marked text exists, returns the stored validated selection entirely inside `0...N`;
- with no marked text, returns `{0, 0}`, representing the only insertion point in the empty composition document;
- it never reports a range into terminal/history text.

`setMarkedText(_:selectedRange:replacementRange:)`

- accepts only `NSString` or `NSAttributedString`; attributes are discarded;
- `replacementRange == {NSNotFound, 0}` means replace the current composition selection/insertion point;
- any explicit replacement range must lie wholly inside the current composition document;
- replacement and resulting selection are computed with overflow-checked UTF-16 ranges;
- the callback applies atomically to the ephemeral document only after the resulting UTF-8 size and ranges validate;
- `selectedRange` is relative to the newly supplied marked string, and the stored absolute selection is translated to the resulting composition document;
- it never submits PTY bytes.

`attributedSubstring(forProposedRange:actualRange:)`

- operates only on the ephemeral composition document;
- if the requested location lies completely outside the composition document, returns `nil` and sets `actualRange` to `{NSNotFound, 0}` when supplied;
- otherwise intersects with the document range and adjusts to valid composed-character boundaries before returning an attributed/plain substring;
- `actualRange` reports the final UTF-16 range actually returned;
- terminal/history text is never consulted as fallback context.

`insertText(_:replacementRange:)`

- accepts only `NSString` or `NSAttributedString`, using only the plain string;
- an explicit replacement range is supported only when it is `NSNotFound` or lies wholly inside the ephemeral composition document;
- a range outside that document is `UnsupportedReplacementRange`; Seyal does not pretend terminal history is editable text storage;
- the supplied string is the committed text and is admitted atomically through section 10.1;
- after the commit attempt, clear the composition document regardless of success/failure so rejected content is not retained for hidden replay;
- if admission fails, surface the non-secret `InputAdmissionFailure` reason and do not silently retry.

`unmarkText()`

- is a **commit**, not a cancellation, when marked text exists;
- snapshot the current marked plain string, atomically submit it through section 10.1 as one committed-text action, then clear the composition document regardless of admission result;
- if no marked text exists it is a no-op;
- failed admission is surfaced visibly/accessibly and is never automatically replayed.

`validAttributesForMarkedText()`

- returns `[]` in M001.

`firstRect(forCharacterRange:actualRange:)`

- never derives geometry by reading terminal/history text;
- validates/intersects the requested UTF-16 range only against the ephemeral composition document;
- returns the current disposable terminal-cursor/candidate anchor converted to **screen coordinates**;
- the M001 anchor is a zero-width caret rectangle with finite height derived from the renderer's current cell/cursor presentation metrics;
- `actualRange`, when supplied, reports the validated/intersected composition range; for the empty document `{0,0}` is valid;
- if no safe cursor/window conversion is available, return a bounded zero-width fallback at the visible terminal surface rather than inventing terminal text geometry.

`characterIndex(for:)`

- M001 has no inline per-character preedit hit-test geometry, so it returns `NSNotFound` for all screen points;
- it never maps a point into terminal cells/history and never exposes terminal text positions.

`doCommand(by:)`

- must not invoke arbitrary editing selectors against terminal/history state;
- it participates only in the event-routing disposition for the current AppKit input event. Known input-system consumption is recorded locally; otherwise the native event router may classify the original event as a supported terminal semantic key after the text-input context declines it;
- one physical event still follows exactly one route.

The optional `attributedString()` method is omitted in M001. If an SDK/platform compatibility shim requires it later, it may return only the ephemeral composition document and never terminal/history content.

Optional coordinate/text-access methods, if implemented, are likewise composition-only. `windowLevel()` reports the owning `NSWindow` level; no method may synthesize a larger document model.

### 13.3 Cancellation and lifecycle are not `unmarkText`

True cancellation is separate from `unmarkText`.

On explicit IME cancellation, relevant focus loss, connection loss, controller loss, terminal teardown, or an over-limit/invalid composition failure:

1. tell the active `NSTextInputContext` to discard marked text/conversion when appropriate;
2. clear the local composition document;
3. submit **no** marked/preedit bytes to Runtime/PTY.

This preserves the distinction between AppKit commit (`insertText` or `unmarkText`) and cancellation (`discardMarkedText`/lifecycle cleanup).

While composition is active, IME-consumed Enter/Escape/arrows/control keys do not also escape through the semantic-key route.

M001 does not require rich inline preedit rendering inside terminal history. Candidate-window placement and composition state are presentation-only seams that can evolve later without replacing the terminal surface.

## 14. Minimum accessibility seam

Pass 7 keeps the Metal terminal surface in the native accessibility tree with:

- stable accessibility identity;
- terminal-surface label/description;
- focusability/focused-state reporting;
- geometry consistent with the visible terminal surface;
- non-secret exposure of input-admission/resize failure state without rejected input contents.

M001 does not claim a complete screen-reader text-range/transcript implementation. Later accessibility text exposure must derive from authorized terminal/history presentation state; it must not be sourced from the `CompositionDocument` or create a second VT/grid authority.

## 15. Latency instrumentation, budgets and privacy

Pass 7 keeps latency measurement active at these boundaries:

1. native event receipt → client admission result;
2. client queue admission → successful socket-frame completion;
3. Runtime frame decode/admission → PTY write completion for accepted bytes;
4. native resize proposal → client admission;
5. Runtime resize receipt → PTY winsize completion → canonical commit/error classification;
6. canonical resize commit → first display generation carrying the new geometry, where measurable without adding acknowledgement to terminal progress.

Instrumentation records only monotonic duration, byte/count/geometry sizes, result category and aggregate counters/histograms. It never records text, encoded key bytes, marked text, terminal contents, environment values or secrets.

Production protocol messages do not gain tracing IDs solely for benchmarking. The connection-local Resize sequence is required correctness metadata for correlating errors, not benchmark instrumentation. Cross-process end-to-end measurements use controlled benchmark/test harnesses; production hot paths retain local low-overhead boundary metrics.

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

- **no persistent timer/poll loop** added for idle input/resize or resize-error retry;
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
- `CompositionDocument` contains only marked text and is bounded by 65,536 UTF-8 bytes;
- all composition ranges are UTF-16 and overflow checked;
- `hasMarkedText`, exact `{0,N}`/`{NSNotFound,0}` `markedRange`, and `selectedRange` semantics;
- `setMarkedText` replacement/selection behavior for `NSNotFound`, explicit in-range replacement, surrogate pairs and composed-character strings;
- out-of-document replacement never queries/replaces terminal/history text;
- `attributedSubstring` intersects out-of-bounds ranges with the composition document and never returns terminal/history text;
- `validAttributesForMarkedText == []` and incoming attributes do not escape the composition seam;
- `firstRect` uses finite screen-coordinate candidate geometry without consulting terminal/history text;
- `characterIndex(for:) == NSNotFound` for M001 and never maps terminal cells to text indices;
- `insertText` commits atomically then clears composition on both success and visible failure;
- `unmarkText` commits current marked text atomically rather than discarding it;
- `discardMarkedText`/focus/controller/connection loss clears composition with zero PTY submission;
- over-limit/invalid composition update fails closed and discards conversion without PTY submission;
- active-IME Enter/Escape/arrows/control keys are consumed by IME when appropriate and are not duplicated into terminal input;
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
- no unbounded allocation/busy retry under stalled Runtime socket;
- client/Runtime Resize sequence starts at 1, increments only for started/received complete Resize frames and never wraps to zero;
- every semantic Resize `Error` echoes the exact sequence in `detail_code`;
- uncorrelatable `detail_code == 0`, impossible future sequence and duplicate impossible sequence fail closed;
- older correlated Resize errors never invalidate a newer outstanding resize;
- Runtime queues a resize error before processing later client mutations that could make correlation ambiguous.

The `TerminalKey` decoder and Resize correlation/error-state paths are included in the existing local binary-protocol fuzz target and receive retained regression seeds for malformed kinds/modifiers/scalars, truncation, invalid/zero/impossible Resize sequence details and error ordering.

### 16.3 Resize tests

- valid viewport/cell-metric floor-and-clamp calculation;
- NaN, +Infinity and -Infinity independently injected into each of viewport width/height, horizontal/vertical insets and cell width/height produce no proposal;
- negative viewport/insets and zero/non-positive cell metrics produce no proposal;
- derived non-finite subtraction/division path produces no proposal before `floor`/conversion;
- tiny positive viewport converges to 1×1;
- huge finite viewport clamps to 512×256;
- invalid/non-positive usable viewport does not propose invalid geometry;
- no-outstanding desired==committed rows/columns produces no request;
- convergence regression: committed 80×24, outstanding 100×30, desired returns to 80×24, and 80×24 is still restored without a further native event when no Runtime failure suppresses it;
- failed **local admission** of a restoring desired resize retains desired state and retries after local queue-capacity recovery;
- projection commit/outstanding transition reruns reconciliation without bypassing runtime-error suppression;
- persistent PTY winsize failure emits one correlated error and causes zero automatic resend attempts from error/outstanding/projection/socket/queue events;
- the same failed target retries at most once after a permitted meaningful layout/recovery epoch, and repeated failure reinstalls suppression;
- an older failed resize sequence cannot clear/suppress a newer outstanding target;
- authority-class errors stop mutation until authority/attachment recovery;
- uncorrelatable/unknown resize errors surface `ResizeProtocolFailure` and do not guess/retry;
- disconnect/reattach resets sequence/retry transport state while retaining only safe desired layout intent;
- backing-scale-only invalidation does not mutate terminal geometry;
- rapid live-resize coalesces only adjacent unsent resize work and consumes no Resize sequence until transmission starts;
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
- dead-key and at least one real IME composition path update only ephemeral composition state before commit;
- `unmarkText` commit and explicit discard/cancel are demonstrably different paths;
- text-input substring/range queries cannot retrieve shell prompt, terminal row or scrollback contents;
- resizing the window changes PTY/canonical/projection geometry consistently;
- resize-away-then-return converges to the current desired geometry even when an older resize was outstanding;
- injected persistent winsize failure visibly reports one failure and does not spin/retry until a permitted recovery event;
- an older resize failure arriving while a newer request exists does not invalidate the newer target;
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
- resize-error path proving no retry loop/idle polling under persistent injected PTY winsize failure;
- input while sustained terminal output is active;
- alternate-screen input/resize;
- idle terminal before/after Pass 7 to detect new polling/CPU cost.

Record exact commit SHA, hardware/OS/build mode, run/repetition count, percentile method, baseline/result, p50/p95/p99/max, CPU/RSS, queue depth/high-water, allocations/reallocations where instrumentable, socket/write counts where instrumentable and the section 15.1 pass/fail decision. No benchmark may depend on logging terminal/input contents.

## 17. Failure behavior

- Runtime unavailable/disconnected: stop terminal mutation acceptance, preserve UI responsiveness, discard marked conversion without sending it and surface a non-secret diagnostic state.
- `ControllerBusy`: remain explicitly noninteractive; never preempt or silently drop apparently accepted typing.
- committed text >65,536 UTF-8 bytes: atomically reject the complete commit; do not chunk or submit a prefix; surface `CommitTooLarge` visibly/accessibly without retaining content.
- client queue full: atomically reject the new input action before ownership, surface `ClientBackpressure` visibly/accessibly, retain no rejected payload and do not block AppKit/main thread.
- rejected input is never automatically replayed; the user retries after the visible failure state because implicit replay can duplicate intent.
- socket `WouldBlock`: retain accepted FIFO bytes and wait for writable readiness.
- malformed server/client protocol: use SPEC-004 failure/cleanup semantics.
- PTY closed/execution finalized: reject input/resize and release controller/client resources idempotently.
- local resize admission backpressure: retain latest valid desired geometry and reconcile after **local** capacity changes because no Runtime request was sent.
- Runtime resize `Error`: correlate by Resize sequence; never invalidate a newer request; never immediately resend the failed same target; apply section 12.5 retry suppression.
- persistent/unknown resize failure: surface a non-secret failure state and wait for a permitted meaningful layout/authority/explicit recovery event, never an internal retry loop.
- uncorrelatable resize error: stop automatic resize submission and require explicit recovery/reconnect rather than guessing.
- renderer failure: does not change input/resize/terminal authority; canonical terminal progress remains independent.

## 18. Security and privacy

Input and IME text may contain passwords, tokens and secrets. Therefore:

- no normal/error/performance log contains input payloads, semantic encoded bytes, marked text or terminal contents;
- protocol validation happens before allocation/mutation beyond bounded receive buffers;
- only the authenticated attached Controller can submit `Input`, `TerminalKey` or `Resize`;
- stale `AttachmentId` values never regain authority after reconnect;
- malformed or unsupported key events fail closed before PTY mutation;
- rejected committed text is not retained for hidden automatic retry, telemetry or diagnostics;
- `InputAdmissionFailure`, `ResizeApplyFailure` and `ResizeProtocolFailure` carry only non-secret category/geometry/sequence metadata;
- `NSTextInputClient` storage/query/range/coordinate methods operate only on bounded ephemeral composition state and never return terminal/history text;
- optional text-input methods cannot be used as a backdoor to expose a terminal transcript;
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
- resize obeys desired/outstanding/committed reconciliation plus correlated request-error handling and propose → authorize/prepare → PTY winsize → canonical commit → damage/projection;
- older resize errors cannot invalidate newer requests and persistent Runtime resize failure cannot create an automatic resend loop;
- resize storms remain bounded without crossing input-order barriers, all geometry operands are validated before numeric conversion and final desired geometry converges when not retry-suppressed by an actual Runtime failure;
- the `NSTextInputClient` document is composition-only, bounded and UTF-16-range correct; terminal/history text is never returned to the input system;
- `insertText`/`unmarkText` commit semantics are distinct from cancellation/discard semantics and marked text never leaks before commit;
- focus/IME/accessibility seams exist on the Metal surface;
- deterministic/native/protocol/integration/failure/fuzz tests pass;
- exact-head latency/CPU/RSS evidence meets section 15.1 and no material Pass 6 output/render regression is found;
- all new/renamed production hot-path functions are registered in the deterministic hot-path guardrail;
- OSS remains independent of commercial code;
- no Pass 8+ behavior is included as scope creep;
- independent final architecture/performance/security review has no unresolved blocking finding.
