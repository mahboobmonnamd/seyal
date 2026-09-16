# Independent M002 #823 keyboard review

- **Review target:** `issue/823` at `f24e2a4d1de57f6d0b1af4d69ecbdb6be06630b4`
- **Expected head:** matched (`f24e2a4`)
- **Comparison base:** `origin/master` at `9b8408506ee2137c4b0cb20b4b2b2a92549649ab`
- **Authority:** `AGENTS.md`; `docs/specs/SPEC-006-M001-NATIVE-INPUT-RESIZE.md` §21, especially §§21.3, 21.5–21.7; and the #823 completion gates
- **Review scope:** independent source/evidence review only. No production code was modified.
- **Date:** 2026-09-10 UTC

## Verdict

**NO-GO. `Closes #823` is not allowed.**

`f24e2a4` corrects the narrow type-29 backpressure classification and installs
the V2 capability/action-ID-zero admission guards. It does not fully satisfy
the required client-side V2 error-correlation contract, and the mandatory
headed/manual/target-TUI/latency evidence remains absent. The latter is
independently sufficient to block a closing PR.

This review does not claim M002 or #823 is complete.

## Exact-tree and evidence provenance

The review was pinned to the candidate SHA above, and `git diff --check
origin/master...f24e2a4` passes. During review the branch advanced to
docs-only commit `b05bae2cd6dc03c9f6d25b3fb57bdc68a902c999`, which contains
the exact-head-ledger edit and headed-manual record. That later commit is
outside the requested candidate review and is not attributed to `f24e2a4`;
I read it as supplied evidence only.

The headed-manual record is candid: every manual row is
`ENVIRONMENT_UNSUPPORTED` on this Linux host, no `Seyal.app` was launched,
and no physical keyboard, keypad, IME input source, or Metal display was
available. That status is not a passing result and cannot discharge a §21.7
native gate.

## Prior source findings

| Previous finding | Disposition at `f24e2a4` | Basis |
| --- | --- | --- |
| P1: type-29 Runtime backpressure was treated as fatal rather than a recoverable input-admission refusal | **Narrow defect resolved in source; final P1 resolution rejected** | `classify_server_error` now recognizes `Backpressure` for `TerminalKeyV2` only when `detail_code != 0`, matching the Runtime's capacity rejection in `handle_terminal_key_v2`, which echoes its nonzero action ID. However, the required client sent/highest-error ID validation remains absent; see P1 below. |
| P2: V2 could be submitted through the client/FFI when extended-key capability was not negotiated | **Resolved in source** | `LocalDisplayClient::submit_terminal_key_v2` now rejects an absent `extended_terminal_key_supported` flag with `UnsupportedInteractiveCapability` before frame encoding. The FFI entry point funnels through that method, and `action_id == 0` is also rejected before encoding. FFI error mapping returns unsupported as `-12`. |

The new P2 guard is correctly located at the common client admission boundary,
not only in the Swift caller. Its newly added Rust tests are macOS-gated with
the entire `local` module, so they did not execute on this Linux review host.

## Findings

### P0

None found in the reviewed source.

### P1 — client accepts an arbitrary correlated V2 backpressure error

SPEC-006 §21.5 requires the client to retain connection-local sent/highest-error
action-ID bounds, without an unbounded pending-action map. An Error whose
action ID is outside the sent range, or is duplicate/non-monotonic relative to
prior V2 errors, is a protocol failure.

`LocalDisplayClient` has no sent-V2 or highest-V2-error field. Its only
incoming handling calls `classify_server_error`; the new branch accepts every
`Backpressure` Error with message type 29 and **any nonzero** `detail_code` as
`InputAdmissionFailure::ClientBackpressure`. It neither verifies that the ID
was sent by this connection nor rejects a duplicate/stale error.

Runtime-side monotonic admission (`handle_terminal_key_v2`) does not replace
the receiver-side requirement: it constrains incoming client frames, while
the client must validate the Error it receives. Thus the narrow previous
backpressure-classification defect is fixed, but the prior review's stated
V2 action-ID-correlation requirement remains unsatisfied. A fabricated or
stale type-29 backpressure Error is silently converted to a retryable failure
instead of terminating as a protocol violation.

Before another final review, the client needs bounded sent/highest-error
tracking and focused cases for in-range accepted errors, out-of-range errors,
duplicate errors, and non-monotonic errors. A real client/Runtime
queue-saturation test must also show that an in-range correlated type-29
Backpressure keeps the attachment usable and preserves later FIFO work.

### P1 — mandatory native, workload, and performance evidence is still open

§21.7 says source fixtures cannot replace native evidence. The supplied
evidence ledger and headed record leave these completion gates open:

- full native/XCUI keyboard matrix, including keypad and repeat/release;
- physical keyboard/layout coverage, Option policy both values, dead keys,
  IME mark/commit/cancel, candidate navigation, and Command non-leak;
- real Neovim flags-3 negotiation and its documented semantic cases;
- held repeat/release under high terminal output;
- Release ARM64 baseline/candidate comparison: three independent runs of at
  least 1,000 accepted actions each, native-key→Runtime and key→PTY
  p50/p95/p99/max, plus CPU/RSS/queue high-water and rejected/deferred counts;
- exact-head native build/test/check evidence after `f24e2a4`.

The retained 45-second Pass-7 fuzz campaign is explicitly only `ci-smoke`,
not the required longer campaign. The prior full `make check` was at
`fe70733`, an ancestor before later keypad changes and before `f24e2a4`; it is
not exact-head validation of this candidate.

`ENVIRONMENT_UNSUPPORTED` correctly records why the Linux agent could not run
the headed checks, but it leaves every required headed/manual result
unverified. It is therefore a P1 completion blocker, as requested.

### P2

No remaining P2 finding in the reviewed capability-admission path. The prior
P2 is resolved in source as described above. The lack of an executed macOS FFI
test is an evidence gap, not a second path around the central guard.

### P3 — held-key capacity rejects silently and drops a tracked repeat

In `InteractiveMetalSurfaceView.keyDown`, after allocating an action ID, the
code returns at `heldKeyboardKinds.count < 256` failure without setting
`nativeFailure` or using the visible input-failure presentation. This violates
§21.3's requirement that a new tracked press overflowing the 256-key bound be
rejected visibly.

The same check runs before replacing/using an existing entry, so a repeat for
an already tracked key is also dropped when the map is full. That is not the
specified "new tracked press" overflow behavior and leaves repeat behavior
unrepresented at capacity. It is bounded and emits no terminal bytes, so this
is an availability/UX P3 rather than an input-injection P0/P1.

### P3 — encoder, wire/admission, and fuzz coverage does not meet §21.6/§21.7

The terminal test target has three tests and the protocol target has two V2
tests. They do not enumerate all V2 kinds × keyboard flags 0/1/2/3 ×
press/repeat/release × accepted modifier combinations, classifying every
result as exact bytes, successful no-byte, or explicit unsupported as §21.6
requires. Printable ASCII base and valid/invalid shifted-field coverage is
also not exhaustively demonstrated.

The Pass-7 fuzz target calls `TerminalKeyV2::decode`, but the committed corpus
contains V1 and truncated-key seeds only; there is no retained 40-byte valid
V2 seed. The evidence itself also notes thinner V2 observer/stale/detach
coverage. The shared Runtime authorization path is fail-closed in source, but
that does not replace the required adverse-case fixtures.

## Positive source observations

- Runtime validates V2 structure, bilateral capability, and per-connection
  monotonic action IDs before Controller authorization, canonical-mode lookup,
  encoding, and queue mutation.
- `handle_terminal_key_v2` sends a capacity rejection as
  `Backpressure`, message type 29, with the action ID in `detail_code`, which
  is the correct Runtime-side correlation shape.
- Runtime reads canonical `TerminalState` modes at admission and enqueues the
  resulting immutable bytes. Swift does not become a mode/escape-sequence
  authority.
- The client-side V2 capability guard and zero-ID guard happen before frame
  encoding; old peers are not sent an unnegotiated type-29 frame through this
  API.
- The native Command routing and typed V2 modifier surface remain fail-closed
  in source, but this does not substitute for headed observation.

## Verification performed on this Linux host

```text
git rev-parse HEAD                                           f24e2a4d1de57f6d0b1af4d69ecbdb6be06630b4
git rev-parse origin/master                                  9b8408506ee2137c4b0cb20b4b2b2a92549649ab
git diff --check origin/master...f24e2a4                     PASS
cargo test -p seyal-protocol --test pass7_input_resize \
  --locked terminal_key_v2                                   PASS (2)
cargo test -p seyal-terminal --test m002_keyboard --locked  PASS (3)
```

`seyal-client::local` and its FFI are compiled only on macOS
(`crates/seyal-client/src/lib.rs`). Consequently the two new f24 client tests
and the input-error classifier test ran **0 tests** here, and the macOS-gated
Runtime local-IPC V2 fixture also ran **0 tests**. No passing macOS, headed,
physical-keyboard, IME, target-TUI, or latency result is claimed by this
review.

## Evidence-gate disposition

| §21.7 gate | Disposition |
| --- | --- |
| Modes/key bytes/negotiation | Partial automated coverage; exhaustive §21.6 matrix is missing. |
| Modern events | Real Neovim flags-3 handshake remains unverified. |
| Wire/security/admission/recovery | Source has positive structural/authorization checks, but client Error ID correlation and V2 adverse coverage remain incomplete. |
| Native | Unverified: the current result is `ENVIRONMENT_UNSUPPORTED`, not PASS. |
| Fuzz/property | Partial only: decoder coverage and a 45-second smoke campaign do not meet the required matrix/campaign. |
| Performance | Missing native ARM64 baseline/candidate latency and resource matrix. |
| Exact-head validation | Missing for macOS/client/native surfaces after `f24e2a4`. |
| Independent review | This review is independent and returns NO-GO. |

## Closure disposition

Do not merge this as a closing PR and do not use `Closes #823`.

At minimum, correct and test the client V2 Error-ID contract and the retained
P3 defects, then obtain the exact-head §21.7 evidence on an appropriate headed
Apple Silicon environment. A subsequent independent exact-head review is
required before considering closure.
