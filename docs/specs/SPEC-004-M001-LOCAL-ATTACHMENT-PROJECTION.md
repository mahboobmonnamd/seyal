# SPEC-004 — M001 local attachment protocol and display projection

- **Status:** Proposed for M001 Pass 5
- **Date:** 2026-08-24
- **Issue:** #103
- **Architecture authority:** `ADR-001-LOCAL-DISPLAY-PROJECTION.md`
- **Depends on:** SPEC-001, SPEC-002, SPEC-003

## 1. Purpose

This specification defines the first permanent local client attachment boundary for M001:

```text
Seyal.app / local client
    │
    │ compact versioned binary Unix-domain control/input
    ▼
Seyal Runtime
    │
    ├─ Runtime registry / attachment authority
    └─ TerminalExecution
         ├─ PTY + child lifecycle
         └─ one canonical TerminalState
                    │
                    ▼
          Runtime-owned derived projection
                    │
                    ▼
        read-only client shared-memory mapping
```

ADR-001 selects the architecture. This specification fixes the observable M001 wire, shared-memory, lifecycle, security, recovery, validation and benchmark contracts required before production implementation.

The projection is a rebuildable presentation cache. It is never terminal authority.

## 2. Non-negotiable invariants

1. Each `TerminalExecution` remains the sole physical owner of its PTY, primary-child lifecycle and canonical `TerminalState`.
2. The Runtime owns execution lookup, attachment authority, controller leasing, projection lifecycle and the single projection writer.
3. A local client owns no PTY, parser, VT state, terminal grid, scrollback authority or mutable canonical terminal memory.
4. The production progress path remains:

   ```text
   PTY → VT → TerminalState → damage
   ```

   and never waits for a client acknowledgement, client read, shared-memory reader, renderer, persistence, Blocks, agents, cloud, telemetry or licensing.
5. Shared memory contains explicitly encoded derived values only. Rust pointers, `Vec` layout, enum layout, parser state, `TerminalState` layout and canonical grid memory are forbidden.
6. A killed, stalled, suspended, malicious or disconnected local client cannot backpressure PTY → VT progress.
7. Display generations are lossy/coalescible. Clients may skip obsolete generations and recover from the newest complete snapshot.
8. First attach, reconnect and resync use current canonical state. Historical PTY bytes are never replayed to rebuild a GUI terminal state.

## 3. Scope

### 3.1 M001 operations

The local protocol supports:

- Runtime connect/discovery;
- protocol hello/version establishment;
- bounded execution enumeration for M001 reconnect selection;
- attach by `ExecutionId` as observer or controller;
- detach;
- input ingress seam;
- resize request seam;
- explicit resync;
- projection descriptor delivery/replacement;
- one-way generation wake;
- lifecycle/error notification;
- graceful connection close.

Pass 7 owns final AppKit keyboard/resize wiring. Pass 5 only establishes the production protocol seam and test client/harness.

### 3.2 Explicit non-goals

This specification does not define:

- Metal rendering, shaping or glyph atlases;
- final AppKit keyboard, resize or IME behavior;
- Blocks/history persistence/scrollback projection;
- tabs, splits, workspaces UI, inspectors or notifications;
- remote/network transport;
- public SDK/plugin/application protocol;
- agent/cloud/Teams/Enterprise behavior;
- Runtime-crash live-PTY recovery;
- Linux/Windows local-IPC implementation;
- M002 VT expansion.

## 4. Runtime discovery and endpoint security

### 4.1 Per-user directory

On macOS, the Runtime resolves the operating-system-provided per-user temporary/runtime directory rather than accepting an arbitrary environment path as authority. The production implementation uses the Darwin per-user directory returned by `_CS_DARWIN_USER_TEMP_DIR` (or an equivalent OS API with the same ownership/isolation semantics) and creates one `seyal-runtime` child directory.

Before creating or trusting endpoint files, the Runtime must verify with no-following filesystem operations that the directory:

- is a directory, not a symlink;
- is owned by the Runtime effective UID;
- is not group/world writable;
- is opened/created with mode `0700` or stricter;
- is reached without trusting an attacker-substitutable leaf path.

The control socket is `control.sock` inside that verified directory and is mode `0600` or stricter. The implementation must account for Darwin `sockaddr_un.sun_path` limits and fail explicitly if the resolved path cannot be represented; it must not truncate a path.

The existing Pass-4 singleton lock remains lifecycle authority. Pass 5 may colocate the singleton metadata in the verified runtime directory, but discovery does not weaken the singleton contract.

### 4.2 Stale socket handling

Only the active Runtime holding the singleton authority may replace a stale control socket. Before unlinking a socket path it must verify the parent directory and leaf ownership/type without following symlinks. An active connectable endpoint is never unlinked as “stale”. Expected `ENOENT`/already-gone races are cleanup success; unexpected type/ownership is a hard security failure.

No process trusts PID text, a pathname alone, or attacker-supplied metadata as proof of Runtime identity.

### 4.3 Peer authentication

Immediately after `accept`, before any client frame is acted upon, the Runtime obtains peer credentials from the connected Unix-domain socket using the Darwin peer-credential facility (`getpeereid` or an equivalent kernel credential API). The peer effective UID must equal the Runtime effective UID.

A UID mismatch is rejected and the connection is closed without attachment creation.

M001 explicitly treats malicious code already executing as the same local UID as inside the same OS user trust domain; this protocol does not make a false sandbox claim against such a process. Controller/observer authority below still ensures that merely opening a same-user socket grants no mutation authority and that observer clients cannot send input/resize.

## 5. Connection and authority model

### 5.1 Limits

M001 hard maxima:

| Resource | Hard maximum |
|---|---:|
| concurrent local control connections | 16 |
| live local attachments | 16 |
| attachments per control connection | 1 |
| controllers per `ExecutionId` | 1 |
| executions in enumeration response | 512 |
| frame payload | 262,144 bytes |
| input bytes in one `Input` frame | 65,536 bytes |
| mandatory queued outbound control bytes per client | 262,144 bytes |
| projection rows | 256 |
| projection columns | 512 |
| projection cells | 131,072 |
| projection region | 8 MiB |
| aggregate live projection mappings owned by Runtime | 128 MiB |

Implementation configuration may choose smaller operational limits. It may not exceed these M001 wire/ABI maxima without a specification change.

Existing SPEC-003 per-execution and Runtime-wide accepted-but-unwritten input budgets remain authoritative and apply in addition to the frame limit.

### 5.2 Roles

Roles are:

```text
Observer
Controller
```

Opening a socket grants neither attachment nor controller authority.

A same-user authenticated client explicitly requests a role in `Attach`:

- `Observer` may read a projection, request resync, observe lifecycle and detach.
- `Controller` has observer rights plus input and resize authority.

The Runtime grants at most one controller lease for an `ExecutionId`. There is no implicit first-frame input authority, controller preemption or observer promotion. A controller request while another controller is attached returns `ControllerBusy` and creates no attachment. Disconnect/detach revokes that connection's controller lease before resources are reclaimed.

An observer sending `Input` or `Resize` receives `PermissionDenied`; the canonical execution is unchanged.

### 5.3 Connection state machine

```text
Accepted
  → peer UID verified
  → AwaitHello
  → Ready
       ├─ ListExecutions → Ready
       └─ Attach → Attached
                      ├─ Input/Resize       (Controller only)
                      ├─ Resync             (both roles)
                      ├─ GenerationWake     (Runtime → client only)
                      ├─ ProjectionReplaced (Runtime → client only)
                      ├─ Lifecycle          (Runtime → client only)
                      └─ Detach → Ready
  → Closing
```

`Attach` while already Attached, `Detach` before Attach, client-sent Runtime-only messages and other invalid transitions produce `InvalidState`. A protocol-fatal framing/version/descriptor error closes the connection. Nonfatal semantic errors leave the connection in its previous valid state.

If a client disconnects mid-frame, the partial frame is discarded and any attachment/controller/projection resources are reclaimed. The execution continues unless its own lifecycle independently ends.

## 6. Binary wire format

### 6.1 Encoding

All integer fields are unsigned little-endian unless explicitly stated. IDs are 16 raw bytes carrying the opaque 128-bit value; clients must not derive semantic meaning from their bits.

Rust memory layout is never the wire format.

### 6.2 Frame header

Every frame starts with exactly 24 bytes:

| Offset | Size | Field | Rule |
|---:|---:|---|---|
| 0 | 8 | magic | ASCII `SEYALIPC` |
| 8 | 2 | major | M001 = `1` |
| 10 | 2 | minor | M001 = `0` |
| 12 | 2 | message_type | section 6.4 |
| 14 | 2 | flags | must be zero in 1.0 unless specified |
| 16 | 4 | payload_len | `<= 262144` |
| 20 | 4 | reserved | must be zero |

The decoder validates the full header before allocating or waiting for a payload. An oversized `payload_len`, invalid magic, nonzero reserved field or integer-overflowing frame size is protocol-fatal.

The implementation uses a bounded reusable receive buffer; it must not allocate directly from an untrusted length.

### 6.3 Version behavior

M001 supports exactly major 1, minor 0.

- Unknown major: if the header is otherwise safe to parse, Runtime may send `UnsupportedVersion`, then closes.
- Future higher minor under a known major is rejected in M001 until compatibility is explicitly specified; do not silently interpret newer fields.
- Unknown message type under a supported version is skipped only after its bounded payload is fully framed, then returns nonfatal `UnknownMessage` while preserving framing continuity.

### 6.4 Message types

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
| 12 | R→C | `GenerationWake` |
| 13 | R→C | `ProjectionReplaced` |
| 14 | R→C | `Lifecycle` |
| 15 | R→C | `Error` |
| 16 | either | `Goodbye` |

### 6.5 Payload contracts

`ClientHello` payload, 8 bytes:

```text
u32 client_capabilities   # M001 must be 0
u32 reserved              # 0
```

`ServerHello` payload, 32 bytes:

```text
u128 RuntimeId
u32 server_capabilities   # M001 bit 0 = projection-fd, bit 1 = observer
u32 max_frame_payload
u32 max_input_payload
u32 reserved
```

`ListExecutions` has an empty payload.

`ExecutionList`:

```text
u16 count
u16 reserved
repeat count times:
  u128 ExecutionId
  u8 lifecycle
  u8 has_controller
  u16 attachment_count
```

`count <= 512`; exact payload length must equal `4 + count * 20`.

`Attach` payload, 20 bytes:

```text
u128 ExecutionId
u8 requested_role         # 0 Observer, 1 Controller
u8[3] reserved            # all 0
```

`Attached` payload, 80 bytes, and exactly one `SCM_RIGHTS` descriptor:

```text
u128 ExecutionId
u128 AttachmentId
u128 ProjectionId
u8 granted_role
u8[7] reserved0
u64 committed_generation
u64 region_bytes
u16 capacity_rows
u16 capacity_cols
u32 reserved1
```

The descriptor is a read-only descriptor for the projection object. Missing, extra or wrong-context descriptors on Runtime→client descriptor-bearing frames are protocol-fatal to that client-side attachment transaction. No client→Runtime M001 frame carries an `SCM_RIGHTS` descriptor; any inbound descriptor, extra descriptor set or truncated/malformed ancillary data is protocol-fatal to that connection, and every descriptor delivered to the Runtime on the rejected path must be closed during bounded cleanup.

`Detach` payload is one `u128 AttachmentId`.

`Detached` payload is one `u128 AttachmentId`.

`Input`:

```text
u128 AttachmentId
u32 byte_count
u8[byte_count] bytes
```

`byte_count <= 65536` and exact payload length must equal `20 + byte_count`. Input is accepted only after controller authority and SPEC-003 input-budget admission succeed. Queue/budget failure returns `Backpressure`; rejected bytes are not partially accepted.

`Resize` payload, 20 bytes:

```text
u128 AttachmentId
u16 rows
u16 columns
```

Rows/columns must be nonzero and within projection hard maxima. Controller authority is validated before the SPEC-002/003 resize transaction begins.

`Resync` payload is one `u128 AttachmentId`.

`GenerationWake` payload, 40 bytes:

```text
u128 AttachmentId
u128 ProjectionId
u64 committed_generation
```

Wakes are advisory and coalescible. A wake is never an acknowledgement requirement.

`ProjectionReplaced` payload has the same shape as `Attached` excluding role, plus exactly one new read-only `SCM_RIGHTS` descriptor:

```text
u128 ExecutionId
u128 AttachmentId
u128 ProjectionId
u64 committed_generation
u64 region_bytes
u16 capacity_rows
u16 capacity_cols
u32 reserved
```

`Lifecycle` payload, 24 bytes:

```text
u128 ExecutionId
u8 lifecycle
u8[7] reserved
```

`Error` payload, 16 bytes:

```text
u16 error_code
u16 offending_message_type
u32 detail_code
u64 reserved
```

No error payload includes terminal contents, input bytes, environment values, secrets or arbitrary attacker-controlled text.

`Goodbye` has an empty payload.

### 6.6 Error codes

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
12 ProjectionUnavailable
13 MalformedPayload
14 InternalFailure
```

Malformed framing/descriptor transfer closes the connection after bounded cleanup. Semantic errors never mutate canonical state before validation succeeds.

## 7. Socket I/O and slow-client behavior

All accepted client sockets are nonblocking and integrated into the Runtime scheduling layer; Pass 5 does not introduce a thread/process per attachment.

Terminal progress never performs a blocking socket send.

Mandatory outbound control frames use a bounded per-client queue of at most 262,144 bytes. Exceeding the queue disconnects the slow client and reclaims its attachment resources.

`GenerationWake` is special: there may be at most one pending wake per attachment. If a newer generation exists before the pending wake is written, the pending generation number is replaced with the newest generation. If the socket cannot currently accept the wake, no history queue is created. The projection itself remains the recovery source.

A suspended client therefore may observe:

```text
N → N+100
```

without receiving N+1…N+99. This is correct behavior.

## 8. Shared-memory object lifecycle

### 8.1 Creation and access

Each live attachment owns at most one current projection region in Runtime resources.

For macOS M001 the Runtime:

1. creates a collision-resistant named POSIX shared-memory object with `O_CREAT | O_EXCL | O_RDWR`, mode `0600`, then immediately enforces `FD_CLOEXEC` with `fcntl` before the descriptor can escape the creation function;
2. sizes it after overflow-checked layout computation;
3. maps the Runtime descriptor writable;
4. opens the same object independently `O_RDONLY` for client transfer and likewise enforces `FD_CLOEXEC` before retaining/transferring it;
5. immediately `shm_unlink`s the name after both descriptors are acquired;
6. sends only the read-only descriptor via `SCM_RIGHTS`;
7. never sends or logs the shared-memory name.

The explicit `fcntl(FD_CLOEXEC)` step is normative for Darwin M001 because macOS `shm_open` does not accept `O_CLOEXEC` as a portable creation flag. A descriptor received over `SCM_RIGHTS` must also have close-on-exec enforced at the receiving boundary before it is retained.

The client maps the received descriptor `PROT_READ` only. A writable client mapping is a protocol/security implementation failure.

The Runtime never maps or exports canonical `TerminalState` memory.

### 8.2 Projection IDs and stale handles

Every region has a fresh opaque 128-bit `ProjectionId`, independent of `ExecutionId` and `AttachmentId`. A replacement region always gets a new `ProjectionId`.

All generation wakes/resync state are checked against the live `(ExecutionId, AttachmentId, ProjectionId)` tuple. A stale attachment/projection identifier cannot affect a later attachment even if an old read-only fd remains mapped in a killed/suspended client.

### 8.3 Capacity and replacement

A projection has explicit `capacity_rows` and `capacity_cols`, each within M001 hard maxima. Initial capacity may include implementation-defined bounded headroom to avoid needless remaps.

If a successful canonical resize exceeds current projection capacity:

1. Runtime creates a replacement region;
2. writes a full current snapshot to it;
3. commits that generation;
4. sends `ProjectionReplaced` with the new read-only descriptor;
5. drops Runtime ownership of the old region after descriptor transfer is complete.

The old client mapping remains read-only and stale. The client validates and switches to the new `ProjectionId`; no acknowledgement is required for terminal progress or Runtime teardown.

Projection creation/replacement failure does not roll back an already successful PTY/`TerminalState` resize. It marks that attachment projection unavailable, sends/queues `ProjectionUnavailable` where possible, and allows later `Resync` to allocate a fresh projection. Canonical terminal authority remains correct.

### 8.4 Cleanup

Detach, graceful client exit, socket loss, client `SIGKILL`, Runtime shutdown or failed attach rollback must release Runtime-owned mappings/descriptors and attachment registry records idempotently. The terminal execution survives client loss unless its own lifecycle ends.

## 9. Projection ABI v1.0

### 9.1 General rules

- All multibyte encoded values are little-endian.
- All offsets are unsigned byte offsets from the start of the containing region or slot as stated.
- Every offset + length computation is checked for integer overflow before use.
- Reserved fields must be zero when written and are ignored only after bounds/version validation by a 1.0 reader.
- Header/slot publication atomics are 8-byte aligned.
- The client validates region size from `fstat` before dereferencing any encoded offset.
- Region size must be `<= 8 MiB`.
- Slot count is exactly 2 for ABI 1.0.

### 9.2 Region header — 128 bytes

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | ASCII `SEYALPRJ` |
| 8 | 2 | ABI major = 1 |
| 10 | 2 | ABI minor = 0 |
| 12 | 4 | header_bytes = 128 |
| 16 | 8 | region_bytes |
| 24 | 16 | `ExecutionId` |
| 40 | 16 | `AttachmentId` |
| 56 | 16 | `ProjectionId` |
| 72 | 4 | slot_count = 2 |
| 76 | 4 | slot_header_bytes = 64 |
| 80 | 8 | slot_stride |
| 88 | 8 | slot0_offset |
| 96 | 8 | atomic publication word |
| 104 | 2 | capacity_rows |
| 106 | 2 | capacity_cols |
| 108 | 2 | cell_bytes = 16 |
| 110 | 2 | damage_bytes = 8 |
| 112 | 16 | reserved = 0 |

`slot0_offset` is 64-byte aligned. Slot 1 begins at `slot0_offset + slot_stride`. Both slots must fit fully in `region_bytes`.

The publication word is one aligned atomic `u64`:

```text
bits 0      committed slot index (0 or 1)
bits 1..63 committed generation (0 .. 2^63-1)
```

Generation zero means no readable snapshot. Production M001 must publish the first snapshot before delivering `Attached`/`ProjectionReplaced`, so a valid attached client sees generation > 0.

Writer stores the publication word with release semantics only after the complete slot has been finalized. Reader loads it with acquire semantics.

### 9.3 Slot header — 64 bytes

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | atomic slot sequence |
| 8 | 8 | generation |
| 16 | 4 | payload_bytes |
| 20 | 2 | rows |
| 22 | 2 | columns |
| 24 | 2 | cursor_row |
| 26 | 2 | cursor_col |
| 28 | 1 | cursor_visible (0/1) |
| 29 | 1 | cursor_style |
| 30 | 2 | mode_flags |
| 32 | 4 | cell_count |
| 36 | 2 | damage_count |
| 38 | 1 | snapshot_flags |
| 39 | 1 | reserved0 |
| 40 | 4 | cells_offset (from slot start) |
| 44 | 4 | damages_offset (from slot start) |
| 48 | 8 | source_damage_generation |
| 56 | 8 | reserved1 = 0 |

M001 `cursor_style` value 0 means the default block-style seam; no additional cursor-shape semantics are claimed by Pass 5.

`mode_flags`:

```text
bit 0  alternate screen active
bit 1  cursor visible
bits 2..15 reserved 0
```

`snapshot_flags`:

```text
bit 0  complete visible snapshot (must be 1 in ABI 1.0)
bits 1..7 reserved 0
```

For ABI 1.0:

```text
cell_count == rows * columns
damage_count <= rows
rows <= capacity_rows <= 256
columns <= capacity_cols <= 512
cursor row/col are in bounds when rows/columns > 0
```

### 9.4 Cell encoding — 16 bytes

Cells are row-major, `row * columns + column`.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | Unicode scalar value |
| 4 | 4 | foreground color |
| 8 | 4 | background color |
| 12 | 2 | attribute flags |
| 14 | 2 | reserved = 0 |

The scalar must be a valid Unicode scalar value. ABI 1.0 represents the current M001 scalar-per-cell terminal contract only; it does not claim grapheme/width correctness beyond SPEC-001.

Color word:

```text
bits 31..30 kind
  00 Default     (remaining bits must be 0)
  01 Indexed     (bits 7..0 index, other payload bits 0)
  10 RGB         (bits 23..16 R, 15..8 G, 7..0 B)
  11 invalid/reserved
```

Attribute flags:

```text
bit 0 bold
bit 1 underline
bit 2 inverse
bits 3..15 reserved 0
```

The projection encodes effective terminal style values, not Rust enum discriminants.

### 9.5 Damage encoding — 8 bytes

| Offset | Size | Field |
|---:|---:|---|
| 0 | 2 | first_row |
| 2 | 2 | last_row |
| 4 | 2 | flags |
| 6 | 2 | reserved = 0 |

`first_row <= last_row < rows`.

Damage flags:

```text
bit 0 full
bits 1..15 reserved 0
```

ABI 1.0 slots always contain a complete visible snapshot. Damage is renderer guidance describing the canonical change that led to the snapshot; it is never required to reconstruct missing state. First attach, replacement and explicit resync emit a full damage descriptor spanning all visible rows.

The current M001 `TerminalState` coalesces damage to a row range. The projection ABI allows multiple descriptors so later compatible minor versions can batch bounded ranges without changing the slot shape. Pass 5 may emit one descriptor per generation.

## 10. Generation publication and race safety

### 10.1 Writer protocol

The Runtime is the sole writer.

To publish generation `N` into slot `S`:

```text
1. choose the non-committed slot
2. atomic_store(slot.sequence, 2*N + 1, Release)   # odd = writing
3. write the complete slot header payload, cells and damage
4. validate all computed sizes/offsets locally
5. write slot.generation = N
6. atomic_store(slot.sequence, 2*N, Release)       # even = finalized
7. atomic_store(region.publication, (N << 1) | S, Release)
8. attempt/coalesce nonblocking GenerationWake(N)
```

`N` must be monotonically increasing for the lifetime of that projection and must never exceed `2^63-1`. Exhaustion is an explicit projection failure/replacement condition; it does not wrap.

### 10.2 Reader protocol

The client:

```text
1. acquire-load region.publication → (N, S)
2. reject N == 0 or S > 1
3. acquire-load slot[S].sequence → a
4. reject/retry if a is odd or a != 2*N
5. validate fixed header fields, region length, offsets, counts and encoded values
6. copy/read the required slot bytes for rendering into client-owned transient render preparation
7. acquire-load slot[S].sequence → b
8. accept only if a == b == 2*N and region publication still identifies N/S
9. otherwise discard and retry from step 1
```

The implementation must use an unsafe/memory-access strategy appropriate for concurrently mapped shared memory; it must not create normal Rust references whose validity assumes the writer cannot mutate the slot. The unsafe surface must be narrow, documented and fuzz/TSAN-equivalent race tested where tooling permits.

A reader never renders a slot observed with an odd/mismatched sequence.

### 10.3 Slot rollover

The writer may reuse the older slot without waiting for any reader. A reader that races rollover detects sequence/publication change and retries. There is no per-generation acknowledgement and no unbounded slot/history growth.

## 11. Snapshot, damage and resync behavior

### 11.1 Projection producer

Runtime projection logic consumes/coalesces canonical `TerminalState` damage once per execution update and fans the resulting derived snapshot/damage to current attachments. Multiple clients must not independently consume/destructively `take_damage()` from canonical state.

Projection production may be scheduled after canonical terminal mutation, but terminal mutation never waits for completion. If projection work is temporarily unable to run, intermediate damage may be coalesced to the newest complete visible snapshot.

### 11.2 First attach

Successful first attach is one transaction with a private-resource preparation phase and a Runtime-authority commit point:

```text
validate peer/state/role/ExecutionId/capacity
→ allocate AttachmentId + ProjectionId/resources privately
→ read current canonical visible TerminalState
→ publish a complete snapshot generation into the private projection
→ enqueue/begin nonblocking Attached + read-only descriptor delivery successfully
→ publish attachment/controller/projection authority in Runtime registries
→ transition the connection to Attached
```

“Delivery successfully” here means the mandatory `Attached` response and descriptor were accepted by the Runtime's bounded nonblocking send/queue path without an enqueue/send failure. It is **not** a client acknowledgement and terminal progress never waits for it.

Before Runtime authority publication, the IDs, projection mapping, writer descriptor and read-only transfer descriptor are private transaction resources. Any failure in projection creation/publication or initial descriptor delivery drops those resources and leaves no attachment/controller lease/projection registry record behind. After Runtime authority publication, later socket loss is handled by normal idempotent disconnect cleanup.

This ordering deliberately avoids publishing a controller lease before a descriptor-send failure and is the normative M001 commit point defined with ADR-001.

### 11.3 Missed generations

If client last consumed `N` and next observes `N+k`, `k > 1`, it discards any assumption that intermediate damage was observed and consumes the newest slot as a complete visible snapshot. Because every ABI-1.0 slot is complete, no PTY replay is needed.

### 11.4 Explicit resync

A valid `Resync(AttachmentId)` requests a newly published full snapshot of the current canonical visible state using the current projection when capacity permits. Runtime may coalesce multiple pending resync requests. It responds by publishing a new generation and a wake; no synchronous ack is required.

If the current projection is invalid/unavailable or too small, resync creates a replacement projection and sends `ProjectionReplaced`.

### 11.5 Reconnect

A new control connection receives a new `AttachmentId` and `ProjectionId`. It discovers/list-selects the existing `ExecutionId`, attaches, and receives current state. It never receives historical PTY bytes and never instantiates a GUI VT/parser to catch up.

## 12. Execution lifecycle interaction

Execution lifecycle remains SPEC-003 authority.

If an execution finalizes while attached:

- Runtime publishes the latest complete snapshot possible from canonical state;
- sends/coalesces a `Lifecycle` terminal/finalized notification;
- revokes controller authority;
- projection may remain readable only until attachment teardown/connection close required by the implementation's bounded cleanup turn;
- no attachment keeps a completed execution alive beyond SPEC-003 lifecycle.

Client disconnect never terminates an otherwise live execution.

## 13. Malformed/hostile client and projection behavior

The implementation must fail closed and remain bounded for:

- truncated headers/payloads;
- invalid magic/version/type/flags/reserved fields;
- lengths that overflow or exceed maxima;
- disconnect mid-frame or mid-descriptor transfer;
- unexpected/missing/extra `SCM_RIGHTS` descriptors and truncated ancillary control data;
- invalid `ExecutionId`/`AttachmentId`/`ProjectionId`;
- stale controller/attachment identities;
- unauthorized observer input/resize;
- invalid geometry;
- client flood at connection/frame/input/control limits;
- corrupt region/header/slot magic/version/size;
- invalid offsets/counts/slot stride;
- invalid color/scalar/attribute encoding;
- odd/incomplete/mismatched generation sequence;
- stale projection replacement;
- mapping/fstat failure;
- killed/stalled clients.

A malformed client may lose its connection; it must not panic the Runtime, mutate canonical state before validation, allocate from attacker-controlled lengths, leak descriptors/mappings, or stall terminal progress.

## 14. Resource and rollback invariants

Every attachment creation is transactional across:

```text
role/authority validation
AttachmentId allocation
ProjectionId allocation
shared-memory writer fd/map
read-only transfer fd
first complete projection generation
bounded Attached + descriptor send/queue acceptance
attachment/controller/projection registry publication
connection Attached-state transition
```

The authoritative commit point is Runtime registry publication after the initial mandatory descriptor-bearing response has been accepted by the bounded outbound path. Before that commit point all acquired IDs, mappings, descriptors and projection state are private and must roll back on failure. No controller lease is published merely because its requested role passed validation. After publication, disconnect cleanup is idempotent.

Repeated attach/detach and failed creation must return Runtime-owned:

- client sockets;
- shared-memory descriptors;
- `mmap` regions;
- attachment registry records;
- controller leases;
- pending wake/control records;

back to baseline apart from allocator/kernel noise explicitly classified by the benchmark.

Hidden/detached executions hold no dedicated projection region.

## 15. Socket-only benchmark comparator

ADR-001 requires an evidence check against an equivalent compact binary socket-only display path.

The comparator is benchmark/reference code only and must not become a second production terminal architecture.

### 15.1 Equivalent semantics

Comparator A serializes the same logical visible snapshot/cell/style/cursor/mode/damage information into bounded Unix-domain binary snapshot/delta messages. Comparator B uses the production control socket + shared-memory projection in this spec. Both derive from the same canonical `TerminalState`; neither owns another VT/grid.

The benchmark must not make A artificially worse by sending per-cell messages or make B artificially better by omitting equivalent validation/copy work.

### 15.2 Required populations

Measure at least execution populations:

```text
1 / 10 / 50 / 100
```

and report separately:

- visible/attached surfaces;
- hidden/detached executions.

A host ceiling is reported as `PLATFORM_LIMITED`; it is not hidden or worked around by changing the workload.

### 15.3 Required metrics

For equivalent workloads record where meaningful:

- canonical damage → readable projection/snapshot latency;
- wake/readiness → client-readable generation latency;
- bytes copied/written;
- allocations per update;
- Runtime CPU;
- client CPU;
- Runtime RSS;
- client RSS;
- full snapshot cost;
- reconnect/resync cost;
- high-output throughput;
- stalled-client behavior;
- killed-client cleanup;
- multiple-client behavior;
- fd/mapping/resource counts before/after repeated attach/detach.

Record hardware model/chip, macOS version/build, build mode, commit SHA, terminal geometry, workload, repetitions and percentile methodology.

### 15.4 Decision rule

If socket-only is materially simpler and measurably equivalent or better across the M001 workload envelope, implementation must not silently preserve the hybrid production decision. Prepare an evidence-backed ADR-001 revisit for architectural review.

Until such a review changes authority, ADR-001 remains selected production architecture.

Canonical VT/state remains Runtime/`TerminalExecution` authority regardless of comparator outcome.

## 16. Tests and fuzz requirements

### 16.1 Protocol

- valid framing and every M001 message;
- arbitrary frame chunking;
- truncated header/payload;
- invalid magic;
- unsupported major/minor;
- oversized length;
- length arithmetic overflow;
- malformed exact lengths;
- unknown message;
- invalid state transitions;
- disconnect mid-frame;
- missing/extra descriptor transfer;
- unexpected inbound descriptors and truncated/malformed ancillary data;
- bounded receive/outbound queues.

### 16.2 Authorization

- allowed same-UID peer;
- UID mismatch rejection where injectable/testable;
- socket open alone grants no attachment/controller;
- observer attach/read/resync;
- observer input/resize rejection;
- one controller lease;
- second controller rejection/no preemption;
- controller disconnect releases lease;
- invalid/stale ExecutionId/AttachmentId.

### 16.3 Projection

- exact ABI header/slot/cell/damage encoding fixtures;
- valid full snapshot;
- canonical damage update;
- generation monotonicity;
- odd/incomplete generation rejection;
- slot rollover race/retry;
- missed-generation recovery;
- explicit resync;
- corrupt header/version/region length;
- invalid offsets/counts/stride;
- invalid scalar/color/attribute;
- stale mapping/projection ID;
- projection replacement after growth;
- client descriptor cannot be mapped writable through the received read-only fd.

### 16.4 Failure/resource

- client killed during projection update;
- client stalls with high-volume PTY output;
- Runtime drops slow client at bounded control capacity;
- shared-memory create/open/map/ftruncate failures;
- socket bind/listen/accept failures;
- partial attach/descriptor-send rollback;
- repeated attach/detach cleanup;
- Runtime shutdown cleanup;
- capacity exhaustion and recovery;
- cleanup idempotence.

### 16.5 Integration

- real `TerminalExecution` output changes canonical `TerminalState` then projection;
- projection matches canonical visible state/cursor/modes;
- no client acknowledgement required for further PTY reads/VT generations;
- disconnect while high-volume output continues;
- reconnect returns current full state without PTY replay;
- multiple observer/controller behavior follows authority rules.

### 16.6 Fuzz

Activate the existing pending M001 surfaces only against real production APIs:

- `local-binary-protocol-decode`;
- `shared-projection-validation`;
- `reconnect-resync-state-machine`.

Retain regression seeds for every discovered crash/invariant violation. Fuzz adapters must exercise the production decoder/validator/state machine, not no-op copies.

## 17. Performance implementation constraints

Production Pass 5 must avoid:

- per-cell IPC or Swift/Rust callbacks;
- per-cell heap allocation;
- synchronous GUI acknowledgements;
- synchronous request/response on PTY output progress;
- unbounded snapshot history/queues;
- thread/process per attachment;
- polling/busy loops;
- direct export of canonical grid memory;
- hidden/detached renderer/projection resources.

Use bounded reusable buffers, coarse snapshot/run work, damage/generation coalescing, explicit capacities and nonblocking one-way wakes.

The explicitly measured projection copy is permitted; additional copies/allocations must be measured and justified.

## 18. Logging and privacy

Protocol/security diagnostics may log numeric error classes, counts, lifecycle state and opaque IDs when needed for development, but must not log:

- raw terminal input/output;
- environment values;
- shared-memory names;
- secret/token material;
- arbitrary malformed payload bytes.

No protocol field introduced by M001 contains a credential or secret.

## 19. Compatibility and change discipline

ABI/wire major 1 minor 0 is internal M001 protocol, not a public stable SDK promise.

Nevertheless, implementation must obey it exactly so tests, native client and Runtime cannot drift silently. Any incompatible field meaning/layout/framing change before M001 completion updates this specification and increments the appropriate version. A public API/ABI promise requires separate future authority.

ADR-001 remains architecture authority; this spec cannot switch production to socket-only based on convenience. Measured comparator evidence may trigger an ADR review.

## 20. Pass-5 acceptance

Pass 5 production implementation is acceptable only when evidence demonstrates:

- exact wire/projection behavior above;
- same-user peer verification and explicit observer/controller authority;
- read-only client projection access;
- one Runtime writer and no second terminal state;
- incomplete/torn generations rejected;
- first attach/current snapshot, missed-generation recovery and resync;
- stalled/killed clients never backpressure PTY → VT;
- bounded resources and leak-free repeated lifecycle;
- hostile protocol/projection inputs safely rejected;
- active fuzz surfaces/regression corpus green;
- equivalent socket-only vs hybrid benchmark evidence recorded;
- ADR-001 justified or an evidence-backed revisit prepared;
- canonical build/test/check/bench and Pass-5-specific gates green;
- independent architecture/security/performance review has no unresolved red/orange issue;
- no Pass-6+ scope is present.
