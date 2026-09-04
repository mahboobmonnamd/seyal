# Pass 9 release-qualification findings (Issue #736)

**Status:** `PARTIAL — Refs #736` (not release-qualified; not closing)

**PR relationship:** **`Refs #736`** — do **not** use `Closes #736` until the Issue DoD is fully evidenced.

## What this PR may claim

- Release-qualification **harness** + calibrated production-budget validator path
- Deferred `prepare_cache` / ensure-prepared split (SPEC-aligned)
- Exact-head matrix tooling and Pass 8 attribution collection
- Dead-key / IME through production `NSTextInputClient`
- VoiceOver-**facing** AX field checks only (not system VO focus/announcement/reconnect)
- Honest measurement labeling after harness integrity remediation (see below)
- Reviewable security outcome: `docs/evidence/pass9-security-review-745.md`

## Harness integrity remediation (this tip)

Addressed review blockers without claiming #736 Done:

| Finding | Remediation |
| --- | --- |
| Vacuous `native_ready` | Relabeled: coordinator `reconstructing→usable` only; topology requires `NOT SPEC` native interaction |
| Synthetic resource proxies | `live_handles` / process fd+thread samples / `socket_fd` / renderer flags; allocator fields unused (0) |
| Abrupt baseline after graceful detach | Pre/post-warmup/final detach uses cohort `mode` |
| `prepared_surface` last-frame overwrite | Captures **first** post-ensure Metal update only |
| Merge hard-coded `recovery` | Merge reads/validates `recovery` from partials |
| WouldBlock busy-yield | Exponential sleep backoff (50µs→1ms) |
| File >1000 lines | Models split to `Pass9ReleaseQualificationModels.swift` (~937 + ~131) |
| VO overclaim | Track C claim narrowed in code + orchestrator report |
| Security asserted only | Retained `pass9-security-review-745.md` |
| `Closes #736` dishonest | PR must be **`Refs #736`** |

Dry-run integrity check (`check-pass9-release-smoke.py --integrity-only`) passes on tip after these fixes. That is **not** a full 5×2×2 production-budget PASS.

## Historical note on `21e8e697` evidence

Retained matrix for `21e8e6976c34` remains useful for latency/Pass 8 attribution history, but under the honest fd/thread sampler + non-vacuous baseline rules the production-budget validator **no longer PASSes** that artifact (baselines were synthetic zeros). Do not treat that head as Issue #736 resource-leak completion evidence.

## What remains open on #736

- SPEC native interaction readiness measurement (first-responder/IME/AX after reconnect)
- Real VoiceOver discovery/focus/announcement after reconnect
- Durable Team-identity Release packaging / trust evidence
- Fresh full 5×2×2 exact-head matrix PASS after harness remediation
- Independent non-implementer maintainer approval
- Issue checkbox updates to match verified reality

## Security

See `docs/evidence/pass9-security-review-745.md` (no medium+ findings; packaging Team-identity remains an open process gate).
