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
- leave production Runtime/TerminalExecution lifecycle semantics unchanged.

Acceptance: isolated `pass7_local_ipc` plus the full exact-head `make check` gate must pass. A single green rerun is not sufficient evidence if the full gate regresses again.

## Block selection full-suite failure

Observed symptom: the final full native run passed 44/45; `testSelectingABlockRevealsRustBlockDetailsInInspector` failed although the same Block-selection case had passed in isolation.

The Rust selection action already validates the selected Block and a successful action bumps application generation. The remaining failure is therefore being treated as a native transcript/hit-testing/reconciliation defect until disproved, not as missing Rust product state.

The transcript currently scrolls to live end when a new Block is first projected. History-range replies arrive asynchronously and can later grow existing Block bodies. In a persistent Runtime/full-suite run this can move the newly submitted Block out of the visible clip after the initial live-end scroll. The production fix must preserve live following only when the user was already at the live end; it must not yank a user who intentionally scrolled back through history.

Acceptance: the Block-selection XCUI test must pass in the combined/full native suite, not only in isolation, and the final exact-head native gate must be green.
