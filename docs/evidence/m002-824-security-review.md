# M002 #824 OSC / input / mouse / query security review

- **Issue:** #824
- **Authority:** `.agents/skills/security-review/SKILL.md`, `docs/engineering/SECURITY.md`
- **Reviewed production code:** prior `issue/824` head `f0e8c01` (the original tests/docs and XCUI evidence). Subsequent native IME and Block hit-testing fixes are not covered by this historical review; their tests and independent source-review limits are recorded in `m002-824-headed-manual.md`.
- **Date (UTC):** 2026-09-16
- **Trust model:** Pass 5 same-effective-UID local UDS. Child PTY bytes are untrusted. OSC payloads are untrusted presentation. Mouse/keyboard modes cannot grant host Command authority.

## Verdict

**PASS for the automated OSC / query / paste / mouse-report surfaces exercised by
`m002_workload_matrix::hostile_osc_query_paste_and_mouse_stay_bounded_and_non_executing`
and the retained #821/#822/#823 tests.** No P0/P1 finding that requires a
compatibility fix in this validation PR.

This artifact does **not** close #824. Headed IME/clipboard/host-shortcut
evidence remains on the headed ledger.

## Assets

| Asset | Why it matters |
| --- | --- |
| Canonical `TerminalState` | Sole terminal authority; PTY bytes must not panic, corrupt, or unbounded-allocate. |
| OSC 0/2/7/8 presentation queue | Untrusted title/CWD/hyperlink; must not execute or become filesystem/network authority. |
| Deferred OSC 52 | Clipboard OSC remains unsupported; parser continuity only. |
| Protocol-reply queue (DA/DSR/DECRQM/kitty) | Replies are opaque PTY bytes, bounded (`MAX_PROTOCOL_REPLIES=16`, 64 B each). |
| Mouse reports | Encoding only; Shift host-override wins; reports are not host Command. |
| Paste encoder | Bounded (`MAX_PASTE_BYTES`); NUL/nested bracket markers stripped. |

## Entry points

1. `TerminalState::feed` OSC dispatch (`terminal.rs` `fn osc`) — truncated OSC is deferred and not enqueued.
2. `parse_osc_presentation` — codes 0/2/7/8 only; payloads truncated to `MAX_PRESENTATION_PAYLOAD_BYTES` (1024). Parser OSC buffer is `MAX_OSC_BYTES` (4096).
3. Protocol replies: DA1, DSR CPR, DECRQM for implemented modes, kitty flags query.
4. `encode_mouse_report` / `mouse_takes_host_override` — application reports vs host selection.
5. `encode_paste` / `sanitize_paste` — host clipboard → PTY bytes.

## Review

### OSC does not execute

OSC 52 (`\x1b]52;c;…`) produces no `HostPresentationEvent` and leaves printable
state intact. Oversized OSC 2 is truncated at 1024 B when it fits the parser
buffer; parser-truncated OSC (>4096) is deferred with no event.
`javascript:` hyperlink URIs remain opaque presentation bytes.

CWD (OSC 7) is a presentation event only. It is not a `chdir`, file open, or
approval.

The presentation queue is capped at `MAX_HOST_PRESENTATION_EVENTS` (16);
overflow increments `deferred_sequences`.

### Queries do not grant host authority

DA/DSR/DECRQM replies enqueue on the protocol-reply seam and never mutate
host policy. Unknown queries increment unknown/deferred counters. The reply
queue drops when full rather than growing.

Kitty flag push/pop is screen-local, masked to bits 1|2, stack capacity 16.

### Mouse / input

Reporting Off yields no application report. Shift forces host override.
Mouse reports cannot set modes, spawn processes, or bypass observer/controller
authorization (that remains Runtime admission; see #823 security review).

Paste rejects empty and oversized payloads. Bracketed wrap is added only when
mode 2004 is set.

### Residual risk

- Headed clipboard/IME/OSC-title chrome policy is host-side; this review does
  not claim AppKit never displays an untrusted title.
- Live SSH is an ordinary PTY child; remote hosts are outside the local UDS
  trust domain and are not a Seyal remote-attach product (M007).
- Graphics/image protocols remain unadvertised and unparsed as supported.

## Tests added

`crates/seyal-terminal/tests/m002_workload_matrix.rs::hostile_osc_query_paste_and_mouse_stay_bounded_and_non_executing`
