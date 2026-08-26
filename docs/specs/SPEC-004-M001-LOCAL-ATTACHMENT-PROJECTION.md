# SPEC-004 — M001 local attachment and display-state transport

- **Status:** Accepted for M001 Pass 5 implementation; performance sign-off open — the decisive production-path benchmark completes the full required matrix without error on physical Apple Silicon, but repeated measurement on that same uncontrolled hardware showed 30-50x run-to-run latency variance and one reproduced timeout under host contention (see ADR-001 "Measured evidence"). No latency/throughput number from this session is trustworthy without an isolated benchmark session, and Issue #651's remaining acceptance items (controlled hardware confirmation, final independent review) are still open. Do not treat this line as sign-off.
- **Date:** 2026-08-24
- **Amended:** 2026-08-25, 2026-08-26
- **Issue:** #105 (implementation), #651 (Pass 5.1 final acceptance)
- **Architecture authority:** `ADR-001-LOCAL-DISPLAY-PROJECTION.md`
- **Depends on:** SPEC-001, SPEC-002, SPEC-003

## 1. Purpose

This specification defines Seyal's first permanent local attachment boundary and implements ADR-001 Candidate D.

```text
real process
  → PTY
  → TerminalExecution
  → Seyal VT
  → one canonical TerminalState
  → consume canonical damage once
  → terminal-model snapshot/delta
  → compact versioned binary Unix-domain socket
  → disposable client RenderState
  → renderer
```

Large future image/rich-graphics objects are a separate bulk-object concern. M001 preserves only that seam; it does not implement shared-memory graphics, IOSurface transport, image protocols, remote transport, or a generic bulk framework.

## 2. Non-negotiable invariants

1. `TerminalExecution` is the sole owner of its PTY, primary-child lifecycle and canonical `TerminalState`.
2. Runtime owns execution lookup, attachment/controller authority, scheduling and display delivery.
3. A client owns no PTY, VT parser, canonical grid, scrollback authority or mutable canonical terminal memory.
4. PTY → VT → canonical `TerminalState` never waits for a client acknowledgement/read, renderer, IPC drain, persistence, agent, cloud or licensing path.
5. Attach/reconnect/resync reconstruct from current canonical state. Historical PTY bytes are never replayed into a client VT engine.
6. Display presentation is replaceable state, not an unbounded reliable event log.
7. Canonical damage is consumed once per execution generation. Expensive model extraction/encoding is execution-scoped and shared across viewers where possible.
8. No thread/process/poll loop is created per attachment. Local sockets remain on the existing Runtime/`ExecutionReactor` readiness layer.
9. Rust layout, pointers, parser internals and renderer/GPU objects are never wire format.
10. A stalled, malformed, suspended, killed or disconnected client cannot backpressure terminal progress or another client.

## 3. M001 scope

The protocol supports runtime discovery, version negotiation, execution enumeration, observer/controller attach, detach, input, resize, explicit resync, initial/current-state snapshots, steady-state display deltas, lifecycle/error notification and graceful close.

The following remain out of scope: Metal rendering, glyph shaping/atlases, AppKit IME/keyboard wiring, Blocks/history persistence, remote/network transport, public SDK/plugin protocol, agents/cloud/commercial features, Runtime-crash live-PTY restoration, Linux/Windows local IPC, Kitty/Sixel/iTerm image protocols, IOSurface/shared-memory bulk transport and M002 VT expansion.

## 4. Endpoint and peer security

On macOS Runtime uses the Darwin per-user runtime/temp directory, verifies the directory and socket leaf without following attacker-controlled symlinks, requires Runtime ownership, mode `0700` or stricter for the directory and `0600` or stricter for `control.sock`, rejects unrepresentable `sockaddr_un` paths, and only the active singleton owner may remove a verified stale socket.

Immediately after `accept`, before protocol processing, Runtime verifies the peer effective UID with `getpeereid` or an equivalent kernel credential API. UID mismatch is rejected before attachment state exists.

Same-UID authentication does not grant attachment or mutation authority. Attachment identity remains bound to the authenticated connection.

## 5. Authority and hard limits

Roles are `Observer` and `Controller`. An observer may receive display state, request resync and detach. A controller additionally owns input/resize authority. At most one controller lease exists per `ExecutionId`; controller requests never preempt an existing controller.

M001 hard maxima:

| Resource | Maximum |
|---|---:|
| local control connections | 16 |
| live local attachments | 16 |
| attachments per connection | 1 |
| controllers per execution | 1 |
| execution-list entries | 512 |
| frame payload | 262,144 bytes |
| input bytes per `Input` | 65,536 bytes |
| mandatory outbound control bytes per client | 262,144 bytes |
| visible rows | 256 |
| visible columns | 512 |
| visible cells | 131,072 |

SPEC-003 accepted-but-unwritten input budgets remain authoritative in addition to these limits.

## 6. Connection state machine

```text
Accepted
  → same-UID verified
  → AwaitHello
  → Ready
       ├─ ListExecutions → Ready
       └─ Attach → Attached
                      ├─ Input/Resize       controller only
                      ├─ Resync
                      ├─ DisplaySnapshot    Runtime → client
                      ├─ DisplayDelta       Runtime → client
                      ├─ Lifecycle          Runtime → client
                      └─ Detach → Ready
  → Closing
```

Invalid state transitions return `InvalidState`. Protocol-fatal framing/version/ancillary-data failures close the connection after bounded cleanup. Disconnect revokes a connection's controller lease and attachment state before resource cleanup; the execution continues independently.

## 7. Binary framing

All integers are unsigned little-endian. Opaque IDs are 16 raw bytes. Every frame starts with the existing 24-byte header:

| Offset | Size | Field | Rule |
|---:|---:|---|---|
| 0 | 8 | magic | ASCII `SEYALIPC` |
| 8 | 2 | major | `1` |
| 10 | 2 | minor | `0` |
| 12 | 2 | message_type | section 8 |
| 14 | 2 | flags | zero |
| 16 | 4 | payload_len | `<= 262144` |
| 20 | 4 | reserved | zero |

The complete header is validated before payload allocation. Receive buffering is bounded and reusable. Client→Runtime frames carry no `SCM_RIGHTS`; unexpected, extra, truncated or malformed ancillary descriptors are protocol-fatal and every recovered descriptor is closed.

## 8. Message types

| Type | Direction | Name |
|---:|---|---|
| 1 | C→R | `ClientHello` |
| 2 | R→C | `ServerHello` |
| 3 | C→R | `ListExecutions` |
| 4 | R→C | `ExecutionList` |
| 5 | C→R | `Attach` |
| 6 | R→C | `Attached` |
| 7 | C→R | `Detach` |
| 8 | R→C | `Detached` |
| 9 | C→R | `Input` |
| 10 | C→R | `Resize` |
| 11 | C→R | `Resync` |
| 12 | R→C | `DisplaySnapshot` |
| 13 | R→C | `DisplayDelta` |
| 14 | R→C | `Lifecycle` |
| 15 | R→C | `Error` |
| 16 | either | `Goodbye` |

M001 server capability bits are:

- bit 0: binary display snapshot/delta transport;
- bit 1: observer role.

There is no text-grid projection-FD capability in M001 Candidate D.

## 9. Control payloads

Existing M001 control payloads remain fixed-width/bounded except `Attached`.

`ClientHello` is 8 bytes: `u32 client_capabilities`, `u32 reserved`.

`ServerHello` is 32 bytes: `u128 RuntimeId`, `u32 server_capabilities`, `u32 max_frame_payload`, `u32 max_input_payload`, `u32 reserved`.

`ExecutionList` is `u16 count`, `u16 reserved`, then `count` entries of `u128 ExecutionId`, `u8 lifecycle`, `u8 has_controller`, `u16 attachment_count`; `count <= 512`.

`Attach` is `u128 ExecutionId`, `u8 requested_role`, three reserved zero bytes.

`Attached` is 48 bytes:

```text
u128 ExecutionId
u128 AttachmentId
u8 granted_role
u8[7] reserved
u64 current_generation
```

No descriptor accompanies `Attached`. A current-state `DisplaySnapshot` is queued as part of the attach transaction.

`Detach`, `Detached` and `Resync` each carry one `u128 AttachmentId`.

`Input` is `u128 AttachmentId`, `u32 byte_count`, then exactly `byte_count` bytes; `byte_count <= 65536`.

`Resize` is `u128 AttachmentId`, `u16 rows`, `u16 columns`; geometry must be nonzero and within section 5 maxima.

`Lifecycle` is `u128 ExecutionId`, `u8 lifecycle`, seven reserved zero bytes.

`Error` remains 16 bytes: `u16 error_code`, `u16 offending_message_type`, `u32 detail_code`, `u64 reserved`. It never includes terminal contents, input bytes, environment data, secrets or attacker-controlled text.

`Goodbye` has an empty payload.

## 10. Display model wire contract

### 10.1 Cell record

Display cells are fixed-width 16-byte presentation-neutral records:

```text
u32 Unicode scalar
u32 foreground
u32 background
u16 attributes
u16 reserved = 0
```

Color encoding uses the existing M001 tagged value: default, indexed-8-bit or 24-bit RGB. Attribute bits currently encode bold, underline and inverse; unknown bits are rejected. Invalid Unicode scalars, colors, attributes or nonzero reserved bits are malformed.

### 10.2 Snapshot/delta chunk header

`DisplaySnapshot` and `DisplayDelta` use the same 40-byte payload header followed by complete rows of cell records:

```text
u64 generation
u64 base_generation       # 0 for snapshot; predecessor generation for delta
u16 rows
u16 columns
u16 cursor_row
u16 cursor_col
u8  cursor_visible
u8  alternate_screen
u8  cursor_style          # M001 = 0
u8  reserved0             # 0
u16 first_row
u16 row_count
u16 chunk_index           # zero based
u16 chunk_count           # >= 1
u32 cell_count            # exactly row_count * columns
[cell_count × 16-byte cell records]
```

A chunk must fit in one ordinary frame and contain whole rows. `first_row + row_count <= rows`; all multiplication/addition is overflow checked. `chunk_index < chunk_count`. All chunks of one update repeat identical generation/base/dimensions/cursor/mode values and cover the update's row range exactly once in ascending order.

For `DisplaySnapshot`, `base_generation` is zero and the assembled chunks cover every visible row from `0` through `rows-1`.

For `DisplayDelta`, `base_generation` is the generation of the client state to which the update applies. The encoded rows are exactly the canonical coalesced damage range for that generation. Full canonical damage may therefore produce a delta spanning all rows; it does not change terminal authority.

A client applies a multi-chunk update atomically only after every chunk validates. Partial/malformed updates never partially mutate the committed client RenderState.

### 10.3 Generation continuity

A client applies a delta only when:

```text
client.generation == delta.base_generation
```

After successful atomic apply:

```text
client.generation = delta.generation
```

A snapshot replaces the complete disposable client RenderState and sets its generation unconditionally after validation.

A duplicate/obsolete update at or below the already committed generation may be ignored. A forward delta whose base does not equal the committed generation triggers `Resync`; the client never replays PTY bytes.

### 10.4 Execution-scoped encoding and fanout

For one canonical execution update the target path is:

```text
1 × consume canonical damage
1 × build terminal-model update
1 × binary encode per update representation
N × bounded references/socket deliveries
```

Viewer identity is connection state and is deliberately absent from display frame payloads so otherwise identical display bytes can be shared across viewers without per-view serialization.

## 11. Backpressure, supersession and slow clients

Mandatory control/lifecycle output and replaceable presentation output have separate bounded queue semantics. Mandatory output is serviced before presentation output.

Each client may have at most one presentation batch in flight and one not-yet-started pending batch. Presentation batches are immutable/shareable encoded bytes. Runtime must not retain unbounded generation history.

If a new delta is contiguous with the last presentation generation targeted for that client and a pending slot is available, it may be queued as a delta. If continuity cannot be guaranteed, or a pending presentation batch must be superseded, Runtime replaces the not-yet-started pending work with a current-state snapshot. Subsequent supersession replaces that pending snapshot with a newer snapshot rather than adding history.

A partially written frame is completed or the connection is closed; bytes from two frames are never interleaved. A slow client may be disconnected under bounded resource policy. No case blocks PTY/VT progress.

## 12. Attach, reconnect and resync transactions

First attach validates peer/state/role/`ExecutionId`/capacity, allocates `AttachmentId` privately, reads current canonical visible state without consuming shared canonical damage, encodes a bounded snapshot, admits both `Attached` and the snapshot into nonblocking bounded output, then publishes attachment/controller authority and transitions the connection to `Attached`.

Failure before authority publication leaves no attachment/controller record. Client disappearance after publication is owned by disconnect cleanup and is idempotent.

Explicit `Resync`, reconnect and detected generation gaps use the same current-state snapshot mechanism. No acknowledgement is required before terminal progress continues.

## 13. Resize and final-state ordering

Controller authorization is checked before resize. The PTY/terminal resize transaction remains owned by `TerminalExecution`. A resize that changes dimensions causes canonical full damage and therefore a subsequent display update/snapshot with the new dimensions. There is no projection-memory replacement lifecycle.

After primary-child exit Runtime drains remaining PTY bytes into canonical `TerminalState`, publishes any resulting final display update to attached clients through the same bounded presentation path, then sends lifecycle finalization and releases attachment authority/resources. Delivery cannot extend process-lifecycle deadlines indefinitely.

## 14. Future bulk-object seam

Large immutable graphics/image/media payload bytes are not embedded into normal text/grid deltas merely to reuse this protocol. A future terminal-model update may reference an immutable `AssetId`/placement, while a separately specified local bulk transport may use shared memory, IOSurface or another measured platform-native mechanism. Remote transport may use chunked/compressed/network object delivery.

M001 does not define or authorize descriptor-bearing bulk frames. Any future FD/shared-buffer protocol requires a separate ABI, resource limits, lifetime model and threat review.

## 15. Error codes

M001 defines:

```text
1  UnsupportedVersion
2  UnknownMessage
3  InvalidState
4  InvalidExecution
5  InvalidAttachment
6  StaleIdentity
7  PermissionDenied
8  ControllerBusy
9  CapacityExceeded
10 Backpressure
11 InvalidGeometry
12 DisplayUnavailable
13 MalformedPayload
14 InternalFailure
```

Semantic errors do not mutate canonical state before validation succeeds. Fatal framing/version/ancillary failures close the connection after bounded cleanup.

## 16. Validation requirements

Pass 5 is not complete until all of the following agree with this specification:

- framing round-trip and malformed-input tests for every control/display payload;
- snapshot and delta encode/decode/apply tests, including chunking and atomicity;
- attach/detach/controller/observer/reconnect/resync/stale-ID tests;
- slow-client queue supersession and generation-gap recovery tests;
- resize/alternate-screen/final-PTY-byte ordering tests;
- same-UID, symlink/path, malformed ancillary-data and descriptor-leak tests;
- real fuzz campaigns for binary framing/display decode and reconnect/resync state transitions, in addition to retained deterministic seeds;
- production-equivalent benchmarks using real process → PTY → Seyal VT → canonical state/damage → Candidate-D encode → UDS → client cache.

Performance evidence must cover 1/2/4/8/16 viewers of the same execution, 1/10/50/100 total executions where platform limits permit, 80x24, 120x40, 200x60 and the practical maximum geometry, primary and alternate screen, sparse typing, token streaming, normal command output, sustained high-volume logs for at least two seconds, burst output, scrolling and TUI/full-screen churn.

Record p50/p95/p99 output-to-client-state latency, throughput, CPU, RSS, allocations/reallocations, bytes written/copied, socket calls where instrumentable, queue/coalescing/resync behavior, FD counts and teardown cleanup. Evidence must be labelled `MEASURED`, `ESTIMATED` or `PLATFORM_LIMITED`.

## 17. Acceptance gate

Candidate D is the accepted architecture. Pass 5 may leave draft only when production code no longer uses per-attachment shared-memory text/grid projections, SPEC/ADR/code/tests agree, all required validation is green, production-equivalent Candidate-D evidence meets Seyal latency/resource goals, and independent architecture/security/performance review has no unresolved blocking finding.

Comparator/reference shared-projection code may remain only if isolated from production and clearly labelled non-production evidence. It must not be reachable as a hidden text-grid fallback.