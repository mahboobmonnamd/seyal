# ADR-011 keyboard protocol readiness draft

- **Status:** Draft
- **Related Issue:** #823
- **Coordinates with:** ADR-011, SPEC-011, SPEC-006

## Problem

The current `Runtime` key encoder still emits fixed arrow escape sequences and has no authoritative source for application-cursor / keypad state or modern keyboard-protocol negotiation. That makes `#823` unready as a production implementation slice.

The accepted architecture already says `TerminalState` is the sole owner of Unicode terminal semantics, and `Runtime` remains the sole hot-path encoder. What is missing is an explicit, authoritative mode surface that the encoder can read without inventing client-side state.

## Canonical owner

The canonical owner of keyboard mode state should remain `TerminalState`.

That means:

- DEC private mode 2027 remains owned by the terminal core, not the client.
- Application cursor / keypad behavior must be derived from terminal state observed by `Runtime`.
- The client may carry a disposable projection of mode state for presentation, but it must not become the source of truth.

## Minimal architecture change

The smallest acceptable change is to extend the authoritative terminal mode surface, then thread that state into the existing Runtime encoder.

Proposed shape:

1. Add an explicit mode field to the authoritative terminal/projection snapshot that already carries `cursor_visible` and `alternate_screen`.
2. Represent the keyboard-relevant compatibility bits there, at minimum:
   - `unicode_core` or `mode_2027`
   - `application_cursor`
   - `application_keypad` if the speced keyboard path needs it separately
   - any modern-keyboard protocol opt-in bit if `#823` is intended to cover Kitty-style semantics
3. Keep `Runtime` as the only encoder that turns semantic keys into PTY bytes.
4. Continue to reject unsupported keys at the protocol boundary instead of inventing new transport encodings.

## Wire/API changes likely required

Likely required, in the smallest bounded form:

- Add a mode snapshot field to the display/projection state so the client can see current terminal mode without becoming authoritative.
- Thread the same authoritative mode state into `Runtime::handle_terminal_key` or the helper it calls.
- If modern keyboard semantics need to be negotiated across local IPC, extend the existing capability handshake rather than overloading `TerminalKey` itself.
- If `TerminalKey` needs more than the current kind/modifier/scalar tuple, add only the minimal extra fields needed for opt-in protocol negotiation, not a second key model.

## What should not change

- Do not move key encoding into the client.
- Do not create a second terminal state owner.
- Do not add a temporary compatibility shim that hard-codes Kitty or application-cursor behavior outside the authoritative mode model.
- Do not retroactively rewrite committed terminal content.

## Suggested implementation issue split

1. Readiness issue: define the canonical mode-state owner and the local IPC/projection surface that exposes it.
2. Implementation issue: thread authoritative mode into the existing Runtime key encoder.
3. Protocol follow-up, if needed: add opt-in keyboard-protocol capability negotiation and tests.

## Required tests

- `Runtime` arrow-key encoding changes when authoritative application-cursor mode changes.
- Legacy/default mode still emits the current normal sequences.
- Control-ASCII encoding remains unchanged.
- Unsupported key/protocol combinations fail at the protocol boundary.
- The client cannot mutate keyboard mode semantics directly.
- Projection/state round-trips preserve the authoritative mode bit(s) without creating a second owner.

## Open questions

- Whether `#823` is strictly application-cursor/keypad compatibility, or whether it must also cover Kitty keyboard-protocol negotiation in the same implementation slice.
- Whether the authoritative mode bit should live in the same snapshot structure as `cursor_visible`/`alternate_screen`, or in a separate compact mode struct carried by the same snapshot.

