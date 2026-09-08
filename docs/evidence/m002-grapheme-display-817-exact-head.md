# M002 #817 exact-head grapheme display evidence record

- **Issue:** #817
- **Authority:** SPEC-011 / ADR-011 grapheme display projection and the #817 acceptance gates
- **Measured production code head:** `f6279ded5ae762d9d50e6be730a787e86bb0313d`
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
make check                                                        # exit 0
```

The production fix preserves frame boundaries when a display frame is
partially written: mandatory and after-display frames finish at frame
boundaries, so control bytes cannot be inserted into a display payload. The
grapheme attach test negotiates the display capability, receives a v2 snapshot,
and applies it through the client path.

The exact-head repository gate completed with exit 0, including the packaged
ARM64 production build, native smoke, deterministic renderer/input/recovery
self-tests, Pass 8 Runtime-to-Swift metadata acceptance, and live Candidate-D
renderer probes. The local XCTest runner could not start under the restricted
host because `com.apple.testmanagerd.control` is unavailable; the existing
canonical native CI job remains the headed acceptance boundary.

The prior canonical native CI run exposed a real startup layout defect: the
scroll document could report a transient narrow width, causing the Metal
surface to propose narrow terminal geometry and become non-hittable. Exact
head `019c5041` forces the document and surface through a completed layout pass
and makes the timeline overlay pass empty-area input through to the surface.
Focused component regressions cover initial/resize viewport tracking and
surface hit-testing. The composer Return test still needs the canonical native
CI rerun to prove the Runtime timeline Block is visibly published.

## Acceptance ledger

| Gate | Status | Evidence / remaining work |
| --- | --- | --- |
| Grapheme-capable attach and v2 snapshot | **Automated** | Runtime attach and snapshot decode/apply test pass. |
| Partial-write frame integrity | **Automated** | Regression covers display remainder followed by mandatory and after-display control frames. |
| TerminalState sole authority | **Reviewed** | Runtime remains the canonical terminal owner; client/renderer consume projections. |
| Native grapheme rendering | **Unverified** | Requires headed visual evidence with Unicode, combining, wide, and emoji cases. |
| AppKit IME and candidate interaction | **Unverified** | Requires real headed IME commit/cancel/replacement evidence. |
| Unicode-heavy latency/RSS | **Unverified** | No exact-head physical ARM64 performance matrix is retained. |
| `make check` | **Automated** | Exact head `019c5041` completed with exit 0 on the Apple Silicon host. |
| Independent review | **Pending** | Re-review of the layout/input fix is in progress; exact-head CI, native visual, manual Unicode/IME, and performance gates remain open. |

This record documents the tested boundaries and does not authorize merging or
closing #817 while the native, manual, and performance gates remain open.
