# M002 #824 active remediation

This note records the diagnose → fix → rerun loop required by the M002 closure audit. It does not close #824 or #672.

## Runtime IPC full-suite failure

Observed symptom: `make check` failed repeatedly in `pass7_local_ipc` while creating test executions with `Exec(Io(code: -6))`, while the isolated suite and hosted CI could pass.

Root-cause candidates found in the test harness:

1. `pass7_local_ipc::config()` used only a process-local counter for singleton and local-IPC paths (`s7-0.lock`, `s7d-0`, ...). Separate Cargo test processes each start the counter at zero, so concurrent test binaries can collide in the shared temporary directory.
2. The harness owns disposable Runtime/PTy children but previously relied on ordinary `Drop`. Seyal production semantics deliberately do not terminate a `TerminalExecution` on ordinary Drop; a test-owned Runtime must explicitly perform controlled shutdown.

Remediation on `issue/824`:

- include the process id plus the process-local counter in test singleton/runtime-dir names;
- perform controlled Runtime shutdown from the test Harness destructor;
- leave production Runtime/TerminalExecution lifecycle semantics unchanged;
- rustfmt the `runtime_dir_override` assignment so Foundation Quality does not fail `cargo fmt --check`.

**Acceptance status (exact head `ad50bd1`):** hosted Foundation Quality
`make check` PASS
([run 35242876394](https://github.com/seyal-org/seyal/actions/runs/35242876394)),
including `pass7_local_ipc`. The historical local double-fail is retained as
diagnosis evidence only.

## Block selection full-suite failure

Observed symptom: the final full native run passed 44/45; `testSelectingABlockRevealsRustBlockDetailsInInspector` failed although the same Block-selection case had passed in isolation.

The Rust selection action already validates the selected Block and a successful action bumps application generation. The remaining failure is therefore being treated as a native transcript/hit-testing/reconciliation defect until disproved, not as missing Rust product state.

The transcript currently scrolls to live end when a new Block is first projected. History-range replies arrive asynchronously and can later grow existing Block bodies. In a persistent Runtime/full-suite run this can move the newly submitted Block out of the visible clip after the initial live-end scroll. The production fix must preserve live following only when the user was already at the live end; it must not yank a user who intentionally scrolled back through history.

Remediation on `issue/824`:

- track `followingLiveEnd` from user transcript scroll proximity;
- on new Block projection and on history-range body growth, re-pin to live end only while following;
- wait for the submitted card to become hittable before the XCUI click so an off-clip ghost cannot pass existence and fail selection.

CI at `d44662c`: Block-selection XCUI and all four workload XCUI cases PASS. Remaining CI FAIL was
`testNativeIMELiveCallbacksDeliverOnlyCommittedUTF8AndTrackCursor` timing out on the
"production Runtime and projection connected" wait; harden that wait with a connection poll.

**Acceptance status (exact head `ad50bd1`):** hosted `make test` native suite
PASS — Block selection XCUI PASS (28.53s), component live IME PASS (1.15s),
XCUI 19 executed / 0 failures / 1 ABC-layout skip
([run 35242876394](https://github.com/seyal-org/seyal/actions/runs/35242876394)).
The historical local 44/45 FAIL is retained in the headed ledger as superseded.
