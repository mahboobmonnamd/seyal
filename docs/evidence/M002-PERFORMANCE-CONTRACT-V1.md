# M002 performance evidence contract v1

Status: proposed contract for Issue #673. This document defines evidence shape
and boundaries; it does not declare any product gate as passing.

## Required identity

Every retained result MUST record the exact production SHA, benchmark-harness
SHA, contract version, build mode, Rust/Xcode/macOS versions, Apple hardware
model and architecture, display scale and refresh, power/thermal state,
workload-fixture hash, execution topology, baseline SHA, cohort/run counts,
percentile method, raw-log locations, metric boundary, and evidence class.

The required percentile method is nearest-rank. A valid physical-host run uses
five fresh-process cohorts, twenty warmups per cohort, and one hundred retained
samples per cohort unless an approved issue-specific exception is recorded.

## Evidence classes

| Class | Establishes | Does not establish |
| --- | --- | --- |
| `CI` | schema, validator, fixture, and structural guard correctness | absolute latency, CPU, RSS, headed presentation |
| `SYNTHETIC` | deterministic component behavior and comparative direction | PTY, Runtime, AppKit, Metal, or key-to-photon behavior |
| `NATIVE_HEADED` | real PTY/Runtime/client/Metal route under WindowServer | physical keyboard or display scanout latency unless instrumented |
| `PHYSICAL_ARM64` | controlled Apple-Silicon Release measurements on the exact executable | any boundary not explicitly measured |

Platform-limited, thermal, display-session, PTY-capacity, and other invalid
environment outcomes MUST be retained as such. They are neither product
passes nor silently discarded samples.

## M002 gate families

The contract covers these independently reported boundaries:

- HistoryStore active reflow and sealed-segment lazy reflow, using the frozen
  #818 ceilings: p50/p95/p99 active `2/4/8 ms` and sealed `1/2/4 ms`.
- PTY-read to canonical `TerminalState` mutation.
- Damage/projection and client-cache readiness.
- Metal preparation/submission and the explicitly named visible-frame proxy.
- Input admission through the named presentation boundary; this is not called
  key-to-photon unless scanout is actually measured.
- Sustained high-output responsiveness while input, resize, and scrolling are
  active.
- Startup, idle CPU, RSS, file descriptors, threads, and teardown recovery for
  1/10/50/100 execution populations where the host permits them.

History-specific required matrix dimensions remain 10k/100k/1M retained
content, 1/10/50/100 executions, widths 40/48/64/80/96/132/160, and
ASCII/styled/CJK/emoji-combining workloads. #819's 16 KiB sealed payload,
32 KiB mutable tail, 32 MiB per-execution history, 256 MiB aggregate history,
4 MiB per-execution cache, and 32 MiB aggregate cache are floors from #818;
this contract may tighten them but MUST NOT weaken them.

## Decision rule

No ceiling is inferred from a best run. A release gate requires a recorded
baseline SHA, accepted noise policy, valid cohorts, complete raw evidence, and
an explicit comparison rule. Missing metrics are recorded as `unknown` or
`not-instrumented`, never estimated or backfilled.
