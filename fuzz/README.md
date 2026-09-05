# Fuzz harnesses

`targets.toml` is the authoritative M001 fuzz-target registry. Issue #11 creates
the registry, corpus locations and deterministic smoke validation before
production target APIs exist.

## Target status

- `pending-production-surface`: corpus and ownership are present, but no adapter
  is executed because the owning implementation does not yet exist.
- `active`: an adapter path must exist, retained seeds must pass the smoke
  adapter, and a `libfuzzer` binary must be declared and present in
  `fuzz/Cargo.toml` so CI/nightly campaigns can run the same target.
- `non-production-comparator`: retained comparator evidence only. It is **not**
  Pass 10 §6.9 production coverage. Candidate-B shared-projection uses this
  status.

Do not create a no-op adapter merely to make a target look active. Activation
belongs to the Issue/pass that introduces the real parser/protocol/projection/
reconnect API.

## Continuous coverage expectations (Pass 10 honesty)

| Layer | Where it runs | Continuous on every Foundation PR? | Pass 10 evidence role |
|---|---|---|---|
| Registry / corpus / adapter smoke (`scripts/fuzz-smoke.py`) | `Foundation Quality` → `repository-policy` | **Yes** | Proves registry integrity and retained-seed adapters; **not** campaign coverage |
| Short libFuzzer campaigns (`.github/workflows/pass5-fuzz.yml`) | path-filtered / `workflow_dispatch` | **No** — Foundation can be green without this workflow | Targeted PR evidence only; ~30s runs are **not** milestone “fuzz clean” |
| Long-running / expanded-corpus campaigns | controlled host or explicit campaign record | **No** | Required for Pass 10 §6.9 campaign evidence where the criterion demands it |

`fuzz/Cargo.lock` pins the separate fuzz workspace dependencies. Path-filtered CI verifies `cargo metadata --locked` before building. Updating fuzz crate versions requires committing an updated lockfile in the same change.

Fuzz inputs are untrusted and must contain no credentials or private data.
Retained crash/regression inputs stay in the target corpus once real targets are
active.

## Campaign parity

`scripts/fuzz-smoke.py` validates:

1. every `active` row has a corpus, adapter, and `libfuzzer` binary mapping;
2. no libFuzzer binary is orphaned from the registry;
3. `surface_decision` rows (Pass 9 N/A) point at an existing proof document;
4. `non-production-comparator` rows never claim a production libFuzzer campaign.

## Evidence grades

See `docs/engineering/M001-FUZZ-EVIDENCE.md`. PR CI (`ci-smoke`, typically 30s)
cannot alone score Pass 10 §6.9 `PASS`. Milestone evidence requires a
`nightly-campaign` / controlled longer campaign with exact-head provenance, or an
explicit `N/A` with architecture proof.
