# Pass 10 headed presentation-proxy evidence (#787)

| Field | Value |
|---|---|
| Evidence class | `controlled-host` |
| Head | `a012ab0b71e74a18c37becacb2bfc1c505f1248c` |
| Host | Apple M5 Pro, macOS 26.5.2 (25F84), arm64 |
| Configuration | Release `Seyal.app` (`SEYAL_CODESIGN_IDENTITY=-` local/CI diagnostic packaging) |
| Primary artifacts | `pass10-787-headed-display-link-summary-a012ab0.txt`, `pass10-787-headed-display-link-bench-a012ab0.exit`, `m001-pass10-headed-display-link-a012ab0.env.txt` |
| Samples | **120/120** `CAMetalDisplayLink` presentation-proxy samples |

## Protocol disposition

`docs/engineering/M001-PASS10-VALIDATION.md` §5.1: headed presentation-proxy proof requires `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1` **or an equivalent headed session that produces presentation-proxy samples**. Retained summary shows `display_link_samples=120` with `committed_generation_to_presented_frame_proxy` percentiles. This is **not** Foundation `native-macos-smoke` with `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0`.

GPU and presented-frame values remain proxies; they do not claim physical display scanout latency.
