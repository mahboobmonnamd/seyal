# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for a closing `Closes #819` merge.** The issue must remain open.

This is an independent closing review, not implementer self-approval. The
candidate corrects the five P1/P2 concerns identified in the prior review at
the inspected source locations, but it still has two P1 source defects and
unmet final evidence gates. Either category independently prevents a closing
merge.

## Revision and scope reviewed

| Item | Value |
| --- | --- |
| Worktree | `/tmp/seyal-oss-work/issue-819-history-store` |
| Branch | `issue/819` |
| Exact SHA reviewed | `2d04f6d0d0df1d4d83e2e291a98405b63e23b598` |
| Subject | `fix(history): present multi-scalar units on the history wire and reflow the retained/active boundary` |
| Comparison | `origin/master...HEAD` |
| Diff integrity | `git diff --check origin/master...HEAD` passed |
| Authorities reviewed | `AGENTS.md`, GitHub #819, ADR-010, SPEC-010, #818 calibration record, prior independent review, headed evidence, exact-head benchmark/evidence records, and the production diff |

## Findings

### P0

None identified.

### P1 — primary resize eagerly reconstructs and reflows all retained history

**Source defect.** `crates/seyal-terminal/src/screen.rs` in
`Screen::prepare_resize` clones every retained history entry into `combined`,
copies active rows into the same temporary history store, and calls:

```rust
let reflowed = source.reflow(cols, usize::MAX);
```

The temporary store retains up to the 32 MiB resident cap and `reflow` builds a
row vector for the whole stream before selecting the last active rows. This is
work and allocation proportional to all retained history before the active
surface can progress. It directly violates ADR-010 §7 and SPEC-010 §7's
requirement that resize may eagerly materialize the active/near-visible window
but must not require work proportional to all retained history.

The new
`resize_reflows_soft_wrap_across_retained_and_active_boundary` regression in
`crates/seyal-terminal/tests/history_store_regressions.rs` proves the formerly
broken logical boundary on a small fixture. It does not establish the required
bounded/lazy resize behavior at the resident-history limit. This is a source
defect, not merely missing performance evidence.

### P1 — a valid large multi-scalar source row becomes an unrecoverable prefix

**Source defect.** The new wire sidecar is bounded at 65,536 bytes in
`crates/seyal-protocol/src/pass7.rs` (`MAX_HISTORY_SIDECAR_BYTES`), while an
accepted canonical grapheme can carry up to 8,192 bytes. In
`crates/seyal-runtime/src/runtime/local/history_blocks.rs`,
`handle_history_range_request` stops packing at sidecar exhaustion and emits a
`Truncated` snapshot containing the prefix of that row.

That bounded response is safe, but the request protocol is line-ID only—there
is no unit-offset continuation. Repeating the same range selects the same
prefix. Further, `SeyalHistoryRange` and `NativeHistoryRange` do not carry
`HistoryRangeStatus`, and
`macos/Seyal/Sources/RustDisplayBridge.swift::publishHistoryRanges` consumes
and removes the request after delivering the prefix. The native client has no
way to observe `Truncated` or request the unavailable suffix.

Thus a valid retained line containing enough accepted multi-scalar graphemes
to exceed the sidecar limit cannot be presented in full until eviction. This
does not split a grapheme, but it violates #819/SPEC-010's retained-history
availability and wide/grapheme presentation contract. No regression exercises
this boundary.

Affected paths:

- `crates/seyal-protocol/src/pass7.rs`
- `crates/seyal-runtime/src/runtime/local/history_blocks.rs`
- `crates/seyal-client/src/ffi/types.rs`
- `crates/seyal-client/src/ffi/display.rs`
- `macos/Seyal/Sources/RustDisplayBridge.swift`

### P2

No remaining P2 source finding from the previous review was identified.

The prior P2 implementations are corrected in source:

- `crates/seyal-terminal/src/history.rs` now drives sealing from
  `HistoryLine::canonical_payload_len`, while segment resident accounting
  separately includes metadata.
- `reflow_rows_allocated_bytes` uses actual vector capacities for inner rows
  and is used for cache admission/accounting rather than `len()`.

This does not cure the P1 eager full-history resize: its unbounded temporary
reflow result exists before cache admission and is separately non-conforming.

### P3

No additional P3 issue changes the verdict. The new sidecar protocol has
Rust-side encode/decode and Runtime wire coverage, but macOS Swift/FFI
execution was not run here and the available headed record is not evidence of
its rendered behavior.

## Audit of the prior P1/P2 source findings

| Prior finding | Source disposition at `2d04f6d` |
| --- | --- |
| Multi-scalar history wire failed closed as `DisplayUnavailable` | Corrected for normal bounded snapshots: `HistoryWireCell`, `HistoryCell::from_text`, the snapshot sidecar, and the Runtime packing path preserve multi-scalar UTF-8. `history_range_combining_grapheme_round_trips_over_runtime_wire` covers a combining grapheme. The distinct large-row continuation P1 above remains. |
| `source_breaks` grew without bound | Corrected: `Screen::append_row_to_history` removes each retained source ID; `source_breaks_stay_bounded_to_active_lines_after_long_output` passes. |
| `evicted_id_ranges` could outgrow the resident cap | Corrected to a fixed cardinality in `crates/seyal-terminal/src/history.rs` using `MAX_EVICTED_ID_RANGES` plus a watermark; the associated unit test passes. |
| Retained/active soft-wrap boundary was reflowed separately | Corrected for the functional small case by the combined stream reconstruction and the new boundary regression. The replacement has the separate eager-full-history P1 above. |
| 16 KiB seal trigger included metadata | Corrected: trigger uses canonical UTF-8 payload; sealed segment accounting retains metadata for the 32 MiB resident cap. |
| Derived-cache accounting used lengths rather than allocations | Corrected: `reflow_rows_allocated_bytes` accounts vector capacities and is used by cache admission and Runtime aggregate derived-cache enforcement. |

## Evidence-gate audit

| #819 / authority gate | Disposition |
| --- | --- |
| One canonical `TerminalState` / retained history authority | Source review found one primary `HistoryStore`; no second mutable transcript authority was identified. |
| Hard/soft lineage and the retained/active boundary | Small functional regression passes, but resize violates the mandatory bounded/lazy active-progress requirement (**P1 source defect**). |
| Multi-scalar history wire | Normal combining-grapheme Runtime round trip passes. Large multi-scalar rows cannot be continued after sidecar truncation (**P1 source defect**). |
| Per-execution resident metadata and frozen payload seal definition | The prior source defects are addressed. |
| Derived cache allocation accounting | The prior `len()` accounting defect is addressed, but it cannot bound the pre-admission full-stream temporary reflow in `Screen::prepare_resize`. |
| Required property/fuzz/benchmark evidence | Focused tests below pass. The retained benchmark records are comparative and precede the reviewed source SHA; they do not establish final-SHA physical performance acceptance. The #673 contract remains proposed in the evidence record. |
| macOS physical ARM64/client/FFI | Not run by this reviewer. No macOS/client/FFI pass is claimed. |
| Headed user-visible history/reflow | **Unmet.** `docs/evidence/m002-history-819-headed-manual.md` records the Linux cloud-agent attempt as `ENVIRONMENT_UNSUPPORTED`. It launched no `Seyal.app`, observed no Metal pixels, and marks every checklist step unsupported. The earlier AX/scrollbar observation is explicitly partial and does not verify rendered text, reflow, lineage, grapheme integrity, alternate-screen exclusion, eviction, or anchors. No Metal, IME, or headed PASS is inferred or claimed. |
| `make check` / Foundation gates at final SHA | Not run by this reviewer at `2d04f6d`; predecessor evidence is not promoted to a final-SHA pass. |
| Independent closing review | This review is independent, but its P1 findings and unmet evidence gates block closure. |

## Verification performed by this reviewer

```text
git diff --check origin/master...HEAD
passed

cargo test -p seyal-protocol --locked
33 passed (27 unit + 6 input/resize integration); 0 failed; 2 fuzz-smoke tests ignored

cargo test -p seyal-terminal --locked
38 unit + 10 fixture + 18 HistoryStore regression + 18 M001 VT
+ 9 M002 Unicode + 7 M002 VT breadth + 3 salvage + 2 terminfo tests passed
105 passed; 0 failed; 2 fuzz-smoke tests ignored
```

I did not run macOS application, Swift, Metal, IME, client/FFI, physical-ARM64,
benchmark-matrix, fuzz-campaign, or `make check` tests, and make no claim for
them.

## Closing decision and remaining-gap classification

**GO / NO-GO: NO-GO.** Do not use `Closes #819`, `Fixes #819`, or
`Resolves #819` for this candidate.

Remaining blockers include both categories:

1. **Source defects:** the eager whole-history resize/reflow and the
   unrecoverable large multi-scalar-row wire prefix described as P1 above.
2. **Evidence-only gaps:** final-SHA macOS physical performance/client/FFI
   verification, final-SHA Foundation validation, and the required headed
   manual acceptance. Linux `ENVIRONMENT_UNSUPPORTED` is an unavailable
   verification lane, not a pass.
