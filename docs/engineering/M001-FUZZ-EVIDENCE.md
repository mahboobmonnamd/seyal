# M001 fuzz evidence grade and surface audit

**Owning Issue:** #758  
**Authority:** `docs/engineering/M001-PASS10-VALIDATION.md` §6.9, `fuzz/targets.toml`

This document defines how Pass 10 scores fuzz evidence and records the Pass 9
surface decision. It does not invent new production decoder APIs.

## Evidence grades

| Grade | Typical duration | Provenance required | May score §6.9 `PASS` alone? |
|---|---|---|---|
| `ci-smoke` | ≤30s PR CI (`pass5-fuzz.yml` pull_request path) | workflow run on the PR head | **No** |
| `nightly-campaign` | ≥600s scheduled/workflow_dispatch campaign per active production target | workflow run ID + exact git SHA + target list | Yes, when all required active targets are clean |
| `controlled-campaign` | longer/local/controlled-host campaign at or above nightly floors | retained artifact/log with SHA + duration + RSS limits | Yes |

Rules:

1. A green `ci-smoke` run proves buildability and short mutation smoke only.
2. Citing “fuzz clean” from 30s CI alone is a Pass 10 evidence error.
3. Registry smoke (`python3 scripts/fuzz-smoke.py`) proves retained-seed adapters
   and registry↔libFuzzer parity; it is not a mutation campaign.
4. Exact-head campaign provenance must name the git SHA under test.

CI and nightly workflows print `FUZZ_EVIDENCE_GRADE=` so logs cannot be
mislabelled.

## Campaign floors (active production targets)

For `nightly-campaign` / milestone evidence, each active production libFuzzer
target runs with at least:

- `-max_total_time=600`
- `-timeout=10`
- `-rss_limit_mb=1024`
- `-print_final_stats=1`
- the registry campaign corpus for that target

`ci-smoke` may keep `-max_total_time=30` for PR latency.

## Candidate-B shared-projection

`shared-projection-validation` is status `non-production-comparator`.

Architecture proof: production attachment/display is Candidate-D binary
snapshot/delta (`ADR-001`, `docs/engineering/LOCAL-ATTACHMENT.md`). The earlier
shared-projection path is isolated behind the non-default
`benchmark-shared-projection` feature and is not reachable from normal
production Runtime builds. It must not be cited as §6.9 production coverage.

## Pass 9 surface audit (`N/A`)

Pass 10 §6.9 requires “any additional final Pass 9 decoder/state-machine surface
required by its accepted implementation contract.”

Pass 9 (`SPEC-009`, release-qualification evidence) owns detach/reconnect/crash
continuity, cleanup/resource return, and native-ready restore. It does **not**
introduce a new byte-oriented wire decoder or parser beyond:

- Pass 5 attachment/reconnect/resync (`reconnect-resync-state-machine`, display
  decode/state);
- Pass 7 protocol decoders (`pass7-protocol-decode`);
- Pass 8 `BlockState` (`block-state-decode`).

Therefore `owner_pass = 9` has no additional fuzz target. The registry records:

```toml
[[surface_decision]]
owner_pass = 9
decision = "N/A"
proof = "docs/engineering/M001-FUZZ-EVIDENCE.md"
```

Native AppKit/Metal presentation boundaries remain integration/perf/security
evidence, not libFuzzer byte-decoder campaigns.

## VT libFuzzer promotion

`vt-byte-parser` and `parser-state-mutation` are production-active and map to
libFuzzer binaries `vt_byte_parser` / `parser_state_mutation`. They are included
in CI/nightly campaigns. Smoke adapters remain the retained-seed path.

## Reconnect campaign honesty

- Smoke adapter: real Runtime attach/resync/detach over UDS (macOS).
- libFuzzer: structural `AttachmentRegistry` + recovery coalescing on all
  platforms; full `ConnectionState` transitions additionally on macOS where the
  production IPC connection module lives.
- Linux no longer silently discards reconnect fuzz input.
