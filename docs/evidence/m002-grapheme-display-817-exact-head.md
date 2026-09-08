# M002 #817 exact-head grapheme display evidence record

- **Issue:** #817
- **Authority:** SPEC-011 / ADR-011 grapheme display projection and the #817 acceptance gates
- **Measured production code head:** `070b04ec2dc24818a757ecac29118765a034fa8e`
- **Recorded:** 2026-09-08
- **Host/build boundary:** local Apple Silicon macOS host
- **Claim status:** focused automated and source evidence only; native/manual/performance gates remain unverified

## Automated evidence

The exact production head passed the focused runtime/client boundaries:

```text
cargo test -p seyal-runtime --lib --locked                 # 45 passed
cargo test -p seyal-runtime --test m002_grapheme_attach --locked  # 1 passed
cargo test -p seyal-runtime --test pass7_local_ipc --locked      # 6 passed
cargo fmt --all -- --check
git diff --check
```

The production fix preserves frame boundaries when a display frame is
partially written: mandatory and after-display frames finish at frame
boundaries, so control bytes cannot be inserted into a display payload. The
grapheme attach test negotiates the display capability, receives a v2 snapshot,
and applies it through the client path.

The focused production composer Return test no longer loses the terminal
connection after the frame-order fix. It still fails to observe a visible
Block in the headed UI assertion, so visual presentation is not claimed.

## Acceptance ledger

| Gate | Status | Evidence / remaining work |
| --- | --- | --- |
| Grapheme-capable attach and v2 snapshot | **Automated** | Runtime attach and snapshot decode/apply test pass. |
| Partial-write frame integrity | **Automated** | Regression covers display remainder followed by mandatory and after-display control frames. |
| TerminalState sole authority | **Reviewed** | Runtime remains the canonical terminal owner; client/renderer consume projections. |
| Native grapheme rendering | **Unverified** | Requires headed visual evidence with Unicode, combining, wide, and emoji cases. |
| AppKit IME and candidate interaction | **Unverified** | Requires real headed IME commit/cancel/replacement evidence. |
| Unicode-heavy latency/RSS | **Unverified** | No exact-head physical ARM64 performance matrix is retained. |
| `make check` | **Unverified at this head** | A run reached the expected acceptance output, but its exit status was not retained; rerun is required before merge. |
| Independent review | **CHANGES_REQUIRED for evidence** | Frame-fix code review is GO; exact-head CI, native visual, manual Unicode/IME, and performance gates remain open. |

This record documents the tested boundaries and does not authorize merging or
closing #817 while the native, manual, and performance gates remain open.
