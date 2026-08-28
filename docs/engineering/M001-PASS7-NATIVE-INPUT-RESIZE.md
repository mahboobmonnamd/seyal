# M001 Pass 7 native input and resize evidence

This record captures the reproducible automated Pass 7 benchmark run for the
implementation head `d0d44869593209064a30787995f81e1eeb9456d2` on 2026-08-28.
It is implementation evidence, not a release-readiness claim.

## Environment and command

```text
host: Apple M5 Pro / arm64
OS: macOS 26.5.2 (25F84)
Rust: rustc 1.98.0 (88d9e12ae 2026-08-18)
build: Release
percentiles: nearest-rank
representative repetitions: 120
command: make bench
```

All benchmark records use `performance_claim=false`. Input text, composition
text, control bytes and terminal contents are not emitted by the benchmark
records.

## Pass 7 measured boundaries

| Boundary/case | p50 | p95 | p99 | max |
| --- | ---: | ---: | ---: | ---: |
| native callback → client admission | 0.125 us | 0.209 us | 2.042 us | 7.875 us |
| client admission → socket complete | 1.167 us | 2.584 us | 3.250 us | 3.708 us |
| Runtime frame admission → PTY write | 2.083 us | 2.667 us | 3.375 us | 6.083 us |
| native callback → PTY write | 7.000 us | 11.667 us | 15.792 us | 18.041 us |
| resize 120x40 | 8.209 us | 10.458 us | 13.000 us | 13.166 us |
| resize 512x256 | 95.584 us | 110.083 us | 116.541 us | 117.875 us |

The measured values are below the SPEC-006 latency budgets. Pass 7 resource
records reported a client queue high-water mark of 56 bytes for resize, an
idle attributable RSS delta of 1,248 KiB, and no persistent write readiness
when idle.

The validation matrix also measured legal 1/16/64 KiB commits, atomic
rejection of 65,537-byte commits, 64-key repeat bursts, input under sustained
output and alternate-screen input/resize. All completed with
`performance_claim=false` and the expected completion/atomicity markers.

## Explicit limits and remaining acceptance work

The benchmark's native boundary is a controlled synthetic `NSEvent`/FFI
equivalent and is labelled `CONTROLLED_FFI_EQUIVALENT_APPKIT_EVENT_NOT_CLAIMED`.
It does not prove a physical keyboard, dead-key or third-party IME session.
The automated matrix does not claim persistent injected PTY winsize failure
or the full physical AppKit shell/IME/focus acceptance path. Those remain
manual/native acceptance gates from SPEC-006 section 16.4 and must be run on
the exact final head before Pass 7 is marked complete.
