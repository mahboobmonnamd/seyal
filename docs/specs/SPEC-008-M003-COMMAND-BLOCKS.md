# SPEC-008 — M003 command Blocks and Pane composer

- **Status:** Proposed; implementation not authorized until ADR-009 is accepted
- **Date:** 2026-08-28
- **Architecture:** ADR-009 plus ADR-004/005/006/007/008
- **Depends on:** accepted SPEC-001 through SPEC-007 and completed Pass 7

## 1. Observable contract

The default Pane presentation is a scrollable Flow/Blocks transcript. A Pane
has one composer. When trusted shell integration reports a supported command
entry state, submitting the composer creates one new logical command Block.
The Block displays the submitted command metadata and the canonical terminal
history range produced by that command. Multiple completed and running Blocks
remain ordered in the Pane transcript.

The same Pane and `ExecutionId` remain authoritative throughout. A command Block
is not a terminal session and does not contain a copied terminal surface.

## 2. Command lifecycle

```text
eligible composer
  → command accepted by trusted integration
  → Block(Current, command identity, start LineId)
  → canonical execution/output continues
  → trusted command completion + accepted final drain
  → Block(Completed, end LineId, exit metadata)
```

The Runtime owns lifecycle truth. Missing, malformed, conflicting or stale
metadata causes the client to quarantine that Block projection and preserve
usable direct terminal input; it never changes PTY/VT behavior.

## 3. Composer rules

- exactly one composer state exists per Pane;
- Return/execute submits the complete committed command only when eligibility is
  negotiated and the Pane is not in TUI/raw/secret/interactive-child state;
- Shift-Return inserts a newline;
- failed admission leaves the draft intact and exposes a functional error;
- successful admission creates the next Block and preserves focus in the Pane;
- while a foreground command occupies the shell, the composer is disabled or
  replaced by a real busy-state strip;
- direct terminal input remains available whenever eligibility is unknown.

## 4. TUI rules

Alternate-screen/full-screen entry is a presentation takeover of the same
execution. Block chrome and the Pane composer yield; the Metal surface fills the
Pane viewport and owns application key/scroll semantics. Exit returns to the
normal Block transcript without converting alternate-screen frames into Blocks.

## 5. Ownership and security

Runtime/Workspace owns Block IDs, command boundary records, logical anchors and
completion state. The client receives bounded read-only metadata. No command
text, shell environment, secrets or terminal cells may be copied into an
unbounded client transcript solely to render a Block. Protocol capabilities,
attachment authorization, malformed-record quarantine and bounded queues are
mandatory.

## 6. Acceptance matrix

| Case | Required result |
|---|---|
| `printf hello` submitted | one Current→Completed Block with canonical output range |
| two sequential commands | two ordered Blocks, one Pane composer |
| long-running normal-screen command | one Running Block; composer disabled/busy |
| command failure | same Block completes with real failure metadata |
| multiline command | one Block when trusted integration accepts it |
| Neovim/Claude full-screen mode | composer/chrome yield; same Pane surface takes over |
| TUI exit | normal Block transcript returns; no alternate-frame Block created |
| unsupported shell/integration | direct raw terminal remains usable; no guessed Block |
| detach/reattach | Block IDs and anchors remain stable while metadata exists |

## 7. Required evidence

Implementation requires unit/property/protocol/PTY/VT/conformance tests,
security review, accessibility and headed native UI tests, exact-head benchmark
comparison against Pass 7, controlled screenshots at the supplied reference
size, and manual workflows for sequential commands, failure, multiline input,
focus, IME, Neovim and Claude-style full-screen takeover.
