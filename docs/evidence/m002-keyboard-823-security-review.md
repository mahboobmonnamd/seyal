# M002 #823 key-admission security review

- **Issue:** #823
- **Authority:** `.agents/skills/security-review/SKILL.md`, `docs/engineering/SECURITY.md`, SPEC-006 §21
- **Reviewed production code head:** `9848add7b03c8d402b55164cd27f9008a3df052e`
- **Date (UTC):** 2026-09-10
- **Reviewer path:** isolated OSS worktree `issue/823`; docs/evidence only (no production-code change)
- **Trust model:** Pass 5 same-effective-UID local UDS. Opening `control.sock` is not a sandbox boundary. This review asks whether observer/stale/malformed/unnegotiated key traffic can mutate PTY bytes, leak composition, or unbounded-allocate/panic.

## Verdict

**PASS for the automated/source key-admission path.** No P0, P1, or P2 finding with a realistic same-UID exploit path beyond already-tested fail-closed contracts.

This artifact does **not** close #823 and does **not** authorize a closing PR. Headed native IME/shortcut, physical keyboard/layout, target-TUI, and native-key latency gates remain open.

## Assets

| Asset | Why it matters |
| --- | --- |
| Child PTY byte stream | Unauthenticated or unauthorized keys become process input. |
| Canonical `TerminalState` cursor/keypad/Kitty flags | Encoding authority; must not be client-writable via key frames. |
| Attachment identity / Controller lease | Keys must not be bearer-token reusable across connections or roles. |
| IME/preedit buffer | Composition is secret-bearing; must not leak to PTY, AX, or diagnostics. |
| Host Command shortcuts | Cmd combinations are host actions, never Super/Meta terminal bytes. |
| Bounded input / protocol-reply / outbound queues | Slow or hostile clients must not stall PTY → VT progress. |
| Negotiated `CAP_EXTENDED_TERMINAL_KEY` | Unnegotiated V2 must not be reinterpreted as escape bytes. |

## Actors and trust levels

| Actor | Trust | Expected authority |
| --- | --- | --- |
| Local Controller client (same UID, Hello+Attach Controller) | Same OS user | Submit committed text and typed keys for its attachment only. |
| Local Observer client | Same OS user, read-only | Display only. No input/resize/key admission. |
| Stale / stolen `AttachmentId` / detached connection | Untrusted identity | Rejected; no PTY mutation. |
| Unnegotiated or old/new peer | Capability-gated | V1 remains valid on a new server; V2 is fatal without bilateral cap 1<<7. |
| Native AppKit/IME | First-party UI | Classify intent and composition; never encode terminal modes. |
| Child PTY / TUI | Untrusted byte source | May negotiate Kitty flags 1/2; cannot grant host Command or observer input. |

## Entry points

1. UDS inbound `MessageType::TerminalKey` (17, 24 bytes) and `TerminalKeyV2` (29, 40 bytes) in `Runtime::handle_terminal_key` / `handle_terminal_key_v2` (`crates/seyal-runtime/src/runtime/local/ingress.rs`).
2. `MessageType::Input` committed UTF-8 (IME commit / printable text).
3. Decoder `TerminalKeyV2::decode` / `TerminalKey::decode` (`crates/seyal-protocol/src/pass7.rs`).
4. FFI `seyal_bridge_submit_key` / `seyal_bridge_submit_key_v2` / `seyal_bridge_submit_utf8` (`crates/seyal-client/src/ffi/input.rs`).
5. Native `InteractiveMetalSurfaceView.keyDown` / `keyUp` / `NSTextInputClient` (`macos/Seyal/Sources/TerminalInputSurface.swift`).
6. Kitty query/set/push/pop into canonical `TerminalState` (`crates/seyal-terminal/src/terminal.rs`), which Runtime reads only after authorization.

## Review of required surfaces

### Runtime encoder

Encoding happens once at Runtime admission from canonical `entry.execution.terminal().modes()`, not from Swift-projected modes. V2 encoding is a `Result`; unsupported combinations return before `try_submit`. Successful no-byte events (flag-2-absent release; Enter/Tab/Backspace release) do not enqueue. Output length is bounded by construction (CSI-u / modifier CSI well under the 64-byte contract). Kitty flags 4/8/16 are masked (`KEYBOARD_FLAGS_MASK = 0b11`). Per-screen stacks are fixed `[u8; 16]` with oldest-evict on push.

V1 type-17 remains valid on a V2-capable server and is still mode-sensitive at admission. Wire fields never grant modes or host authority.

### Observer / stale / detached rejection

`AttachmentRegistry::authorize_mutation` requires the connection that created the attachment and `Role::Controller`. Observer yields `PermissionDenied`. Missing identity, detach, and stolen IDs (`WrongConnection`) map to `StaleIdentity` in the key handlers — fail-closed without disclosing foreign live IDs. Structural decode precedes capability/monotonic checks; those precede authorization; authorization precedes mode lookup and queue mutation, matching SPEC-006 §21.5.

Action IDs are connection-scoped and advanced before authorization so a rejected observer/stale V2 still consumes the ID on that connection only. Duplicate/non-monotonic IDs are connection-fatal. Tests cover malformed V2, unnegotiated V2, duplicate ID, observer Input denial, and stolen-ID Input/resize/resync. V2 reuses the same authorize path.

Mandatory error enqueue failure closes the connection (`send_mandatory_frame` → `close_local_connection`).

### IME / preedit non-leak

`setMarkedText` mutates a bounded `CompositionDocument` (≤ 64 KiB UTF-8) and never calls `submitCommittedText`. `insertText` clears then submits exactly one Input transaction. `unmarkText` commits remaining marked text (AppKit unmark semantics). Focus loss / window teardown / bridge failure call `cancelComposition`, which clears without submit. While marked text exists, `inputContext.handleEvent` has first opportunity; `doCommand(by:)` is a no-op so IME-consumed navigation/Enter/Escape do not fall through as semantic keys.

AX `accessibilityValue` is recovery metadata (`process` / `connection` / identities / alternate-screen), not marked text. Pass 9 input-accessibility qualification asserts marked text is absent from AX value. Input bytes are excluded from Pass 7 benchmark marks and Error frames.

Headed physical IME/dead-key remains an open evidence gate, not a source finding that preedit is submitted.

### Cmd shortcut non-leak

`keyDown` returns to `super.keyDown` when `.command` is set, before V2/semantic/text classification. V2 modifiers accept only Shift/Alt/Control (`bits & !0b111` is malformed → connection-fatal). There is no Command→Super alias. Component self-tests cover Option-as-Alt and capability-loss dropping held V2 releases; headed Cmd shortcut XCUI is still required.

### Capability negotiation

Server Hello always advertises `CAP_EXTENDED_TERMINAL_KEY` (1<<7). Client Hello allowlists that bit (plus blocks/grapheme/metadata). Runtime admits V2 only when the **client** also advertised the bit. Unnegotiated structurally-valid V2 is fatal after one bounded error. Client `extended_terminal_key_supported` disables V2 against old servers; native held-key release is discarded on capability loss rather than sent unnegotiated. Unknown Hello capability bits fail closed.

### Bounded queues

| Queue | Bound | Overflow behavior |
| --- | --- | --- |
| Client outbound control | `MAX_OUTBOUND_WIRE_BYTES` = 262144 | Admission `ClientBackpressure`; no unbounded pending-action map. |
| Runtime input reservation | global + per-execution byte caps; `SyncSender` | `InputBackpressure` / `ControlQueueFull`; reservation rolls back on drop. |
| Runtime outbound mandatory | `MAX_OUTBOUND_QUEUE_BYTES` = 262144 | Close connection. |
| `ProtocolReply` at TerminalState | 16 × 64 bytes | Drop + `deferred_sequences`. |
| Execution pending replies | 16 | Drop + `dropped_protocol_replies`. |
| Kitty flag stacks | 16 per screen | Evict oldest; huge pop is `min(count, len)`. |
| Native held keys | 256 keyCodes | New press not tracked (see P3). |
| Composition | 65536 UTF-8 | Cancel + visible failure. |
| V2 frame | exact 40 bytes; kinds/events/reserved/version validated | Malformed is connection-fatal. |

One slow client cannot enlarge these limits. Presentation remains coalescible on the existing display path; key admission does not add a second PTY queue.

## Findings

### P0

None.

### P1

None.

### P2

None.

### P3 (residual / process)

1. **Headed IME, physical layout, and Cmd-shortcut non-leak** are implemented fail-closed in source and component tests but are not substituted by this review. Manual/XCUI gates stay open.
2. **Dedicated V2 observer/stale/detach IPC fixtures** are thinner than type-17/Input. Rejection is the same `authorize_mutation` function; residual is coverage, not a second authorize path.
3. **Held-key map overflow** (`count < 256 else { return }`) drops the new press without the visible native failure layer SPEC-006 §21.3 asks for. It does not submit bytes; availability/UX residual only.
4. **`action_id` exhaustion:** native wrapping add stops at 0 (`guard actionID != 0`); Runtime rejects wrap/replay as non-monotonic/zero. Spec prefers explicit reconnect before exhaustion; stuck-at-zero is fail-closed availability, not injection.
5. **FFI `seyal_bridge_submit_key_v2`** validates kind/event/modifier bits but not the full value/`shifted_ascii`/`action_id` matrix before `encode()`. Invalid combinations are Runtime-fatal (same-UID self-DoS), not PTY injection. Same-UID already owns the process.
6. **Fuzz corpus** for `pass7-protocol-decode` retains a 24-byte V1 TerminalKey seed and truncated-key seed; there is no dedicated 40-byte TerminalKeyV2 seed. The harness still calls `TerminalKeyV2::decode` on raw slices. A 45s campaign is `ci-smoke` duration, not Pass 10 §6.9 nightly (600s).
7. **Key-latency:** existing Pass 7 harness is macOS-only. This Linux host recorded `PLATFORM_LIMITED`. No scanout/key-to-photon claim.

No tests were weakened. No production instrumentation was added.

## Residual risk

Same-UID local clients remain inside one OS-user trust domain. A hostile same-UID Controller can type into a PTY it already controls; that is the product. The residual that still blocks #823 Done is **evidence**, not an unfixed admission bypass: headed IME/Cmd, physical matrix, target TUI negotiation, and native-key→PTY latency on Apple Silicon.

Closing PR allowed: **no** (headed/manual gates assumed still open).
