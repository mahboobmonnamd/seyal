# Pass 10 final freeze — clean-checkout demo (c536c54)

| Field | Value |
|---|---|
| Freeze SHA | `c536c5454583f6a036910e145fe1187446319630` |
| Last production behavior | `a012ab0b71e74a18c37becacb2bfc1c505f1248c` (#789) |
| Host | Mahboob MacBook Pro (2), arm64, macOS 26.5.2 |
| UTC | 2026-09-05 |
| Evidence class | controlled-host |

## Exits (exact freeze clean worktree `/tmp/pass10-final-clean-demo`)

| Step | Exit |
|---|---|
| `make bootstrap` | 0 |
| `make build` | 0 |
| `make test` | 0 |
| `make check` | 0 |
| `make bench` (`SEYAL_CODESIGN_IDENTITY=-`, `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1`) | 0 |
| `--renderer-self-test` | 0 |
| Pass9 budget validator (historical release-qual artifact) | PASS |

## Notable proofs

- Full XCTest + XCUIAutomation passed, including `testPass9ProductionRecoverySurvivesGracefulAndForcedGUIExit` (forced GUI exit).
- Headed DisplayLink: `display_link_samples=120` on freeze tip bench.
- `docs/engineering/ENGINEERING-QUALITY-BASELINE.md` present.
- Machine RSS gate unchanged: `CLIENT_RSS_KIB = 1536`.

Raw logs: `/tmp/pass10-final-freeze/{bootstrap,build,test,check,bench}.{log,exit}`.
