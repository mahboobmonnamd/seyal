# M002 #820 current-head closing evidence — 2026-09-16

| Field | Value |
| --- | --- |
| Source head | `2444c78` (`origin/master`) |
| Host | Darwin arm64 / macOS 26.5.2 / Rust 1.98.0 |

Selection/copy/search/paste implementation is already on master. This record is the current-head evidence package for independent review.

## Portable selection

```text
m002_selection 12/12
  copy_preserves_hard_newline_and_not_soft_wrap
  linear_visual_copy_does_not_split_graphemes
  rectangular_copy_uses_visual_row_separators
  search_next_and_prev_cycle_source_matches
  keyboard_copy_mode_yanks_linear_range_without_mouse
  linear_selection_copy_survives_scroll_as_source_anchors
  linear_selection_anchors_stable_across_resize_oscillation
  linear_selection_endpoints_stale_after_eviction
  paste_rejects_empty_and_oversized_payloads
  bracketed_paste_mode_is_canonical_and_wraps_host_bytes
```

Host paste admission (`testHostPasteAdmissionRejectsEmptyAndOversizedUTF8`) passed in `SeyalHostComponentTests` 18/18.

## Headed Flow/Blocks

From the same exclusive-Runtime `scripts/test-macos-ui.sh` run as the keyboard package:

```text
testCopyPasteAndQuitMenusAreWired   passed (16.040s)
```

Cmd-C / Cmd-V on Flow did not crash the host and left composer + Blocks visible. Flow has no click-through terminal cell selection (ADR-009): clicks on the Metal surface request composer focus. Terminal drag-select / copy-mode yank is a Raw/TUI presentation and is the reviewer matrix below.

## Reviewer verification (Raw/TUI)

1. Print wrapped ASCII + CJK + emoji; drag-select across wraps; copy; graphemes must not split or duplicate.
2. Resize before and after a selection; surviving source text remains selected.
3. Hard newline remains; soft wrap does not become a fabricated newline.
4. Bracketed paste on/off in a shell.
5. Search next/previous over thousands of lines after resize.
6. Keyboard copy mode without the mouse.

Documentation impact: N/A for User Guide (no new setting). This file is developer evidence only.
