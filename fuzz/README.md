# Fuzz harnesses

`targets.toml` is the authoritative M001 fuzz-target registry. Issue #11 creates the registry, corpus locations and deterministic smoke validation before production target APIs exist.

A target has one of two states:

- `pending-production-surface`: corpus and ownership are present, but no adapter is executed because the owning implementation does not yet exist;
- `active`: an adapter path must exist and the smoke runner executes every retained corpus seed against it.

Do not create a no-op adapter merely to make a target look active. Activation belongs to the Issue/pass that introduces the real parser/protocol/projection/reconnect API.

Fuzz inputs are untrusted and must contain no credentials or private data. Retained crash/regression inputs stay in the target corpus once real targets are active.
