# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for `Closes #819`.** Do not close the issue from
`24a13c565ebe1364da359cefae52409cd44afcdc`.

The candidate improves two concrete predecessor paths, but it still violates
the active-progress reflow rule for one arbitrarily long retained SoftWrap
chain, and it renders the right half of every retained width-two glyph under a
later continuation-cell background. Either P1 is sufficient to prevent
closing. Required exact-head macOS headed, FFI/Metal, and physical performance
evidence is also absent on this Linux x86_64 review host.

This was an independent source and verification review. No production or
candidate-tracked file was modified, and nothing was pushed.

## Candidate, scope, and authority

| Item | Value |
| --- | --- |
| Worktree | `/tmp/seyal-oss-work/issue-819-history-store` |
| Branch | `issue/819` |
| Exact SHA reviewed | `24a13c565ebe1364da359cefae52409cd44afcdc` |
| Subject | `fix(history): bound wrap-column walks and keep truncated sidecar prefixes` |
| Comparison | `origin/master...HEAD` |
| Host | Linux 6.12.94+ x86_64 |
| Diff integrity | `git diff --check origin/master...HEAD` passed |
| Authority reviewed | `AGENTS.md`; GitHub [#819](https://github.com/mahboobmonnamd/seyal/issues/819); ADR-010; SPEC-010 §7; SPEC-011 §7.2 and §12.1 |

`git rev-parse HEAD` returned
`24a13c565ebe1364da359cefae52409cd44afcdc`, and `git log -1 --oneline`
returned the required `24a13c5` subject before review. HEAD remained clean
after verification.

## Findings

### P0

None identified.

### P1 — a retained SoftWrap chain can still make resize walk all retained history

`HistoryStore::wrap_column_before` was improved over the predecessor: it now
walks backwards and stops when it encounters a hard break
(`crates/seyal-terminal/src/history.rs:508-557`). That fixes the old
unconditional traversal of hard-broken history.

It is not a bounded/lazy solution, however. A legal retained logical line may
be one SoftWrap chain spanning every resident segment. For a suffix cut in that
chain, the loop visits every preceding record/unit in the chain and materializes
each width in `Vec<Vec<u8>>`, then materializes a second flattened `Vec<u8>`
before `wrap_occupancy` runs (lines 518-557). Thus the active resize path can
still require work and temporary allocation proportional to all retained
history before it can construct the active projection.

Neither ADR-010 §7 nor SPEC-010 §7 permits that exception: older retained
history must remain lazy and a resize must not require work proportional to all
retained history before active-surface progress. The new regression test proves
that 8,000 *hard-broken* rows are skipped, followed by only a seven-unit
SoftWrap fragment (`history.rs:1462-1484`); it does not exercise one
resident-capacity SoftWrap chain.

The carry column needs bounded source/segment metadata or a bounded derived
index/cache to avoid replaying an unbounded predecessor chain on the resize
path.

### P1 — history mode paints a continuation background over the right half of a wide glyph

The candidate now transports width and continuation flags to Metal, and the
shader does expand flagged cells in render mode 2. That is necessary but not
sufficient for correct history rendering.

`applyHistoryPrepare` emits one instance for every physical history cell in
row order (`macos/Seyal/Sources/MetalTerminalRenderer.swift:731-779`). A
width-two lead receives `instanceWideGlyphFlag`, while its next continuation
receives no glyph flag. History is then drawn in a single composited mode-2
pass (`MetalTerminalRenderer.swift:1307-1335`). In the shader, a mode-2 wide
lead expands across two cells and mixes its glyph coverage
(`TerminalShaders.metal:49-55,97-105`), but the subsequently drawn
continuation has no glyph and returns its opaque cell background over the
second cell. The continuation therefore overwrites the right half of the
lead's expanded glyph.

The live surface deliberately avoids this ordering failure with a background
pass followed by a glyph-only pass (`MetalTerminalRenderer.swift:1285-1306`);
history has no equivalent second glyph pass. This fails SPEC-011 §7.2 and
§12.1's requirement that a width-two unit occupy its adjacent lead and
continuation rectangle with terminal-supplied geometry. This is source-flow
evidence only; no macOS/Metal execution is claimed.

### P2

No additional independent P2 finding is needed for the decision.

### P3

No P3 finding changes the decision.

## Audit of the three prior P1s

| Prior P1 | Disposition at `24a13c5` | Independent evidence |
| --- | --- | --- |
| `wrap_column_before` walked from the oldest retained record whenever a SoftWrap suffix cut needed a carry column | **Partially fixed; closing blocker remains.** Hard-broken prefix traversal is removed, but an unbounded current SoftWrap chain still traverses/materializes all of its retained predecessor units. | `history.rs:508-557`; the added test at `1462-1484` covers only a short chain after 8,000 hard breaks. |
| Sidecar overflow discarded the current-row sidecar prefix, Runtime then popped the row, yielding zero-lead `Truncated` | **Fixed for the identified sidecar-overflow path.** Packing now retains the valid current-row prefix, preserves its sidecar, and marks it truncated; the standard Swift request has leads to advance `start_unit`. | `HistoryRangeSnapshot::pack_source_rows` at `crates/seyal-protocol/src/pass7.rs:481-534`; the eight 8,192-byte width-two test proves seven retained leads, an adjacent continuation, encodability, and nonzero progress (`1166-1214`). Runtime consumes this helper at `history_blocks.rs:113-151`. The source result is not substituted for unavailable macOS end-to-end execution. |
| History width/continuation were dropped before Metal and mode 2 did not double width-two glyphs | **The precise transport/mode omission is fixed, but a replacement P1 remains.** Width/continuation now cross Runtime/FFI as flags and mode 2 expands wide glyphs. A following continuation instance nevertheless overpaints that expansion in the sole history draw pass. | `pass7.rs:400-416`; `SeyalBridge.h:81-89`; `MetalTerminalRenderer.swift:736-779,1307-1335`; `TerminalShaders.metal:49-55,97-105`. |

## Evidence-gate audit

| Closing gate | Status for exact `24a13c5` | Evidence / limitation |
| --- | --- | --- |
| One canonical `TerminalState` / retained-history authority | Source-review pass | No second mutable history authority was introduced by this candidate. |
| Bounded/lazy resize before active-surface progress | **Fail — P1** | An all-resident SoftWrap chain remains an all-history walk/allocation. |
| Sidecar-overflow continuation progress | Source path improved; macOS integration unverified | The protocol prefix test passes, and the prior zero-lead mechanism is absent from the reviewed Runtime flow. No macOS Runtime-to-Swift execution is available here. |
| Width-two retained history presentation | **Fail — P1** | History's single mode-2 pass lets the continuation overpaint the lead glyph's right half. |
| Required focused Linux Rust checks | Qualified pass | The requested default `cargo` commands cannot parse edition 2024 with installed Cargo 1.83.0. The same locked package commands passed using already-installed Cargo 1.98.0; details below. |
| `make check` / Foundation gates | Unverified for exact SHA | Not run in this review. Existing retained claims name predecessor heads, not `24a13c5`; this candidate changes terminal/protocol/runtime/Metal source. |
| Five-step headed history/reflow verification | Unmet | Linux x86_64 cannot supply a headed macOS/Metal pass. The retained manual record explicitly says its Linux attempts are `ENVIRONMENT_UNSUPPORTED` and its partial observation does not verify rendered content or resize reflow. |
| macOS FFI, Swift, and Metal behavior | Unmet | Not executable on this host. No macOS, Swift, or Metal claim is made. |
| SPEC-010/#673 physical ARM64 performance/resource acceptance | Unmet for exact SHA | The retained benchmark record names earlier code heads and is explicitly comparative; no exact-`24a13c5` controlled ARM64 Release evidence exists. |
| Exact-head fuzz/property campaign | Unverified | Focused package tests include ignored fuzz-smoke tests; no exact-SHA campaign was run in this review. |

## Verification results

```text
git -C /tmp/seyal-oss-work/issue-819-history-store rev-parse HEAD
24a13c565ebe1364da359cefae52409cd44afcdc

git -C /tmp/seyal-oss-work/issue-819-history-store log -1 --oneline
24a13c5 fix(history): bound wrap-column walks and keep truncated sidecar prefixes

git -C /tmp/seyal-oss-work/issue-819-history-store diff --check origin/master...HEAD
passed

cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
cargo test -p seyal-terminal --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
not runnable as written: default Cargo 1.83.0 rejects manifest feature
`edition2024`; no test binary was built by those commands.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
28 unit tests and 6 integration tests passed; 2 fuzz-smoke tests ignored.

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
40 unit tests, 10 fixture-corpus tests, 20 HistoryStore regressions,
18 M001 VT tests, 9 M002 Unicode tests, 7 M002 VT-breadth tests,
3 salvage regressions, and 2 terminfo tests passed; 2 fuzz-smoke tests ignored.
```

The compatible-toolchain rerun confirms represented Rust behavior only. It
does not cover the P1 SoftWrap-chain bound or execute the unavailable
macOS/Metal presentation path.

## Closing decision

**NO-GO — do not use `Closes #819` for `24a13c5`.**

Before a closing review can pass, the resize carry-column path must become
bounded even for a resident-capacity SoftWrap chain, and history must use a
draw ordering that cannot cover a wide lead glyph with its continuation
background. The resulting exact head then still needs the missing required
platform and performance evidence.
