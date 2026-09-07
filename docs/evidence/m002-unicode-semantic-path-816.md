# M002.2 (#816) Unicode semantic-path measurement notes

- **Issue:** #816
- **Head:** recorded at PR open time
- **Authority:** SPEC-011 / ADR-011; release ceilings remain #673

## Method

Local `cargo test -p seyal-terminal` plus focused Unicode fixtures. Semantic-path
throughput is exercised by the overflow combining storm and chunk-equivalence
tests; this note does not claim key-to-photon latency.

## Observations

- ASCII + combining chunk-vs-oneshot feeds match.
- Width-2 CJK occupies lead+continuation with no orphan halves under overwrite.
- Mode 2027 defaults to set and answers DECRQM.
- DECAWM-reset fixtures from `tests/fixtures/m002-unicode/` pass.
- Active grapheme payload above 8,192 bytes increments
  `grapheme_payload_overflow_count` and recovers on the next grapheme boundary.
- No CoreText/AppKit dependency enters `seyal-terminal`.

## Follow-on

#817 owns grapheme display v2 / Metal shaping measurements. #673 owns versioned
release ceilings for 1/10/50/100 execution scaling.
