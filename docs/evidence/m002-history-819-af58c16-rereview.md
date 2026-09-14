# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for a closing `Closes #819` merge.** The issue must remain open.

The requested head fixes neither prior P1 conclusively. Resize no longer clones
the complete store, but still walks all retained source for an omitted
soft-wrapped prefix before the active surface can progress. The added
continuation control path cannot continue a sidecar-truncated first row because
the Runtime emits an empty `Truncated` snapshot, and the native consumer
intentionally does not re-request when it consumed zero leads. A separate P1
also remains in the history FFI/Metal path: it drops terminal width/role
semantics, so retained width-two glyphs do not occupy their two-cell rectangle.

This is an independent source review of the specified exact candidate, not
implementer self-approval. No production or worktree-tracked file was modified.

## Revision and scope

| Item | Value |
| --- | --- |
| Worktree | `/tmp/seyal-oss-work/issue-819-history-store` |
| Branch | `issue/819` |
| Exact SHA reviewed | `af58c167eecaa1a3e6d4fde778b119110b63abde` |
| Subject | `fix(history): reflow only the near-visible suffix and continue truncated rows` |
| Comparison | `origin/master...HEAD` |
| Diff integrity | `git diff --check origin/master...HEAD` passed |
| Host | Linux x86_64 (`Linux 6.12.94+`) |
| Authorities inspected | `AGENTS.md`, GitHub #819, ADR-010, SPEC-010, SPEC-011, production calibration, all retained #819 review/evidence records, and the exact diff/source |

`HEAD` was confirmed before review with `git rev-parse HEAD` and `git log -1
--oneline`; it matches the mandated SHA and subject.

## Findings

### P0

None identified.

### P1 — soft-wrapped retained history still forces an all-history walk during resize

**Source defect.** `HistoryStore::eager_resize_suffix` in
`crates/seyal-terminal/src/history.rs:452-499` limits copying to a near-visible
suffix, but when the first omitted source record has `SoftWrap` lineage it
calls `wrap_column_before(from, cols)`.

`wrap_column_before` at `history.rs:503-548` iterates
`self.entries()` from the oldest retained record through `from` and then
iterates every unit in every such record to compute occupancy. A long
soft-wrapped retained chain therefore performs work proportional to the entire
retained history before `Screen::prepare_resize` can construct/commit the
active surface. It avoids the predecessor's full clone, but violates the same
mandatory active-progress rule in ADR-010 §7 and SPEC-010 §7:

> A resize must not require work proportional to all retained history before
> the active surface can progress.

The new unit test at `history.rs:1411-1432` proves only the hard-break case
where `start_col == 0`. `history_store_regressions.rs:448-476` uses a tiny
retained/active soft-wrap fixture. Neither fills resident history with a
soft-wrapped prefix, exercises this all-history walk, or establishes the
specified bounded/lazy property.

Affected paths:

- `crates/seyal-terminal/src/history.rs:452-548`
- `crates/seyal-terminal/src/screen.rs:364-383`
- `crates/seyal-terminal/tests/history_store_regressions.rs:448-491`

### P1 — sidecar exhaustion at the first returned row produces zero-progress `Truncated`

**Source defect.** The new `start_unit` request field and native continuation
logic only progress when the response contains one or more lead cells. That
condition is explicitly enforced in
`macos/Seyal/Sources/RustDisplayBridge.swift:966-984`.

For an in-range row whose multi-scalar sidecar needs more than 65,536 bytes,
`HistoryCell::from_text` rejects the next cell at
`crates/seyal-protocol/src/pass7.rs:344-350`. In the Runtime pack loop,
`history_blocks.rs:116-158`, the error path:

1. truncates the sidecar back to `sidecar_at`, the start of the current row;
2. leaves the already-packed sidecar-referencing cells in `wire_cells`;
3. marks the response truncated.

`HistoryRangeSnapshot::try_encode` then rejects those dangling sidecar
references (`pass7.rs:461-470`). The Runtime's recovery loop at
`history_blocks.rs:179-199` pops that whole row. When it is the first returned
row, the sent snapshot is consequently `Truncated` with no rows and no leads.
Swift consumes it, sees `leads == 0`, and never issues the follow-up request.

For example, eight legal 8,192-byte multi-scalar graphemes in the first
history row require `8 * (2 + 8192) = 65,552` sidecar bytes. The first seven
are initially packed, the eighth triggers this path, and the recovery removes
the only row. This is a valid SPEC-011-sized payload, not malformed input.
The unread suffix remains unrecoverable—the prior P1 in another form.

The terminal-only test
`history_store_regressions.rs:423-445` verifies `skip_wire_leads` but does not
exercise Runtime packing, 64-KiB sidecar exhaustion, the zero-row
`Truncated` response, or Swift continuation. The Runtime IPC test is
`#[cfg(target_os = "macos")]` and currently covers only one combining
grapheme (`crates/seyal-runtime/tests/pass7_local_ipc.rs:231-311`).

Affected paths:

- `crates/seyal-protocol/src/pass7.rs:344-350, 461-507`
- `crates/seyal-runtime/src/runtime/local/history_blocks.rs:116-203`
- `crates/seyal-terminal/src/terminal.rs:313-420, 1455-1472`
- `macos/Seyal/Sources/RustDisplayBridge.swift:151-155, 964-984`

### P1 — history wire/FFI drops width-two cell semantics before Metal rendering

**Source defect.** `HistoryWireCell` carries `width` and `continuation`
(`crates/seyal-terminal/src/history.rs:71-78, 362-380`), but the Runtime maps
it to `HistoryCell` without either semantic
(`crates/seyal-runtime/src/runtime/local/history_blocks.rs:123-147`).
`HistoryCell` contains only scalar, colors, style/sidecar flags, and a sidecar
offset (`crates/seyal-protocol/src/pass7.rs:302-310`). The C ABI and Swift
`NativeHistoryRange.Cell` preserve the same omission
(`crates/seyal-client/src/ffi/types.rs:397-403`,
`macos/Seyal/Sources/SeyalBridge.h:81-87`, and
`macos/Seyal/Sources/RustDisplayBridge.swift:56-103`).

The live renderer decodes role/width and marks a width-two lead with
`instanceWideGlyphFlag` (`MetalTerminalRenderer.swift:1140-1192`). The history
renderer has neither step: it assigns every history `TerminalInstance` a
one-cell `size` and never sets that flag
(`MetalTerminalRenderer.swift:731-775`). Moreover, history is drawn in
render mode 2 (`MetalTerminalRenderer.swift:1300-1329`), whereas
`TerminalShaders.metal:52-55` only doubles a flagged glyph's rectangle in
render mode 1.

Thus a retained CJK/emoji width-two lead is not provided a terminal-authorized
two-cell rectangle; its continuation is only an untyped scalar-zero cell.
The result cannot meet SPEC-011 §7.2 and §12.1: width-two history must occupy
one lead plus one continuation and renderer preparation must receive the
terminal cell rectangle rather than infer width from fonts/text. This is
established by source flow; no macOS/Metal execution is claimed.

Affected paths:

- `crates/seyal-terminal/src/history.rs:71-78, 362-380`
- `crates/seyal-protocol/src/pass7.rs:302-390`
- `crates/seyal-runtime/src/runtime/local/history_blocks.rs:116-158`
- `crates/seyal-client/src/ffi/types.rs:397-403`
- `macos/Seyal/Sources/SeyalBridge.h:81-87`
- `macos/Seyal/Sources/RustDisplayBridge.swift:56-103, 937-963`
- `macos/Seyal/Sources/MetalTerminalRenderer.swift:731-775, 1140-1192, 1300-1329`
- `macos/Seyal/Sources/TerminalShaders.metal:49-55`

### P2

No additional P2 source defect is needed to determine the verdict. The
resident-history test suite does not test a resident-cap soft-wrapped prefix
or a 64-KiB sidecar continuation; those gaps are material to the P1 findings
above rather than independent evidence of a bounded implementation.

### P3

No additional P3 finding changes the closing decision.

## Audit of the two predecessor P1 findings

| Prior P1 at `2d04f6d` | Disposition at `af58c16` | Source proof |
| --- | --- | --- |
| `Screen::prepare_resize` clones/reflows all retained history | **Replaced by a new P1; not acceptable as fixed.** The full temporary-store clone was removed, and `screen.rs:364-383` now combines a suffix. But `history.rs:494-548` performs an oldest-to-suffix all-unit occupancy walk whenever that suffix follows omitted `SoftWrap` source. | This remains work proportional to all retained history before active progress, prohibited by SPEC-010 §7. |
| A >64-KiB multi-scalar sidecar row is an unrecoverable prefix | **Not fixed.** `start_unit` is encoded at `pass7.rs:198-264`, terminal skip logic respects lead/continuation boundaries at `terminal.rs:313-420,1455-1472`, and Swift has a status/merge/re-request path at `RustDisplayBridge.swift:964-984`. However, Runtime packing removes the only partially packed sidecar row and yields zero leads at `history_blocks.rs:140-199`; Swift then terminates continuation. | The first-row eight-by-8,192-byte legal case is zero-progress and loses the unread suffix from presentation. |

Positive source observations, insufficient to change the verdict:

- The suffix clone is bounded for hard-broken input and does preserve earlier
  sealed content; `truncate_from` operates from the suffix boundary instead
  of replacing the full store.
- `start_unit` skips an entire width-two lead plus its continuation, so a
  follow-up that is actually issued begins at a grapheme/cell-unit boundary.
- The candidate does not introduce a second mutable terminal history
  authority, and the prior metadata/seal/cache-accounting fixes remain present.

## Evidence-gate audit

| Required closing gate | Status at exact `af58c16` | Evidence / limitation |
| --- | --- | --- |
| Headed five-step history/reflow verification | **Unmet** | `m002-history-819-headed-manual.md` records partial earlier observation and Linux `ENVIRONMENT_UNSUPPORTED`; later attempt source head was `2d04f6d`, not `af58c16`. No Debug app, Metal pixels, HID keyboard, IME, or macOS headed PASS was observed here. Unsupported Linux is not a pass. |
| `make check` / Foundation gates | **Unmet for this SHA** | Skipped: it was not clearly short/feasible in this Linux review lane and can include unavailable native gates. Older retained records are predecessor-head evidence and are not promoted to this source-changing SHA. |
| macOS FFI, Runtime IPC, Swift and Metal presentation | **Unmet** | Not runnable on this Linux x86_64 host. `history_runtime.rs` and `pass7_local_ipc.rs` are macOS-gated; no macOS FFI/Swift/Metal execution is claimed. Static review instead found the width/role P1 above. |
| #673 physical performance/scaling acceptance | **Unmet / no accepted exact-head pass** | [#673](https://github.com/mahboobmonnamd/seyal/issues/673) remains open. Retained benchmark records identify themselves as comparative and predecessor/instrumented-head evidence, not an `af58c16` physical-host release pass. The resize P1 also invalidates the required active-window work shape for soft-wrapped retained history. |
| Bounded/lazy resize source contract | **Fail — P1** | Conditional all-history `wrap_column_before` walk before active progress. |
| Full multi-scalar retained history presentation | **Fail — P1** | Zero-progress first-row sidecar overflow; separate width-two FFI/renderer loss. |
| Required focused Linux Rust checks | **Pass, represented tests only** | Commands and exact counts below. They do not cover the source defects. |

## Verification performed

```text
git -C /tmp/seyal-oss-work/issue-819-history-store rev-parse HEAD
af58c167eecaa1a3e6d4fde778b119110b63abde

git -C /tmp/seyal-oss-work/issue-819-history-store log -1 --oneline
af58c16 fix(history): reflow only the near-visible suffix and continue truncated rows

git -C /tmp/seyal-oss-work/issue-819-history-store diff --check origin/master...HEAD
passed

cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
33 passed; 0 failed; 2 ignored
  - 27 library tests
  - 6 pass7 input/resize integration tests
  - 2 ignored fuzz-smoke tests

cargo test -p seyal-terminal --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
108 passed; 0 failed; 2 ignored
  - 39 library tests
  - 10 fixture-corpus tests
  - 20 HistoryStore regressions
  - 18 M001 VT tests
  - 9 M002 Unicode tests
  - 7 M002 VT-breadth tests
  - 3 salvage regressions
  - 2 terminfo tests
  - 2 ignored fuzz-smoke tests
```

`make check` was not run. No macOS application, pass7 local-IPC, Runtime
history, client FFI, Swift, Metal, HID/keyboard, IME, physical ARM64,
headed-manual, benchmark-matrix, or fuzz-campaign pass is claimed.

## Closing decision

**NO-GO. Do not close #819 from this candidate.**

Remaining source blockers are:

1. the all-retained-history soft-wrap occupancy walk in resize;
2. the zero-progress sidecar-overflow continuation path; and
3. loss of width-two history semantics across Runtime/FFI/Metal.

Independently, the required exact-head headed five-step PASS, `make check`,
macOS FFI/Metal verification, and #673 performance evidence are still
unmet. These are evidence-only gaps in addition to—not substitutes for—the
three P1 source defects.
