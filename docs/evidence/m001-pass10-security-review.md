# M001 Pass 10 — Focused security review

**Owning Issue:** #727  
**Freeze candidate (pre-#776):** `3f7b2d926dcab888e4dadc480033c1d137fd5ad7`  
**Date (UTC):** 2026-09-05  
**Class:** controlled-host / exact-head test evidence + focused threat pass  
**Prior artifact (not sufficient alone):** `docs/evidence/pass9-security-review-745.md` (Pass 9 packaging head)

## Scope

Fresh Pass 10 review of M001 terminal fundamentals security boundaries on the Phase 2 validation path:

- PTY / child process boundary
- per-user Runtime UDS endpoint location, ownership, permissions
- same-user authentication and connection-bound attachment identity
- Observer / Controller authorization
- malformed / bounded protocol and projection inputs
- unexpected ancillary descriptors (`local_ipc_ctrunc`)
- slow-client / resource abuse isolation
- reconnect / stale identity
- FFI misuse fail-closed behavior
- renderer / native input privacy (no terminal secrets via AX)
- Block metadata bounds
- no commercial / cloud / licensing requirement in OSS terminal path

## Method

1. Re-run adversarial / protocol / FFI suites on the freeze lineage:
   - `cargo test -p seyal-runtime --test local_ipc_protocol --locked` → EXIT:0
   - `cargo test -p seyal-runtime --test local_ipc_adversarial --locked` → EXIT:0
   - `cargo test -p seyal-runtime --test local_ipc_ctrunc --locked` → EXIT:0
   - `cargo test -p seyal-client --test ffi_misuse_macos --locked` → EXIT:0
   - Pass 10 disconnect-during matrix → EXIT:0
2. Confirm layering / OSS↛commercial: `scripts/check-layering.py` EXIT:0
3. Review Pass 10 finding lineage (#748–#760) for security-relevant items (FFI ABI/panic, TerminationFailed recovery, disconnect matrix) as closed with tests.
4. Treat Pass 9 security review #745 as historical context only; do not inherit its SHA as Pass 10 proof.

## Findings

**No medium/high/critical findings** with a realistic same-user or local-attacker exploit path beyond already-tested fail-closed contracts.

Residual / process notes (below medium bar):

- Controlled fuzz campaigns for §6.9 must complete on the final freeze head before claiming fuzz-clean.
- Pass 9 five-cohort production budgets must be re-measured on the final freeze head (retained JSON files are older SHAs).
- Headed presentation evidence remains `controlled-host` with `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1`; CI benches stay labeled `CI`.

## Verdict

**PASS** for Pass 10 §6.10 focused threat review on the Phase 2 evidence branch, contingent on re-affirmation after `#776` merge / re-freeze (harness-only delta expected: Pass9MergeAcceptance RSS ordering + macos_environment drain test).
