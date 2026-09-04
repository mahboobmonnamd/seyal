# Fuzz harnesses

`targets.toml` is the authoritative M001 fuzz-target registry. Issue #11 creates the registry, corpus locations and deterministic smoke validation before production target APIs exist.

A target has one of two states:

- `pending-production-surface`: corpus and ownership are present, but no adapter is executed because the owning implementation does not yet exist;
- `active`: an adapter path must exist and the smoke runner executes every retained corpus seed against it.

Do not create a no-op adapter merely to make a target look active. Activation belongs to the Issue/pass that introduces the real parser/protocol/projection/reconnect API.

## Continuous coverage expectations (Pass 10 honesty)

| Layer | Where it runs | Continuous on every Foundation PR? | Pass 10 evidence role |
|---|---|---|---|
| Registry / corpus / adapter smoke (`scripts/fuzz-smoke.py`) | `Foundation Quality` → `repository-policy` | **Yes** | Proves registry integrity and retained-seed adapters; **not** campaign coverage |
| Short libFuzzer campaigns (`.github/workflows/pass5-fuzz.yml`) | path-filtered / `workflow_dispatch` | **No** — Foundation can be green without this workflow | Targeted PR evidence only; ~30s runs are **not** milestone “fuzz clean” |
| Long-running / expanded-corpus campaigns | controlled host or explicit campaign record | **No** | Required for Pass 10 §6.9 campaign evidence where the criterion demands it |

`fuzz/Cargo.lock` pins the separate fuzz workspace dependencies. Path-filtered CI verifies `cargo metadata --locked` before building. Updating fuzz crate versions requires committing an updated lockfile in the same change.

Fuzz inputs are untrusted and must contain no credentials or private data. Retained crash/regression inputs stay in the target corpus once real targets are active.

Registry-vs-campaign parity gaps (missing Pass 7/9 surfaces, orphan targets, smoke≠libFuzzer semantics) are tracked separately; this document does not claim those gaps are closed.
