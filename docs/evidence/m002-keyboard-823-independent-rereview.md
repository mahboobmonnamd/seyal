# Independent closing re-review — M002 #823 keyboard architecture

- **Verdict:** **NO-GO — do not use `Closes #823`.**
- **Candidate branch:** `issue/823`
- **Exact SHA reviewed:** `b6816764467ba7e2e0490f78f7a2744e517ec768` (`fix(823): correlate TerminalKeyV2 backpressure to sent action IDs`)
- **Comparison base:** `origin/master` at `9b8408506ee2137c4b0cb20b4b2b2a92549649ab`
- **Review host:** Linux 6.12.94+ x86_64
- **Authority reviewed:** `AGENTS.md`, `docs/specs/SPEC-006-M001-NATIVE-INPUT-RESIZE.md` §§21.5–21.7, and [issue #823](https://github.com/mahboobmonnamd/seyal/issues/823).
- **Scope:** independent source and retained-evidence review. No production code or candidate-tracked files were modified.

## Exact-tree qualification

`git rev-parse HEAD` returned the SHA above and `git diff --check
origin/master...HEAD` passed.

The worktree was initially clean, but during this review it acquired the
untracked `docs/evidence/m002-keyboard-823-source-p1.md`. It is not part of
`b681676`, is not production evidence for this exact commit, and is not
attributed to the candidate. At final inspection the branch had advanced to
`a05b13c` with that documentation only, and
`docs/evidence/m002-keyboard-823-headed-manual.md` was unstaged. Neither
post-review documentation change is assessed as evidence for `b681676`; the
production files are unchanged between the requested SHA and `a05b13c`.

## P1 re-review — V2 Error-ID correlation

**Not fully resolved; the Backpressure subset is improved, but the §21.5
contract remains incomplete.**

`b681676` makes these source improvements:

- `LocalDisplayClient` now keeps bounded connection-local
  `last_sent_v2_action_id` and `highest_v2_error_id` fields.
- `submit_terminal_key_v2` rejects zero and non-increasing IDs before
  encoding, and records the high-water ID after the frame enters the outbound
  queue.
- A type-29 `Backpressure` Error with a nonzero ID now rejects IDs above the
  sent high-water and duplicate/non-monotonic IDs; an accepted ID becomes the
  highest received V2 error and reports visible `ClientBackpressure`.

However, §21.5 applies the correlation rule to the Error for **every
structurally readable V2 rejection**, not only queue backpressure. Runtime
source sends the V2 action ID for `PermissionDenied`, `StaleIdentity`,
`InvalidExecution`, unsupported encoding (`MalformedPayload`), and
`Backpressure`. In `classify_incoming_error`, only the
`Backpressure`/type-29 combination reaches `classify_v2_backpressure`.
Every other type-29 Error is forwarded to `classify_server_error` without
validating `detail_code` or advancing `highest_v2_error_id`. Thus a forged,
out-of-range, duplicate, or non-monotonic action ID on those V2 errors is not
reported as `ClientError::Protocol`, contrary to §21.5.

The new unit test also explicitly accepts a type-29 `Backpressure` Error with
`detail_code == 0` as `ClientError::Server(ErrorCode::Backpressure)`, rather
than a protocol failure. Zero cannot correlate a valid V2 action. A generic
zero-detail error is allowed for an unreadable/fatal frame, but a V2
Backpressure error is emitted only after successful V2 decoding and must
carry the validated nonzero action ID.

Finally, the field named `last_sent_v2_action_id` is advanced when
`admit_frame` enqueues the frame, before `flush_control_write` has completed
the frame to the socket. A `WouldBlock` or partial write therefore leaves the
high-water ahead of wire-complete actions. This is not a demonstrated
violation if “sent” is intentionally defined as client admission, but it is
not the ordinary wire-send meaning used by §21.5 and is untested. The
implementation needs either a wire-complete sent boundary or an explicit,
spec-consistent justification and an adversarial partial-write case.

The prior P1 is consequently only partially corrected, not closed.

## Remaining completion blockers

1. **P1 — complete V2 Error correlation.** Validate the action ID and
   monotonic highest-error bound for every correlated type-29 Error category,
   reject impossible type-29 zero-detail Backpressure as protocol-invalid,
   and resolve/test the enqueue-versus-wire-send high-water boundary.
2. **P1 evidence gate — native/physical/target-TUI/latency remains
   unverified.** The retained headed-manual ledger records every manual row as
   `ENVIRONMENT_UNSUPPORTED` on Linux. No Seyal.app, physical keyboard/keypad,
   layout, IME, or Metal display was available. This is not PASS. Required
   shell/Neovim/modern-TUI behavior, Option policy, Command non-leak,
   dead-key/IME lifecycle, repeat under high output, and physical native
   observations remain open.
3. **P1 evidence gate — performance remains missing.** There is no exact
   baseline/candidate Release ARM64 comparison with three independent
   1,000-accepted-action workloads, native-key→Runtime and key→PTY
   percentiles, CPU/RSS/queue high-water, and rejected/deferred counts.
   The retained Linux benchmark declares `performance_claim=false`; it is not
   a substitute.
4. **P3, but still a normative closure gap — held-key overflow is silent.**
   `InteractiveMetalSurfaceView.keyDown` increments the action ID then returns
   when `heldKeyboardKinds.count >= 256`, without setting `nativeFailure` or
   refreshing the visible failure layer. It also applies that capacity check
   before considering whether the key is already tracked, so a repeat at
   capacity is dropped. This does not meet §21.3’s visible rejection of a new
   tracked press and leaves capacity-repeat behavior unrepresented.
5. **P3, but still a normative closure gap — §21.6 matrix and §21.7 adverse
   evidence are incomplete.** The terminal target has three keyboard tests
   and the protocol target has limited V2 fixtures. They do not enumerate all
   V2 kinds × flags 0/1/2/3 × press/repeat/release × accepted modifiers and
   printable-ASCII/shifted-field outcomes as required. The retained 45-second
   decoder fuzz campaign was at `9848add`, not this exact SHA, and its own
   ledger classifies it as smoke-only. V2 observer/stale/detach, persistent
   pressure/fairness, and a real Runtime/PTY saturation recovery result remain
   incomplete; on this Linux host the macOS-gated local-IPC test executed zero
   tests.
6. **Exact-head qualification is incomplete.** The retained `make check`,
   native build/smoke, and fuzz claims predate `b681676`; no exact-head
   macOS/client/native `make build`, `make test`, or `make check` result was
   supplied. The source P1 test itself is under the macOS-gated
   `seyal-client::local` module and did not run here.

## §21.7 evidence-gate audit

| Gate | Disposition |
| --- | --- |
| Modes/key bytes/negotiation | Partial automated coverage only; required finite encoder matrix is absent. |
| Modern events | No real Neovim flags-3 handshake result. |
| Wire/security/admission/recovery | Runtime source has positive V2 authorization/admission ordering, but P1 Error-ID handling is incomplete and required V2 adverse/recovery coverage is absent. |
| Native | Unverified. `ENVIRONMENT_UNSUPPORTED` is a record of missing capability, not a passing native result. |
| Fuzz/property | Partial only; prior short decoder smoke is neither exact-head evidence nor full matrix/property coverage. |
| Performance | Missing native ARM64 baseline/candidate latency and resource qualification. |
| Exact-head validation | Only the focused Linux checks below were performed for this re-review; no exact-head macOS/client/native full gate is established. |
| Independent review | Completed by this review, with unresolved blockers and a NO-GO verdict. |

## Tests actually run by this reviewer

```text
git diff --check origin/master...HEAD
PASS

cargo test -p seyal-protocol --test pass7_input_resize --locked
PASS — 8 passed

cargo test -p seyal-terminal --test m002_keyboard --locked
PASS — 3 passed

cargo test -p seyal-runtime --test pass7_local_ipc --locked
PASS command / 0 tests executed (macOS-gated on this Linux host)

cargo test -p seyal-client --locked
PASS command — 7 unrelated block_cache tests passed;
seyal-client local/FFI and macOS integration targets executed 0 tests
```

No result above claims that `seyal-client::local`, FFI, Runtime local-IPC V2,
headed AppKit/Metal, IME, physical keyboard, target-TUI, or latency tests
passed. They did not execute on this Linux host.

## Closure disposition

Keep #823 open. The candidate must use a non-closing relationship until the
P1 correlation contract is complete and the exact-head §21.7 native,
target-workload, fuzz/property, and performance gates are actually satisfied
on an appropriate headed Apple Silicon environment. A later independent
exact-head closing review is required.
