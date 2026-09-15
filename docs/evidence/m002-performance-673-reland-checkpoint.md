# #673 performance-contract re-land (Linux)

## Identity

| Item | Value |
| --- | --- |
| Canonical branch | `issue/673` from current `origin/master` (`0f7ad3d`) |
| Relationship | `Refs #673` — do **not** use `Closes #673` |
| Predecessor | PR #843 closed unmerged; remote `issue/673-performance-contract` was deleted |

## What this slice does

Re-apply the versioned M002 performance evidence contract onto current master
without restoring deleted M001.1 product-authority Swift tests.

- `docs/evidence/M002-PERFORMANCE-CONTRACT-V1.md` / `.toml`
- `scripts/check-m002-performance-contract.py`
- validator negative fixtures in `scripts/test-ci-validators.py`
- `make check` / `make bench` / Foundation `repository-policy` invoke the checker

No release gate is claimed. Physical ARM64 / headed measurements remain open.

## What deliberately did not change

- No `SeyalShellUITests.swift` restoration (file deleted on master).
- No `test-macos-skeleton.sh` revival.
- No product/PTY/renderer changes.

## Linux verification

```text
python3 scripts/check-m002-performance-contract.py
python3 scripts/test-ci-validators.py
```
