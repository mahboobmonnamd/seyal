# Independent Terra closing re-review — M002 keyboard architecture, #823

## Verdict

**GO on the source and portable-Rust scope of this SHA. NO-GO for closing #823;
do not use `Closes #823`.**

Every source P0–P2 previously raised against this candidate is now gone, and an
independent exhaustive re-derivation of the SPEC-006 §21.6 decision table found
no new source P0, P1 or P2. The named headed, native, IME, fuzz, performance and
exact-head Definition-of-Done gates remain unmet and independently preclude a
closing relationship, so #823 stays open.

- Candidate: `/tmp/seyal-oss-work/issue-823-keyboard-architecture`
- Branch: `issue/823`
- Reviewed HEAD: `a9d7f51d3440c765014ae9a93551a05e99b1b343`
- Subject: `fix(823): encode modified F3 as CSI 13;m~`
- Comparison: `origin/master...HEAD`
- Scope: independent source and portable-Rust review on Linux x86_64 only. No
  headed, native/AppKit, IME, Metal, HID, or latency claim is made or implied.

The requested SHA and subject matched exactly. The worktree was clean before and
after review, `git diff --check origin/master...HEAD` passed, and the expected
docs commit `abe00d8` plus this single production commit are present in the
range.

## Claimed predecessor fix — verified complete

The predecessor NO-GO on `e2d7f67` (retained at
`/tmp/m002-reviews/m002-keyboard-823-e2d7f67-rereview.md` and, byte-identical, at
`docs/evidence/m002-keyboard-823-e2d7f67-rereview.md`) recorded one P1: with
flags 0 or 2, a modified F3 press fell into the generic F1–F4 branch and emitted
`CSI 1;mR`, colliding with a cursor-position reply. SPEC-006 §21.6 decision 4
requires `CSI 13;m~` whenever F3 uses a modifier or event-bearing CSI form.

**The fix is complete and correctly scoped.** F3 is now special-cased in all
three encoder paths that can select a CSI form
(`crates/seyal-runtime/src/key_v2_encode.rs:154`, `:177-179`, `:222-225`,
`:276-279`), and the legacy `b'P' + value - 1` arithmetic can no longer produce
the `R` final byte because `value == 3` is excluded from it everywhere.

Confirmed byte forms for the exact rows the predecessor cited, at both flags 0
and flags 2, all seven accepted Shift/Alt/Control combinations, and both DECCKM
and DECNKM settings:

| Modifiers | Previous (defective) | This SHA |
| --- | --- | --- |
| Shift | `ESC [ 1 ; 2 R` | `ESC [ 13 ; 2 ~` |
| Alt | `ESC [ 1 ; 3 R` | `ESC [ 13 ; 3 ~` |
| Alt+Shift | `ESC [ 1 ; 4 R` | `ESC [ 13 ; 4 ~` |
| Control | `ESC [ 1 ; 5 R` | `ESC [ 13 ; 5 ~` |
| Control+Shift | `ESC [ 1 ; 6 R` | `ESC [ 13 ; 6 ~` |
| Alt+Control | `ESC [ 1 ; 7 R` | `ESC [ 13 ; 7 ~` |
| Alt+Control+Shift | `ESC [ 1 ; 8 R` | `ESC [ 13 ; 8 ~` |

Nothing adjacent regressed:

- Unmodified F3 press and repeat without flag 1 remain legacy `ESC O R`
  (`\x1bOR`), as §21.2 requires for F1–F4.
- Unmodified F3 with flag 1 remains `ESC [ 13 ; 1 ~`; flags-2 repeat and release
  remain `ESC [ 13 ; 1 : 2 ~` and `ESC [ 13 ; 1 : 3 ~`.
- F1/F2/F4 keep `CSI 1;mP/Q/S` in every modified and event-bearing form and
  `SS3 P/Q/S` unmodified.
- F5–F12 keep tilde numbers 15/17/18/19/20/21/23/24.
- No CSI output anywhere in the encoder now ends in `R`.

The dead `code` value computed for `Function` on the flag-1 path
(`key_v2_encode.rs:141-147`) is now unreachable, because every `Function` case
returns earlier. It is harmless and not a defect.

## Independent exhaustive re-derivation of §21.6

Rather than rely on reading alone, I re-derived the §21.2/§21.6 decision table
from the specification text into an independent checker and ran the candidate's
encoder against it out of tree, so the candidate worktree was never modified. The
encoder source was copied to `/tmp/m002-f3-probe/src/encoder.rs` with only the
crate-level `allow(dead_code)` attribute stripped and `encode_terminal_key_v2`
widened to `pub`; the real `seyal-core`, `seyal-exec` and `seyal-protocol` crates
from the candidate were used as path dependencies so wire validation and
`ModeState` semantics are the candidate's own.

The probe enumerated every V2 kind 1–17, every valid `value` for each kind
(Function 1–12, Keypad 0–16, ASCII 0x20–0x7e), all eight accepted modifier masks,
press/repeat/release, keyboard flags 0/1/2/3, and both DECCKM and DECNKM — 36,384
rows after `TerminalKeyV2::validate` filtering — and classified each result as
exact bytes, successful no-byte, or explicit unsupported.

**Result: 0 deviations from the independently derived table.** The checks
included §21.6 decision 1 (flag-2-absent release is a no-byte event, and with
flag 2 present release is still no-byte for Enter/Tab/Backspace, Escape, legacy
ASCII and literal keypad output because their selected output is literal
text/control bytes), decision 2 (Enter CR, Control-Backspace BS, Shift-Tab
`CSI Z`, Alt+Shift-Tab `ESC CSI Z`, Alt ESC prefixes, at every flag value),
decision 3 (`CSI 27;m u` for Escape with flag 1, `CSI value;m u` for kind 17 with
flag 1 using the unshifted value, and the §6.2 legacy Control mapping with
explicit rejection of unmapped bases), decision 4 (arrows/Home/End `CSI 1;m<final>`
versus DECCKM `SS3`, tilde navigation keys, and the F-key rows above), decision 5
(keypad `CSI code;m u` at 57399–57415, explicit rejection of modified keypad
without flag 1, the legacy numeric and application `SS3` tables, and CSI-u with
an event field for application-keypad repeat/release under flag 2 alone), and
decision 6 (`m:e` only for repeat/release, explicit `m = 1` for unmodified CSI
output). Structural §21.5 bounds also held: no output exceeded 64 bytes and no
CSI parameter field contained whitespace.

Two initial probe reports were my own derivation errors, not encoder defects, and
were corrected before the clean run: legacy Alt+Space is correctly `ESC SP`, a
literal space byte outside any CSI parameter field; and Escape release at flags 2
is correctly a no-byte event under decision 1 because its selected output at that
flag value is a literal control byte.

## Related V2 admission and encoding

I re-read the admission path for remaining defects and found none at P0–P2.

`handle_terminal_key_v2` (`crates/seyal-runtime/src/runtime/local/ingress.rs:190-285`)
keeps the §21.5 ordering: structural decode, then capability negotiation, then
action-ID monotonicity, then Controller authorization, then canonical mode
lookup, then encoding, then queue mutation. The connection-fatal classes
(unreadable frame, unnegotiated V2, non-monotonic or reused ID) route through
`fatal_terminal_key_v2` (`:71-83`), which emits exactly one generic error with
detail zero and sets `close_after_flush`. Ordinary authorization, invalid
execution, unsupported encoding and backpressure use `send_error_detail` with
`offending_message_type = 29` and `detail_code = action_id`, and are not fatal, so
other authorized actions stay independent. A successful no-byte encode submits
nothing and raises no error, which is the §21.6 decision 1 contract. Because
`send_error_detail` goes through `send_mandatory_frame`
(`crates/seyal-runtime/src/runtime/local/send.rs:15-25`, `:76-92`), a rejection
that the bounded output queue cannot accept closes the connection instead of
silently claiming success, as §21.5 requires.

`TerminalKeyV2::validate` (`crates/seyal-protocol/src/pass7.rs:1172-1213`)
enforces the §21.5 field rules that the encoder depends on: nonzero action ID,
Function 1–12, Keypad 0–16, ASCII 0x20–0x7e with A–Z rejected, Alt or Control
mandatory for ASCII, and `shifted_ascii` mandatory-and-printable with Shift and
zero without it. Function `value == 0` therefore cannot reach the encoder's
`b'P' + value - 1` arithmetic.

The screen-local keyboard flag stacks in
`crates/seyal-terminal/src/terminal.rs` match §21.4: capacity 16 with
oldest-eviction on push, pop saturating to an empty stack and flags zero,
set/clear creating a base value when empty, mask 0b11 on stored and reported
state, and independent primary/alternate stacks across screen switches. DECRQM
for private modes 1 and 66 reports the actual canonical value per §21.2.

## Findings

### P3 / completion-gated — §21.6 exhaustive fixture matrix is still absent

SPEC-006 §21.6 requires committed unit fixtures enumerating all V2 kinds × flags
0/1/2/3 × press/repeat/release × accepted Shift/Alt/Control combinations, each
classified as exact bytes, successful no-byte, or explicit unsupported, with kind
17 additionally covering every printable ASCII base and valid/invalid shifted
field. The candidate carries ten encoder unit tests and four `m002_keyboard`
tests, which is not that matrix.

My 36,384-row probe is independent review evidence that the encoder currently
satisfies the table. It is deliberately **not** a substitute for the committed
fixtures: it lives outside the repository, is not run by `make test`, and cannot
protect the table against future regression. This remains P3 and
completion-gated, consistent with the review mandate, because no source P0–P2
was found.

### Documentation nit — no recorded result in the new evidence file

`docs/evidence/m002-keyboard-823-source-p1-e2d7f67.md` names the encoder test
command under a `## Tests` heading but records no outcome. The file does
correctly disclaim headed acceptance, native/IME/TUI proof and latency, and
correctly states `Do not use Closes #823`. Cosmetic only.

## Remaining evidence gates that independently preclude closing #823

Issue #823 is **OPEN** and still marked **Blocked**, with all eight Acceptance
checkboxes unchecked. Per SPEC-006 §21.7 and the issue's Definition of Done, the
following remain unmet and cannot be established from this host:

- **Native / headed**: keyboard layouts, `input.option_as_alt` at both values,
  dead keys and IME mark/commit/cancel, candidate navigation, Command non-leak,
  physical keypad, and hardware repeat/release with no duplicate input. XCUI and
  AppKit boundaries are macOS-only; this host is Linux x86_64.
- **Modern events end to end**: a real Neovim flags-3 handshake over real
  Runtime/PTY, including Control-I versus Tab, Escape, repeated arrows, release,
  and the Enter/Tab/Backspace exceptions.
- **Wire / security and admission-recovery**: lengths 0–39 and 41+, unknown
  kind/event/version/bits, stale/observer/detach, missing capability, old and new
  peers, mode change between admission and write, FIFO and partial writes, queue
  saturation and persistent pressure, unrelated PTY fairness, and
  focus/detach/reconnect.
- **Fuzz / property**: arbitrary byte chunking and hostile negotiation, bounded
  stacks and replies, parser recovery, enum and field fuzz, and input
  ordering/authorization invariants.
- **Performance**: exact baseline and candidate on a Release ARM64 host, three
  runs of at least 1,000 accepted semantic actions on a 120x40 shell and Neovim
  workload, nearest-rank p50/p95/p99/max for native-key→Runtime admission and
  key→PTY, plus CPU, RSS and queue high-water under ordinary and high output,
  adjudicated against §15 and #673.
- **Exact-head full validation**: `make build`, `make test` and `make check` at
  this SHA, plus the §21.6 fixture matrix and independent native and security
  review.

No headed, native, IME, Metal, HID or latency evidence is invented or implied
anywhere in this report.

## Verification

```text
git rev-parse HEAD
PASS — a9d7f51d3440c765014ae9a93551a05e99b1b343

git log -1 --oneline
PASS — a9d7f51 fix(823): encode modified F3 as CSI 13;m~

git rev-parse --abbrev-ref HEAD
PASS — issue/823

git status --porcelain=v1   (before review)
PASS — no output

git diff --check origin/master...HEAD
PASS — exit 0, no whitespace errors

git log --oneline origin/master..HEAD | head -2
PASS — a9d7f51 production commit over docs commit abe00d8

diff HEAD:docs/evidence/m002-keyboard-823-e2d7f67-rereview.md against the retained /tmp copy
PASS — identical

grep for a closing keyword in the commit range
PASS — only "Do not use Closes #823" negations

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-runtime --locked --lib \
  --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml -- key_v2_encode
PASS — 10 passed, 0 failed, 23 filtered out
       (includes the new modified_f3_press_uses_tilde_csi_instead_of_cursor_report)

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked \
  --test m002_keyboard \
  --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 4 passed, 0 failed

independent out-of-tree §21.6 re-derivation (/tmp/m002-f3-probe)
PASS — 36,384 rows evaluated, 0 deviations, 0 CSI outputs ending in R,
       0 outputs over the 64-byte bound

git status --porcelain=v1   (after review)
PASS — no output; no stashes; candidate worktree unmodified
```

Host: Linux x86_64. Toolchain: `1.98.0-x86_64-unknown-linux-gnu`. Cargo options
were placed before `--` as required.

## Disposition

Accept this SHA's source change. Keep #823 **open** and do not use a closing
keyword. The next steps for completion are the committed §21.6 fixture matrix and
the remaining native, modern-event, wire/security, fuzz, performance and
exact-head evidence listed above, none of which can be produced on this host.

## Review hygiene

The only file written by this review is this report. The candidate worktree at
`/tmp/seyal-oss-work/issue-823-keyboard-architecture` remains exactly as found:
clean, on `issue/823`, at `a9d7f51`. The out-of-tree probe lives entirely under
`/tmp/m002-f3-probe` with its own workspace and lockfile, so it touched no
candidate-tracked file.
