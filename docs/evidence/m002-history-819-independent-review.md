# Independent closing review — #819 HistoryStore / reflow

## Verdict

**NO-GO for a closing merge.** A `Closes #819` / `Fixes #819` / `Resolves
#819` PR is **not allowed** at the reviewed head. The issue must remain open.

This is an independent review of the exact candidate requested, not an
implementer self-approval. A closing merge requires no unresolved P0–P2. This
candidate has unresolved P1 and P2 findings, and its required headed
history/reflow evidence is explicitly unsupported rather than passing.

M002 is **not** claimed complete.

## Revision and scope reviewed

| Item | Value |
| --- | --- |
| Worktree | `/tmp/seyal-oss-work/issue-819-history-store` |
| Branch | `issue/819` |
| Exact SHA reviewed | `941fc22cbc3a5d94b0764b9c420453f87d8f3d1d` |
| Production-code milestone in this branch | `9e691a9775786f24eb59d590dad02b9af4160b21` allocation instrumentation; `16afe73a09f47358a92fd6d92e6e83087666c966` formatting only |
| Comparison | `origin/master` at `9b8408506ee2137c4b0cb20b4b2b2a92549649ab`; merge base `43e443ee93f6e441d3b1658dcc6675beff6bccf2` |
| Diff integrity | `git diff --check origin/master...HEAD` passed; worktree was clean |
| Governing sources read | `AGENTS.md`, `docs/engineering/ISSUE-PROTOCOL.md`, issue #819, ADR-010, SPEC-010, calibration evidence, headed manual evidence, allocation-shard evidence, and the production/test changes |

The worktree advanced from the initially observed `4a1abfe` to `941fc22`
during review. The additional commit changes only five headed-evidence lines:
it records another Linux `ENVIRONMENT_UNSUPPORTED` attempt that did not launch
the app. Its diff was inspected and is included in this review; no production
source or test changed after `16afe73`.

The review treats documentation commits after the production commit as part of
the candidate evidence record. Evidence from older predecessor heads is not
promoted to a final-head acceptance result merely because the code change was
later described as formatting or evidence-only.

## Findings

### P0

None identified in this review.

### P1 — retained multi-scalar history is deliberately unavailable to the UI wire path

`HistoryStore` retains the full UTF-8 payload, but the production
history-to-client path cannot represent it:

* `HistoryUnitRef::legacy_cell` returns `None` whenever a retained unit has
  more than one scalar (`crates/seyal-terminal/src/history.rs`, lines 270–278).
* `TerminalState::primary_history_range` converts that condition to
  `HistoryRangeError::Unrepresentable` (lines 246–318).
* Runtime maps that error to `ErrorCode::DisplayUnavailable` instead of a
  history snapshot (`crates/seyal-runtime/src/runtime/local/history_blocks.rs`,
  lines 71–84).
* The protocol `HistoryCell` contains one scalar rather than a canonical UTF-8
  grapheme payload (`crates/seyal-protocol/src/pass7.rs`, lines 281–295).
* The candidate's own Runtime test
  `history_range_unrepresentable_payload_fails_closed_over_runtime_wire`
  explicitly expects `DisplayUnavailable` after retaining `é`
  (`crates/seyal-runtime/tests/pass7_local_ipc.rs`, lines 231–292).

This is not only missing headed proof. It is an implemented refusal of the
history presentation path for combining and ZWJ/emoji graphemes. It fails the
#819/SPEC-010 requirement that retained wide/grapheme units support derived
reflow without splitting their canonical text, and blocks the manual
“ASCII + CJK + emoji” rendered-history gate. A fail-closed transport response
is safer than corrupt rendering, but it does not satisfy the feature's Done
gate.

### P1 — frozen resident-history cap is not a hard bound for all history-owned metadata

SPEC-010 §6 and the #818 calibration freeze a 32 MiB per-execution resident
history cap that includes history metadata. The implementation accounts only
`HistoryStore` allocations in `HistoryStore::update_resident_bytes`
(`history.rs`, lines 542–550). Two lineage/availability metadata structures
escape a durable hard bound:

1. `Screen::source_breaks: HashMap<LineId, HistoryBreakAfter>` receives an
   entry on every line feed (screen.rs lines 924–937) and is consulted while
   appending history (lines 1331–1389). There is no removal when its source
   line is sealed or evicted. Regular long-lived output without a resize can
   therefore grow this per-line history-lineage map without limit; it is not
   included in `HistoryStore::resident_bytes`.
2. `HistoryStore::evicted_id_ranges` is charged to `resident_bytes`, but is
   never bounded or compacted beyond adjacent numeric `LineId` ranges
   (history.rs lines 589–628). Alternate-screen allocations deliberately
   create gaps, so repeated primary/alternate activity can create a distinct
   evicted range per primary identity. Once this metadata itself exceeds the
   cap, `evict_to_cap` empties available sealed segments and then breaks with
   the cap still exceeded (lines 553–567).

The existing tests verify a small gap is reported `Invalid`; they do not test
bounded metadata after unbounded scrolling, repeated alternate-screen gaps, or
the no-segment-left case. This violates both the frozen cap and the issue's
“eviction obeys frozen byte budgets” acceptance criterion.

### P1 — resize does not reflow a soft-wrapped retained/active boundary as one stream

`Screen::prepare_resize` constructs a temporary `HistoryStore` solely from the
current active screen (`screen.rs`, lines 267–360), computes layout from that
temporary store, and leaves existing `self.history` out of the stream. It then
retains a prefix of only that active reconstruction (lines 369–409).

Consequently, a logical soft-wrap whose earlier source row has already scrolled
into retained history and whose suffix remains active is split at the
history/active boundary during resize. The retained part can be reflowed by
`primary_history_reflow`, while the visible active suffix is reflowed
independently. That is contrary to ADR-010 §8 / SPEC-010 §7–8, which require
adjacent `SoftWrap` records to form one logical reflow stream and require the
active suffix to preserve the same source order/lineage across resize.

The present tests cover an entirely active soft-wrapped source
(`primary_resize_reflows_active_soft_wrapped_source`) and a resize before later
scrolling (`output_and_scroll_after_resize_preserve_source_ids_offsets_lineage_and_style`);
neither covers a logical chain crossing retained history into the active
viewport. No headed evidence closes that gap.

### P2 — segment seal trigger includes metadata contrary to the frozen payload definition

The frozen #818 calibration states that the 16,384-byte trigger is canonical
encoded/uncompressed **payload** and explicitly excludes segment metadata from
that trigger, while including metadata in the 32 MiB resident cap.

`HistoryLine::payload_len` includes `size_of::<SegmentLine>()` and each
`HistoryUnit::encoded_len` includes `size_of::<SegmentUnit>()`
(`history.rs`, lines 77–80 and 142–148). `push_fragment` uses this combined
value to seal the tail (lines 443–465). Thus the seal point is based on
Rust-layout metadata plus payload, not the frozen payload quantity. A
many-short-source-row workload seals materially before 16 KiB of canonical
payload. The unit test reinforces the different contract by asserting total
segment content (metadata plus payload) is at most the target
(`history.rs`, lines 969–999).

This is a measurable non-conformance with SPEC-010 §5.1 and the calibration
record; it needs correction and a target-conformance test before the numeric
budget gate can be accepted.

### P2 — derived-cache accounting does not enforce the actual allocation cap

The cache is created after `reflow_uncached` has already allocated the returned
rows. Admission estimates `rows.len()` for each inner vector, whereas the
allocation can be its larger `capacity()` and also includes the outer
`Vec<ReflowRow>` / row allocations (`history.rs`, lines 663–687; accounting at
lines 367–377). The public reflow API also permits a caller-provided
`max_rows`, so the temporary derived result can exceed the 4 MiB derived budget
before the cache is declined. Runtime aggregate eviction then uses the same
under-counted value.

The cache/rebuild equality test checks semantic equality for a small result,
not cap enforcement under a near-cap retained history. This remains an
unresolved bounded-derived-state gate in SPEC-010 §9.

### P3

No additional P3 finding changes the disposition. The benchmark schema and
focused terminal tests are useful diagnostic evidence, but they cannot
compensate for the P1/P2 failures above.

## Evidence-gate audit

| #819 / authority gate | Independent disposition | Evidence and rationale |
| --- | --- | --- |
| One `TerminalState` / canonical `HistoryStore` authority | Partial | The primary `Screen` owns `HistoryStore`; no separate parser or mutable transcript was found. This does not cure the unavailable presentation path or unbounded metadata. |
| Hard/soft lineage and source identity survive resize | Partial / **P1 open** | Focused tests pass, but the retained-to-active soft-wrap boundary is not one reflow stream, and no headed rendered verification exists. |
| Wide, combining and emoji units remain renderable as intact historical glyphs | **Fail — P1** | Canonical unit storage preserves text, but the Runtime history wire path deliberately returns `DisplayUnavailable` for multi-scalar units. |
| Alternate screen excluded from primary history | Partial automated evidence | Unit tests cover exclusion and sparse IDs. The required headed alternate-screen observation is `ENVIRONMENT_UNSUPPORTED`, not a pass. |
| 32 MiB per-execution / 256 MiB Runtime caps and deterministic eviction | **Fail — P1** | Core segment eviction exists, but `source_breaks` and gap-sensitive evicted-ID metadata are not bounded as retained-history ownership. |
| 16 KiB payload target / 32 KiB tail policy | **Fail — P2** | The seal trigger includes `SegmentLine`/`SegmentUnit` metadata although frozen authority excludes metadata from the payload trigger. |
| Derived indexes are bounded and rebuildable | **Fail — P2** | Rebuild semantic test passes; actual allocation and returned-work bounds are not enforced by the len-based cache accounting. |
| Required VT, regression and fuzz/property coverage | Partial | Current exact-head `cargo test -p seyal-terminal --locked` passed: 36 unit, 10 fixture, 16 HistoryStore regression, 18 M001 VT, 9 Unicode, 7 VT-breadth and 3 salvage tests. The two fuzz-smoke tests are ignored in the normal suite; retained fuzz evidence is not an independent exact-head rerun. The required adversarial cap and retained/active-boundary cases are absent. |
| `make check` / Foundation gates | Not independently established at final SHA | Older evidence records green results, but this review did not treat predecessor reports as a final-head closing result. Focused current tests passed. |
| Physical ARM64 reflow/resource acceptance | **INCONCLUSIVE, not a pass** | The 2026-09-10 allocation-shard interpretation correctly retains `performance_claim=false`. Its 1M×10×48 row is below a *proposed* #673 latency value, but its shape is not the proposed 5 cohorts × 20 warmups × 100 samples. The observed RSS row is not an accepted baseline percentile. #673 and PR #843's contract remain open/proposed. This is honest comparative evidence, not a pass or a failure invented from incomplete policy. |
| Complete instrumented resource matrix | Incomplete | The older 336-case matrix was at the uninstrumented `ac1f6bf` head. The instrumented `9e691a9` record retains bounded shards only; it expressly does not claim a complete instrumented matrix. |
| Headed user-visible history/reflow | **Fail / unmet Done gate** | `m002-history-819-headed-manual.md` labels the 2026-09-09 AX observation partial: no verifiable pixels/rows, no content, reflow, lineage, grapheme, alternate-screen or anchor proof. The 2026-09-10 attempt is `ENVIRONMENT_UNSUPPORTED` on Linux: no macOS app/pixels and every checklist row is unsupported. Neither result is a pass. |
| Independent closing review | **NO-GO** | This review supplies independent review, but blocks closing because P1/P2 findings and mandatory evidence gates remain unresolved. |

## Verification performed by this reviewer

```text
git diff --check origin/master...HEAD
# passed

python3 scripts/check-history-benchmark.py --self-test
python3 scripts/test-history-benchmark.py
# both passed

cargo test -p seyal-terminal --test history_store_regressions --locked
# 16 passed

cargo test -p seyal-terminal --locked
# all executed tests passed; 2 fuzz-smoke tests remained ignored by design
```

These results establish only the behavior represented by the tests. They do
not override the code-path refusals, the missing adversarial resource tests,
the unresolved physical performance policy, or the headed-rendered Done gate.

## Closing decision

**GO / NO-GO: NO-GO.**

The candidate must not use a closing keyword for #819 and must not be merged as
the permanent #819 completion. A future review may reconsider a closing merge
only after the P1/P2 defects are resolved, their adversarial regressions are
added, physical performance is evaluated against an accepted #673 contract
(or honestly remains unresolved and keeps #819 open), and a macOS headed run
proves rendered historical glyphs and the complete reflow checklist. In
particular, `ENVIRONMENT_UNSUPPORTED` and partial AX/scrollbar observation are
evidence of an unavailable verification lane, not feature acceptance.
