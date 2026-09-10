# Independent Terra closing re-review — M002 keyboard architecture, #823

## Verdict

**NO-GO for closing #823. Do not use `Closes #823`.**

- Candidate: `/tmp/seyal-oss-work/issue-823-keyboard-architecture`
- Branch: `issue/823`
- Reviewed HEAD: `e2d7f67d931de071f6a312f6a0d67ea23c376736`
- Subject: `fix(823): reject flags-0 modified keypad release`
- Comparison: `origin/master...HEAD`
- Scope: independent source and portable-Rust review on Linux x86_64 only; no headed/native claim.

The requested HEAD and subject matched. The candidate worktree was clean before
and after review, and `git diff --check origin/master...HEAD` passed.

## Claimed predecessor fix

**Verified.** `encode_terminal_key_v2` rejects a modified `Keypad` whenever
flag 1 is absent before it reaches the flags-0 release no-byte return
(`crates/seyal-runtime/src/key_v2_encode.rs:97-102`). The condition is
independent of `application_keypad`, so it covers numeric and application
keypads, press/repeat/release, and flags 0 or 2. The new regression exercises
Control+keypad-Enter release under both default numeric mode and application
keypad mode with flags 0 (`:566-618`).

The predecessor's flags-0 modified-keypad-release P2 is fixed. No remaining
P0/P1/P2 was found in the targeted keypad/Escape/release paths.

## Findings

### P1 — modified F3 press uses the cursor-position-report form

**File:** `crates/seyal-runtime/src/key_v2_encode.rs:263-293`

With flags 0 or 2, a modified F3 **press** takes the generic legacy modified
function branch. It emits `CSI 1;mR` (for example Shift+F3 is
`ESC [ 1 ; 2 R`). SPEC-006 §21.6 decision 4 instead requires `CSI 13;m~`
whenever F3 uses a modifier CSI form, specifically because `CSI ... R`
collides with a cursor-position reply. Flags 1/3 and flags-2 repeat/release
already take the correct `13;m~` path; the modified press rows do not. There
is no F3 fixture.

This is a deterministic supported-key encoding defect and blocks completion.

### P3 / completion-gated — §21.6 decision matrix is absent

SPEC-006 §21.6 requires an exhaustive fixture matrix across V2 kinds, flags
0/1/2/3, press/repeat/release, and accepted Shift/Alt/Control combinations,
plus every printable ASCII base and shifted-field validity for kind 17. The
nine encoder unit tests and four `m002_keyboard` tests are not that matrix.

The open, blocked #823 issue also has every Acceptance checkbox unchecked and
requires real Runtime/PTY, native, fuzz/property, performance, exact-head
build/test/check, and independent review evidence in its Definition of Done.
Those unmet gates independently preclude a closing relationship.

## Verification

```text
git rev-parse HEAD
PASS — e2d7f67d931de071f6a312f6a0d67ea23c376736

git log -1 --oneline
PASS — e2d7f67 fix(823): reject flags-0 modified keypad release

git diff --check origin/master...HEAD
PASS — exit 0

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-runtime --locked --lib --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml -- key_v2_encode
PASS — 9 passed, 0 failed, 23 filtered out

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test m002_keyboard --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 4 passed, 0 failed

git status --short
PASS — no output after review
```

## Disposition

Keep #823 open. Correct the modified F3 press rows, add the required §21.6
matrix, and satisfy the remaining exact-head Definition-of-Done evidence before
another closing review.
