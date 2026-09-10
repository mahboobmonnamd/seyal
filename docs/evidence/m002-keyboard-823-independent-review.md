# M002 #823 independent keyboard-architecture review

- **Review target:** `issue/823` at `c341f845ac9735defa6db972aec19e86ad7823cf`
- **Comparison base:** `origin/master` at `9b8408506ee2137c4b0cb20b4b2b2a92549649ab`
- **Authority reviewed:** `docs/specs/SPEC-006-M001-NATIVE-INPUT-RESIZE.md` §21, #823 acceptance/Done gates, `AGENTS.md`, and `docs/engineering/ISSUE-PROTOCOL.md`
- **Scope:** independent source and evidence review; no production changes and no headed macOS execution
- **Review date:** 2026-09-10 UTC

## Verdict

**NO-GO for merge. A closing PR is not allowed.**

The candidate preserves the important ownership boundary in source: Runtime
reads canonical `TerminalState` modes after Controller authorization, V2 has
fixed structural validation and capability gating, and the native surface does
not translate Command into a terminal modifier. Those positive properties do
not close the issue.

There is a P1 V2 rejection-handling defect, and the acceptance-required headed,
manual, target-TUI, and native-latency evidence is still absent. SPEC-006
§21.7 explicitly says source fixtures cannot replace named native evidence and
that manual IME/physical-layout evidence remains blocked until observed.

`c341f845` itself adds only the two evidence documents after production head
`9848add7`. This review applies to the exact requested branch head; it does
not treat the earlier full-check result at `fe70733` as an exact-head native
validation of the later keypad and documentation changes.

## Findings

### P0

None found in the reviewed source.

### P1 — V2 backpressure breaks the required recoverable-admission contract

`Runtime::handle_terminal_key_v2` sends a normal, action-ID-correlated
`ErrorCode::Backpressure` for a full input ingress
(`crates/seyal-runtime/src/runtime/local/ingress.rs`, `handle_terminal_key_v2`).
This is the specified ordinary rejection path: it must admit no bytes while
leaving later authorized actions independent.

The client does not recognize that response as recoverable. Its
`classify_server_error` only accepts `Backpressure` when the offending message
is `Input` (16) or legacy `TerminalKey` (17), not `TerminalKeyV2` (29)
(`crates/seyal-client/src/local/input_resize.rs`). Consequently a valid V2
capacity rejection becomes `Err(ClientError::Server(Backpressure))` during
client polling, rather than the existing non-fatal retryable input-failure
state. The V2 action ID is also not range/monotonicity-correlated on the client
as required by SPEC-006 §21.5.

This is a functional and recovery-contract violation under ordinary bounded
pressure, not merely missing test coverage. Add a focused client/runtime
fixture that fills ingress, asserts type-29 backpressure preserves the
connection and later FIFO work, and validates the echoed action-ID bounds.

### P1 — mandatory completion evidence remains absent

The exact-head ledger itself records all of the following as open:

- native/XCUI full keyboard integration;
- physical keyboard/layout, dead-key and IME observation;
- Command-shortcut non-leak observation beyond the focused synthetic shortcut
  test;
- actual target-TUI/Neovim flags-3 negotiation;
- repeat/release under high terminal output;
- Apple Silicon native-key-to-Runtime and key-to-PTY latency/resource matrix.

The 45-second protocol fuzz run is honestly labelled `ci-smoke`, not the
documented longer campaign. The prior focused XCUI shortcut test covers one
host-routing workflow only; it does not supply the native key matrix, physical
layout, composition, or performance gates. These are explicit #823 acceptance
and SPEC-006 §21.7 requirements, so their absence blocks merge even if every
source finding were fixed.

### P2 — V2 capability negotiation is enforced by the native caller, not the client admission API

`InteractiveMetalSurfaceView` checks `terminalSupportsKeyV2()` before creating
a V2 key. However, the exported FFI function and
`LocalDisplayClient::submit_terminal_key_v2` do not reject a call when
`extended_terminal_key_supported` is false
(`crates/seyal-client/src/ffi/input.rs` and
`crates/seyal-client/src/local/input_resize.rs`).

Thus another native/client call path can queue a type-29 frame to an older
server despite the stored negotiated capability being false. That violates the
§21.5 old-server rule that the new client use only M001 input and report richer
actions as unsupported; it can instead provoke a peer protocol failure. Guard
V2 at the client admission boundary, return an explicit unsupported result,
and test it independently of the Swift classifier.

### P3 — bounded held-key overflow is silent and repeat behavior is not represented at capacity

At `heldKeyboardKinds.count >= 256`, `keyDown` returns after consuming a new
action ID but neither submits the key nor sets the native visible failure state
(`macos/Seyal/Sources/TerminalInputSurface.swift`). It also takes this branch
for a repeat of an already tracked key while the map is full. SPEC-006 §21.3
requires visible rejection of a new tracked press and requires repeat/release
coverage. This is bounded and fail-closed, so it is not an input-injection
finding, but it remains an availability/UX defect.

### P3 — finite encoder, V2 authorization, and malformed-field coverage is substantially thinner than the specified matrix

The reviewed tests exercise representative V2 encoder paths and wire
round-trips, but they do not enumerate all V2 kinds × flags 0/1/2/3 ×
press/repeat/release × modifier combinations as SPEC-006 §21.6 requires.
The standalone protocol test has eight cases and the terminal keyboard test
has three. The retained fuzz harness invokes `TerminalKeyV2::decode`, but no
40-byte V2 seed is retained. The evidence also identifies thinner dedicated
V2 observer/stale/detach IPC coverage. This is not a demonstrated bypass—the
shared `authorize_mutation` path is fail-closed—but it is insufficient
evidence for the required security and compatibility matrix.

## Reviewed admission-path assessment

- **Runtime ingress and canonical modes:** positive. Both V1 and V2 authorize
  the connection/attachment before reading `TerminalState::modes()` and encode
  immediately before submitting immutable bytes to the existing bounded input
  ingress. This avoids Swift mode authority and write-time re-encoding.
- **Protocol V2:** fixed 40-byte decoding, reserved/version checks, allowed
  modifier bits, value validation, nonzero monotonic IDs, and Runtime
  capability gating are present. The P1/P2 findings are in client-side
  rejection/capability handling, not a Runtime authorization bypass.
- **Observer/stale rejection:** positive in source. The shared
  `AttachmentRegistry::authorize_mutation` requires the originating connection
  and Controller role; observer, detached, and stolen attachment identities
  are rejected before terminal-mode lookup or queue mutation.
- **IME and preedit:** source routing is plausibly fail-closed. Marked text is
  retained in the bounded composition document; commit goes through one Input
  submission, and focus/teardown failure clears composition. This is not a
  substitute for the missing real AppKit IME/dead-key evidence.
- **Command non-leak:** source checks Command before V2/semantic/text
  classification, V2 wire modifiers allow only Shift/Alt/Control, and there
  is no Command-to-Super mapping. The focused synthetic shortcut result is
  useful but does not discharge the headed/manual gate.

## Verification performed by this reviewer

On the Linux review host:

```text
git diff --check origin/master...HEAD                         PASS
cargo test -p seyal-protocol --test pass7_input_resize --locked  8 passed
cargo test -p seyal-terminal --test m002_keyboard --locked       3 passed
```

`cargo test -p seyal-runtime --test pass7_local_ipc --locked`
compiled but ran zero tests on this host because the Runtime local-IPC suite
is macOS-gated. It supplies no headed/native evidence here. No performance,
Apple Silicon, physical keyboard, IME, or display claim is made by this
review.

## Closure disposition

Do not merge this candidate and do not open or mark a PR as `Closes #823`.
After the P1/P2/P3 fixes and all mandatory evidence gates are complete, a new
independent exact-head review is required. A documentation-only evidence PR,
if one is ever needed, must use a non-closing relationship and leave #823
open.
