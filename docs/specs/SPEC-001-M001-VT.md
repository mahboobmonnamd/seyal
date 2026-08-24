# SPEC-001 — M001 Seyal VT parser and terminal-state behavior

- **Status:** Active for M001
- **Issue:** #38
- **Architecture:** `docs/architecture/ADR-004-VT-STATE-OWNERSHIP.md`

## 1. Purpose

Define the observable M001 contract for Seyal's incremental VT parser and authoritative `TerminalState`. This specification narrows the supported/deferred matrix in `MILESTONE-001.md`; it does not expand M001.

## 2. Construction and dimensions

- A terminal has non-zero `cols` and `rows`.
- Construction or resize with a zero dimension fails with `TerminalError::InvalidSize`.
- A failed resize leaves existing dimensions/state unchanged.
- Cells outside the active dimensions are not addressable.
- Operations that require a new logical line identity fail explicitly with `TerminalError::LineIdentityExhausted` if the finite identity space is exhausted; they must never wrap or silently reuse an existing `LineId`.

## 3. Input framing

### 3.1 Incremental bytes

`feed(bytes)` may receive any split of the original byte stream. Supported final canonical state must be independent of feed chunk boundaries.

The parser must retain incomplete UTF-8 and escape/CSI framing between feeds. No caller may be required to align reads to characters or escape sequences.

`feed(bytes)` and `finish_input()` surface terminal-state failures such as logical-line identity exhaustion to the caller. A terminal-state failure is not reported as malformed VT input.

### 3.2 UTF-8

- ASCII printable bytes `0x20..=0x7e` print their scalar value.
- Valid 2/3/4-byte UTF-8 scalars print one scalar into the current cell path.
- Invalid UTF-8 emits U+FFFD, increments malformed diagnostics and reprocesses a following non-continuation byte as fresh input.
- An incomplete UTF-8 scalar remains pending across `feed` calls.
- `finish_input()` converts an incomplete UTF-8 scalar to U+FFFD and resets incomplete parser framing.
- M001 does not claim grapheme/emoji/East-Asian-width correctness beyond storing the scalar.

### 3.3 Bounded parser state

- CSI parameter storage is fixed and bounded.
- Parameter overflow/unsupported syntax is consumed safely as deferred input.
- OSC/DCS/SOS/PM/APC payloads must not cause unbounded accumulation in the M001 parser.

## 4. C0 controls

M001 semantics:

| Control | Behavior |
|---|---|
| BS | move cursor left by one, clamped at column 0 |
| HT | move to the next 8-column tab boundary, clamped to last column |
| LF/VT/FF | line feed; at bottom row perform full-screen upward scroll |
| CR | move to column 0 |

When full-screen line feed or pending-wrap scroll creates a new bottom row, the row is blanked using the current pen background color and otherwise default cell attributes. This matches the erase/blanking color model and prevents scrolling from exposing default-background cells inside an active background rendition.

Other C0 controls may be consumed with no visible mutation unless later specified.

## 5. Printable output and wrap

- Printable output writes the active pen `Style` into the addressed cell.
- After writing the final column, the cursor enters pending-wrap state.
- The next printable scalar wraps to column 0 of the next line before printing.
- Wrapping at the bottom performs the same full-screen scroll as line feed.
- Cursor motion/control that repositions the cursor cancels pending wrap where applicable.

## 6. Cursor CSI

Parameters use VT defaults: omitted/zero movement counts default to 1.

Supported:

- `CSI Ps A` CUU
- `CSI Ps B` CUD
- `CSI Ps C` CUF
- `CSI Ps D` CUB
- `CSI row ; col H` CUP
- `CSI row ; col f` HVP
- `CSI col G` CHA
- `CSI row d` VPA

Coordinates are 1-based in the byte protocol and clamped to the active grid.

## 7. Save/restore cursor

M001 supports:

- `CSI s` / `CSI u`
- `ESC 7` / `ESC 8`

Saved state includes cursor position, pending-wrap state and active pen style. Restore clamps to current dimensions after resize.

The saved-cursor slot belongs to the current `Screen`. The M001 `?1049` subset in §11 does not overwrite an existing primary saved-cursor slot.

## 8. Erase

### 8.1 ED (`CSI Ps J`)

- `0`: cursor cell through end of display
- `1`: beginning of display through cursor cell
- `2`: entire display

### 8.2 EL (`CSI Ps K`)

- `0`: cursor cell through end of line
- `1`: beginning of line through cursor cell
- `2`: entire line

Erased cells become spaces using the current pen background and otherwise default style.

Unsupported erase modes have no cell mutation.

## 9. SGR and colors

Supported SGR parameters:

- `0` reset
- `1` bold/intensity seam on
- `22` bold/intensity seam off
- `4` underline seam on
- `24` underline seam off
- `7` inverse seam on
- `27` inverse seam off
- `30..37`, `90..97` foreground indexed ANSI colors
- `40..47`, `100..107` background indexed ANSI colors
- `39`, `49` default foreground/background
- `38;5;n`, `48;5;n` indexed 256-color representation
- `38;2;r;g;b`, `48;2;r;g;b` truecolor representation

Other SGR parameters are deferred: they do not corrupt subsequent supported SGR state and increment deferred diagnostics.

`Color` is canonical semantic representation, not a renderer palette lookup result.

## 10. DEC private modes in M001

Supported:

- `CSI ?25h/l` — cursor visibility
- `CSI ?1049h/l` — minimal alternate-screen subset defined below

Other private modes are deferred and must not corrupt parser continuity.

## 11. Primary and alternate screens

`CSI ?1049h/l` in M001 is deliberately narrower than a claim of complete xterm mode-1049 DECSC/DECRC compatibility.

On `?1049h`:

- the primary `Screen` remains unchanged, including its cursor, active pen and any existing `ESC 7` / `CSI s` saved-cursor state;
- a clean alternate screen with the same dimensions is activated;
- alternate rows receive fresh `LineId` values from the same terminal-owned allocator;
- the alternate active pen is initialized from the current primary pen;
- a printed alternate cell therefore receives the full inherited pen style;
- untouched blank cells in the clean alternate grid carry only the inherited background color with otherwise default cell attributes.

While alternate is active:

- cursor, pen and saved-cursor mutations belong to the alternate `Screen` lifetime;
- alternate rendition/cursor changes do not mutate the preserved primary screen.

On `?1049l`:

- the alternate screen is discarded;
- the unchanged primary screen is revealed with its pre-entry cursor, pen and saved-cursor slot intact.

Re-entering while already active and leaving while already on primary are idempotent. Resize while alternate is active resizes both primary and alternate buffers so leaving alternate does not reveal stale dimensions.

M001 does **not** claim the broader xterm behavior in which mode 1049 itself participates in DECSC/DECRC save-slot semantics. That interaction is deferred until it is deliberately implemented and conformance-tested together with the required cursor-save behavior.

## 12. Resize

- Retained top-left cell content is preserved across resize.
- Retained logical row IDs remain stable.
- New rows receive new `LineId` values from the terminal-owned allocator.
- Cursor and saved cursor are clamped to new dimensions.
- Resize clears pending-wrap state.
- Resize produces full-screen damage.
- Before a multi-screen resize commits, enough fresh IDs for all newly created primary/alternate rows must be available; identity exhaustion must not leave one screen resized and the other stale.

Production reflow is deferred.

## 13. Logical line identity

- Every visible screen row has a `LineId`.
- `TerminalState` owns the single allocator for all logical line identities during its lifetime.
- Ordinary cell/style/cursor mutations do not change a retained row's ID.
- Full-screen line-feed scrolling moves the old lower-row ID with its content and allocates a new ID for the new bottom row.
- Primary construction, resize growth and every alternate-screen lifetime draw from the same allocator; screen objects do not own independent namespaces/counters.
- A `LineId` is never reused during the `TerminalState` lifetime.
- Finite-space exhaustion is explicit (`TerminalError::LineIdentityExhausted`) rather than wrapping, saturating or duplicating an ID.
- Viewport row number is never the durable Block/history anchor.

## 14. Damage

- Construction publishes initial full-screen damage.
- Mutating `feed`/resize transactions advance a monotonic generation.
- Multiple mutations within one feed are coalesced before commit.
- If previously published damage has not been consumed, subsequent generations coalesce row bounds/full flag while exposing the newest generation.
- Pure parser framing or pen-only SGR changes need not publish cell damage until visible state changes.

## 15. Deferred, unknown and malformed input

`Diagnostics` exposes monotonically increasing counters:

- `deferred_sequences`
- `unknown_sequences`
- `malformed_sequences`

Deferred/unknown input must be consumed without panic, memory-unsafety, unbounded payload allocation, or corruption of later supported parsing.

M001-tested deferred families include character/line editing, scroll-region commands, unsupported modes, OSC strings and escape-intermediate charset forms. Full semantics remain deferred.

## 16. Explicit non-goals

This specification does not claim:

- PTY or child lifecycle;
- runtime attach/projection protocol;
- full scroll regions or ICH/DCH/ECH/IL/DL/SU/SD semantics;
- mouse protocols;
- OSC title/CWD/hyperlink semantics;
- OSC 52;
- device replies;
- sixel/Kitty/iTerm images;
- complete grapheme/emoji/width handling;
- full xterm `?1049` interaction with DECSC/DECRC save slots beyond the M001 subset in §11;
- production scrollback/reflow;
- Vim/tmux/htop/Claude Code compatibility.

## 17. Tests and provenance

Required tests are implemented under `crates/seyal-terminal/tests/` and the repository VT fixture harness.

Historical implementation evidence reviewed for this contract comes from RILL commit `b39bf1a19ec9e24e4de9bf897f8638fd7a41f042`, notably:

- `crates/vt-engine/src/parser.rs`
- `crates/vt-engine/src/screen.rs`
- `crates/vt-engine/src/color.rs`
- `crates/vt-engine/src/diff.rs`
- `crates/vt-engine/tests/t_chip1_slice*.rs`

RILL behavior is not normative. Where RILL and this specification differ, this specification and current Seyal architecture win.

Issue #68 re-audited active-background blanking for scroll-created and newly cleared alternate cells. Issue #78 then tightened the normative `?1049` wording after comparing the implemented M001 behavior with the broader xterm DECSC/DECRC interaction: Seyal now specifies only what it implements and tests, rather than borrowing a stronger compatibility label.

Issue #71 re-audited long-lived line identity and removed the initial `u32` namespace/local-counter saturation model. Tests cover uniqueness across scroll, resize growth and repeated alternate-screen lifetimes, plus allocator boundary injection proving the final finite ID is issued at most once before explicit exhaustion.

Acceptance requires exact-byte tests for chunk boundaries, controls, CSI/SGR/color, alternate screen, resize, malformed/deferred recovery, line identity and damage, plus fuzz invariants once the VT fuzz target is activated.
