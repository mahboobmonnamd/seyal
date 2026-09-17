# Shell-integration hook cost — Tier 1 evidence for #967

Authority: `docs/architecture/ADR-009-COMMAND-BLOCKS-COMPOSER-AND-TUI.md`,
2026-09-16 amendment, mechanisms 7–8. Raw log:
[`m003-shell-integration-967-tier1-4da2354.log`](m003-shell-integration-967-tier1-4da2354.log).

| Field | Value |
|---|---|
| Source head | `4da2354709e7` (branch `issue/967`) |
| Host | Apple M2 Pro, macOS 26.6.2, `/bin/zsh` 5.9 (controlled physical Apple Silicon) |
| Harness | `scripts/bench-shell-integration.py --runs 5 --iterations 200 --ghostty-rev f9a3f24a56bf05f70894e1a084809d4fffadf420` |
| Method | one headless PTY per run; same zsh binary, same isolated `HOME` and `.zshrc` (`PROMPT='PS% '`, one alias), each variant under its own current zsh integration script; nearest-rank percentiles |
| Noise band | max(2 %, 100 µs) per ADR-009 mechanism 8 |
| Performance claim | Tier 1 hook cost only; whole-application (Tier 2) not measured here |

## Variants

- **off** — no integration; own baseline.
- **seyal** — `crates/seyal-runtime/assets/shell-integration/zsh/.zshenv`, nonce delivered over an inherited pipe exactly as `ShellIntegrationPolicy::apply` does.
- **ghostty** — `.zshenv` + `ghostty-integration` fetched at benchmark time from `ghostty-org/ghostty` at `f9a3f24a56bf05f70894e1a084809d4fffadf420` (GPLv3, never vendored; ADR-003). Also covers libghostty-based terminals (cmux).
- **warp** — `bundled/bootstrap/zsh_body.sh` extracted from the installed `/Applications/Warp.app` (v0.2026.08.18.02.52.00) binary and loaded the way Warp loads it: `unsetopt ZLE`, bootstrapped after the user's rc files, Warp session variables set. Prompt-ready signal is Warp's own `Precmd` DCS hook.

## Results (5 runs × 200 iterations)

| Variant | startup p50 / p95 / p99 (µs) | prompt→prompt `true` p50 / p95 / p99 (µs) |
|---|---:|---:|
| off | 24 057 / 27 551 / 27 551 | 119 / 207 / 323 |
| **seyal** | **20 000 / 25 231 / 25 231** | **146 / 245 / 390** |
| ghostty | 19 656 / 40 534 / 40 534 | 254 / 444 / 704 |
| warp | 35 587 / 37 678 / 37 678 | 51 777 / 60 815 / 85 537 |

Startup samples: 5 per variant (one per run). Prompt-to-prompt samples: 1000 per variant.

## Verdicts (ADR-009 mechanism 8, Tier 1)

| Comparison | startup p50 | startup p95 | p→p p50 | p→p p95 | Result |
|---|---|---|---|---|---|
| seyal vs ghostty | 20 000 ≤ 19 656 + 393 | 25 231 ≤ 40 534 + 811 | 146 ≤ 254 + 100 | 245 ≤ 444 + 100 | **PASS** |
| seyal vs warp | 20 000 ≤ 35 587 + 712 | 25 231 ≤ 37 678 + 754 | 146 ≤ 51 777 + 1 036 | 245 ≤ 60 815 + 1 216 | **PASS** |
| seyal vs off (own baseline, mechanism 7 (d)) | −4 056 µs | — | **+27 µs** | +38 µs | within noise band |

The +27 µs per prompt cycle is the cost of three builtin `printf` markers (`A`, `C`, `D`, ≈130 bytes) and is below the 100 µs noise floor. Warp's per-prompt cost is dominated by its `precmd` collecting git/virtualenv/conda/node/kube state through external commands to render Warp's own prompt; that is its design, reported here as its current integration's cost.

## Tier 2 (whole application) — status

`ENVIRONMENT_UNSUPPORTED` on this host for the headed comparison: Ghostty and cmux are not installed, and Warp cannot be driven headlessly from this harness. Per ADR-009 mechanism 8, Tier 2 is reported, not gating; any Tier 2 shortfall is owned by the milestone performance contract. Seyal's own on-vs-off comparison above ((d)) shows no regression beyond noise.

## Reproduce

```sh
python3 scripts/bench-shell-integration.py --runs 5 --iterations 200 \
  --ghostty-rev f9a3f24a56bf05f70894e1a084809d4fffadf420 \
  --out docs/evidence/m003-shell-integration-967-tier1-<head>.log
```

Requires network access for the Ghostty fetch and `/Applications/Warp.app` for the Warp variant; a missing variant is recorded as `ENVIRONMENT_UNSUPPORTED`, never as a pass.
