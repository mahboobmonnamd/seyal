# Independent closing re-review — M002 keyboard architecture, issue #823

## Verdict

**NO-GO for closing #823. Do not use `Closes #823`.**

- Candidate worktree: `/tmp/seyal-oss-work/issue-823-keyboard-architecture`
- Candidate branch: `issue/823`
- Exact SHA reviewed: `889ff411749a394909bf13f31d493e760f462348`
- Required subject: `fix(823): encode legacy Alt+Escape and Alt+Control+Backspace`
- Comparison: `origin/master...HEAD`
- Review host: Linux `6.12.94+` x86_64
- Scope: independent source and portable-test review. No candidate-tracked or production file was modified.

`HEAD` and the one-line subject matched the requested revision. The worktree
was clean before review, and the final cleanliness check is recorded below.
`git diff --check origin/master...HEAD` passed.

Authority reviewed: `AGENTS.md`; [issue #823](https://github.com/mahboobmonnamd/seyal/issues/823); SPEC-006 §21, especially §§21.2–21.7; and the predecessor independent NO-GO for
`c7e86c0`. The issue remains open, its acceptance items are unchecked, and its
Definition of Done requires evidence unavailable on this host. Those gates
independently preclude a closing handoff.

## What 889ff41 fixes

The two P1 byte defects found at `c7e86c0` are fixed in source:

- For Alt+Escape in legacy flags 0 or 2, the encoder's Escape-with-Alt arm
  returns `ESC ESC` (`crates/seyal-runtime/src/key_v2_encode.rs:260-296`).
  With flag 1, Escape instead follows the required CSI-u path
  (`:127-200`). The portable regression covers flags 0 and 2
  (`:733-750`).
- The Enter/Tab/Backspace branch now maps Control+Backspace to `BS` before
  adding Alt (`:100-125`), so Alt+Control+Backspace returns `ESC BS`;
  the regression is at `:752-760`.
- Moving the encoder to `key_v2_encode.rs` makes its unit table executable on
  Linux without moving local-IPC authorization or PTY submission out of the
  macOS-only Runtime ingress boundary.

The focused portable encoder suite passed, but it is selected-case coverage
rather than the required complete decision table.

## Findings

### P0

None found in this source review.

### P1 — legacy Shift/Control Escape is rejected although its required legacy form is `ESC`

**Files:** `crates/seyal-runtime/src/key_v2_encode.rs:260-300`;
`crates/seyal-protocol/src/pass7.rs:1067-1094,1172-1213`;
`macos/Seyal/Sources/TerminalInputSurface.swift:245-313`.

SPEC-006 §21.2 states that legacy Escape is `ESC`, or `ESC ESC` with Alt.
§21.6 applies the decision table to every accepted Shift/Alt/Control
combination and says Escape uses its legacy byte form when flag 1 is absent.
Shift and Control are accepted V2 modifier bits, and the native classifier
constructs a V2 Escape intent retaining those bits.

At flags 0 or 2, an Escape press or repeat with Shift-only, Control-only, or
Shift+Control reaches the non-ASCII `modifiers != 0` branch. That match handles
Escape only when Alt is present; its default returns `Err(())`. Runtime then
rejects the otherwise valid action as `MalformedPayload` (`runtime/local/ingress.rs:256-263`), so it emits no required `ESC`. For example:

- Shift+Escape, flags 0: rejected instead of `ESC`;
- Control+Escape, flags 0: rejected instead of `ESC`;
- Shift+Control+Escape, flags 2 press/repeat: rejected instead of `ESC`.

Alt variants are now correct, but that does not repair the non-Alt modifier
combinations. The new regression exercises unmodified and Alt Escape only; it
does not cover these cases. This is a source P1 because it breaks valid
semantic key delivery in the advertised bounded keyboard matrix.

### P2 — modified application-keypad repeat/release at flag 2 is encoded after its press was explicitly unsupported

**File:** `crates/seyal-runtime/src/key_v2_encode.rs:246-295`.

SPEC-006 §21.6 says that without flag 1, a keypad uses the legacy
numeric/application form and that a modified keypad combination without
accepted legacy bytes is explicitly unsupported. Flag 2 alone gives an
unmodified application-keypad repeat/release its dedicated CSI-u event form;
it does not supply a legacy representation for a modifier combination.

The encoder first rejects a modified keypad **press** through the generic
non-ASCII modifier branch. However, with flag 2 and application keypad mode,
the preceding non-press branch returns `csi_u(...)` without checking that
modifiers are zero. Thus Control+keypad-Enter (or Shift/Alt variants) has an
unsupported press but its repeat/release is emitted, for example as
`ESC [ 57414 ; 5 : 2 u` and `ESC [ 57414 ; 5 : 3 u` for Control.

The inconsistency can inject an event after the corresponding press was
rejected. It has no regression coverage. The path is a source finding; no
native execution is claimed.

### P3 / completion-gated — SPEC-006 §21.6 exhaustive matrix is still absent

**Files:** `crates/seyal-runtime/src/key_v2_encode.rs:410-761`;
`crates/seyal-terminal/tests/m002_keyboard.rs`;
`crates/seyal-protocol/tests/pass7_input_resize.rs`.

SPEC-006 §21.6 requires externally expected fixtures for every V2 kind × flags
0/1/2/3 × press/repeat/release × accepted Shift/Alt/Control combination, plus
printable-ASCII shifted-field cases. The portable encoder module has eight
selected unit tests; the terminal target has four parser/state tests; and the
protocol target has one structural V2 positive fixture plus limited negatives.
They do not enumerate the finite decision table, which is why both findings
above escaped.

This absence is P3 as a source-coverage finding, but it remains a normative
completion gate: it must be present before #823 can close.

## Predecessor P1 audit

The predecessor P1 findings at `c7e86c0` are fixed in source:

1. **Pre-attachment/unnegotiated type-29 must be fatal:** fixed.
   `runtime/local/session.rs:38-55` detects `TerminalKeyV2` rejected by the
   pre-attachment state machine and calls `fatal_terminal_key_v2`, which
   queues the bounded generic error and marks the connection for close.
2. **Native V2 routing omitted Enter, Tab, Backspace, and Escape:** fixed.
   `TerminalNativeKeyClassifier.v2` now creates V2 intents for
   Enter/Tab/Backspace (`TerminalInputSurface.swift:245-257`) and Escape
   (`:268-288`), retaining modifiers for Runtime encoding.
3. **Kitty set/clear failed to create/update the screen-local stack base:**
   fixed. `TerminalState::set_keyboard_flags` creates the base entry when the
   stack is empty and replaces the current entry otherwise
   (`crates/seyal-terminal/src/terminal.rs:500-519`).

The predecessor P2 action-ID exhaustion issue is also fixed in source:
`TerminalInputSurface.swift:854-868` stops before zero or `UInt32.max`,
surfaces a failure, and invokes existing reconnect recovery. The prior FFI
shape-validation P3 is fixed in source because
`LocalDisplayClient::submit_terminal_key_v2` calls `TerminalKeyV2::validate()`
before frame admission (`crates/seyal-client/src/local/input_resize.rs:302-341`).
These source fixes do not constitute macOS/native execution evidence.

## Evidence-gate assessment

Linux portable results support only the Rust boundaries actually compiled and
run below. They do not close the following required §21.7 gates:

- **Modes, key bytes, negotiation:** partial selected coverage only; the
  exhaustive decision matrix remains absent, and the P1/P2 source findings
  invalidate exact required outcomes.
- **Modern events and admission/recovery:** no real Runtime/PTY proof here for
  the named ordering, saturation, partial-write, persistent-pressure, or
  target-TUI cases. The macOS-gated local ingress is not executable on this
  host.
- **Native:** no Debug `Seyal.app`, Metal, HID keyboard, keypad, layout,
  dead-key, IME, or XCUI environment exists on this Linux host. No such result
  is claimed.
- **Fuzz/property:** no exact-head keyboard encoder/admission matrix or
  applicable property campaign was run by this review.
- **Performance:** no Release ARM64 baseline/candidate, three-run,
  1,000-action native-key-to-Runtime/key-to-PTY percentile, CPU/RSS, or
  queue-high-water evidence exists. Linux cannot substitute for that gate.
- **Required exact-head build/test/check and independent native/security
  review:** this review ran the requested focused portable tests only. It is
  an independent source review, but it cannot supply the required native
  review or native/manual evidence.

## Verification performed

Default `cargo` is `1.83.0`, which predates Edition 2024 support. I therefore
used `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo`.

The requested Runtime command places `--manifest-path` after Cargo's `--`
test-runner delimiter. Its literal form therefore passes that option to the
test binary and fails before running tests:

```text
rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-runtime --locked --lib -- key_v2_encode --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
FAIL (command syntax) — test binary: Unrecognized option: 'manifest-path'
```

The following equivalent command puts the Cargo option before the delimiter,
and executed the intended suite:

```text
rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-runtime --locked --lib --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml -- key_v2_encode
PASS — 8 passed, 0 failed, 23 filtered out.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test m002_keyboard --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 4 passed, 0 failed.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 26 unit tests and 8 pass7_input_resize integration tests passed.
NOTE — pass7_fuzz_smoke and pass8_fuzz_smoke each had one ignored test.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-client --locked --lib --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 10 passed, 0 failed.
NOTE — macOS-only local client/FFI code did not compile or execute on Linux.
```

Required repository checks:

```text
git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture rev-parse HEAD
PASS — 889ff411749a394909bf13f31d493e760f462348

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture log -1 --oneline
PASS — 889ff41 fix(823): encode legacy Alt+Escape and Alt+Control+Backspace

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture diff --check origin/master...HEAD
PASS — exit 0

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture status --short
PASS — no output; candidate worktree clean.
```

## Closing disposition

Keep #823 open. `889ff41` addresses the two reported legacy byte P1s, but the
remaining Shift/Control Escape P1 and modified-keypad P2 must be corrected and
covered by the required exhaustive fixture matrix. A later exact-head review
can consider closure only after every §21.7 native, Runtime/PTY, target-TUI,
fuzz/property, performance, and independent-review gate has actual evidence
from an appropriate macOS/Apple-Silicon environment.

The requested copy to
`docs/evidence/m002-keyboard-823-889ff41-rereview.md` was intentionally not
made: it would create an untracked candidate-worktree file, contrary to the
instruction to leave the candidate clean. This canonical report is retained at
the `/tmp/m002-reviews/` path above.
