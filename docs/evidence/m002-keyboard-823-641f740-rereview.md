# Independent closing re-review — M002 keyboard architecture, issue #823

## Verdict

**NO-GO for closing #823. Do not use `Closes #823`.**

- Candidate worktree: `/tmp/seyal-oss-work/issue-823-keyboard-architecture`
- Candidate branch: `issue/823`
- Exact SHA reviewed: `641f7404c11704b47bcd5c649d45188f62941b06`
- Subject: `fix(823): encode Shift/Control Escape and reject modified keypad`
- Comparison: `origin/master...HEAD`
- Review host: Linux `6.12.94+` x86_64
- Scope: independent source and portable-test review. No production or candidate-tracked file was modified.

The requested SHA and subject matched before review and again after verification.
The candidate worktree was clean at both observations. `git diff --check
origin/master...HEAD` passed.

Authority reviewed: `AGENTS.md`; [issue #823](https://github.com/mahboobmonnamd/seyal/issues/823); SPEC-006 §21, especially §§21.2, 21.6, and 21.7; and the predecessor independent NO-GO for `889ff41`. The issue is still open, declares itself blocked, has every Acceptance checkbox unchecked, and requires production protocol/native/integration evidence in its Definition of Done. Those status and evidence gates independently preclude a closing handoff.

## Audit of the claimed fixes

### P1 from `889ff41` — Shift/Control legacy Escape

**Fixed in source for press/repeat.**

For flags 0 and 2, the non-ASCII modified-key branch now handles `Escape`
regardless of Shift/Control: it emits `ESC` when Alt is absent and `ESC ESC`
when Alt is present (`crates/seyal-runtime/src/key_v2_encode.rs:263-305`).
This complies with SPEC-006 §§21.2 and 21.6. Under flag 1, Escape remains
the required CSI-u form (`:127-200`). The new selected tests cover Shift
Escape, Control Escape, and Shift+Control Escape at flag 2
(`:805-835`). As required by §21.6, the flag-2 legacy Escape release remains
a successful no-byte event; this is not a failure to encode Escape.

### P2 from `889ff41` — modified application-keypad event after rejected press

**Fixed for flag 2.**

When flag 1 is absent, the new modifier/keypad guard returns `Err(())`
before the flag-2 application-keypad repeat/release CSI-u branch
(`crates/seyal-runtime/src/key_v2_encode.rs:246-258`). The regression
exercises a Control+keypad-Enter press/repeat/release at flags 2 and observes
three errors (`:566-598`). Thus the predecessor inconsistency—an unsupported
modified keypad press followed by a generated flag-2 event—is corrected.

## Findings

### P0

None found in this targeted source review.

### P1

None found in this targeted source review.

### P2 — flags-0 modified keypad release is accepted as a no-byte event, not rejected

**File:** `crates/seyal-runtime/src/key_v2_encode.rs:92-99,246-248`.

The new guard does not meet the implementer claim for **all** modes without
flag 1. With keyboard flags 0 and any modified keypad release:

1. the early `if flags & 2 == 0 && key.event == Release` at lines 97-99
   returns `Ok(Vec::new())`;
2. execution never reaches the later `flags & 1 == 0 && Keypad &&
   modifiers != 0` rejection at lines 246-248.

For example, Control+keypad-Enter release in either numeric or application
keypad mode at flags 0 is accepted silently. It returns no CSI-u bytes, but
it still is not the required explicit unsupported result. SPEC-006 §21.6
states that keypad without flag 1 uses its legacy form and a modified keypad
combination without accepted legacy bytes is explicitly unsupported; the
candidate’s stated requirement likewise requires `Err` for modified keypad
press, repeat, **and release** whenever flag 1 is absent.

The new regression only covers flag 2 and application keypad
(`:566-598`), so it cannot detect this flags-0 path. This remains a P2
behavioral defect and blocks closure.

### P3 / completion-gated — the exhaustive §21.6 decision matrix remains absent

SPEC-006 §21.6 requires fixtures for every V2 kind × flags 0/1/2/3 ×
press/repeat/release × accepted Shift/Alt/Control combination, classifying
each as exact bytes, successful no-byte, or explicit unsupported. Kind 17
also requires every printable ASCII base plus valid/invalid shifted-field
coverage.

The portable encoder module has nine selected unit tests
(`crates/seyal-runtime/src/key_v2_encode.rs:424-836`), and
`crates/seyal-terminal/tests/m002_keyboard.rs` has four mode/parser tests.
They are not the required finite, externally expected table. The missing
flags-0 keypad-release row above demonstrates why this is a completion gate.
This is P3 as a source-coverage classification, but remains normative
closure-blocking work.

## Evidence-gate assessment

The Linux results below establish only the compiled portable Rust boundaries.
They do not supply the following SPEC-006 §21.7 completion evidence:

- the exhaustive modes/key-bytes/negotiation matrix;
- real Runtime/PTY admission, FIFO, partial-write, saturation, persistent
  pressure, and target-TUI evidence;
- native/XCUI tests or physical keyboard, keypad, layout, dead-key, IME,
  Option-policy, Command non-leak, and no-duplicate-input evidence;
- exact-head fuzz/property campaigns;
- Release ARM64 baseline/candidate latency, CPU/RSS, and queue-high-water
  measurements; or
- the required exact-head build/test/check and independent native/security
  completion evidence.

No Debug `Seyal.app`, Metal, HID, IME, headed native environment, or
Apple-Silicon performance host was available. This review makes no
headed/native/latency claim.

## Verification performed

```text
git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture rev-parse HEAD
PASS — 641f7404c11704b47bcd5c649d45188f62941b06

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture log -1 --oneline
PASS — 641f740 fix(823): encode Shift/Control Escape and reject modified keypad

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture diff --check origin/master...HEAD
PASS — exit 0

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-runtime --locked --lib --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml -- key_v2_encode
PASS — 9 passed, 0 failed, 23 filtered out.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test m002_keyboard --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 4 passed, 0 failed.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 26 unit tests and 8 pass7_input_resize integration tests passed.
NOTE — pass7_fuzz_smoke and pass8_fuzz_smoke each contain one ignored test.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-client --locked --lib --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 10 passed, 0 failed.

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture status --short
PASS — no output; candidate worktree clean after review.
```

## Closing disposition

Keep #823 open. `641f740` fixes the predecessor Shift/Control Escape P1 and
the flag-2 modified-keypad sequence P2, but it leaves the flags-0 modified
keypad-release P2. Correct that path, add the required exhaustive §21.6
matrix, and then establish every exact-head §21.7 gate on the appropriate
native Apple-Silicon environment before another independent closing review.
