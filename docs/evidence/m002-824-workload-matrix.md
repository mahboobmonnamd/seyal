# M002 #824 workload matrix — current-head inventory

| Field | Value |
| --- | --- |
| Owning Issue | #824 (parent #672) |
| Classification | production validation / focused gap closure |
| Exact head at this record | `dcbb90faaf870fdabbc543e6947adff2af48c8a3` (`issue/824` from `master` `4121c12`). This docs-only follow-up does not re-run that matrix. |
| Host | Darwin arm64 / macOS 26.5 / Rust 1.98.0 |
| Date (UTC) | 2026-09-16 |

This is retained automated + classified native/manual evidence for the #672
workload matrix. It does **not** close #824 or #672. Comparative/release
performance remains #673 authority (`performance_claim=false` here). PTY
smokes spawn with `TERM=seyal-m001` and a `tic`-compiled bundled terminfo
directory, not `xterm-256color`.

## Matrix

| Workload | Automated evidence | Hosted binaries this run | Headed / manual | Result |
| --- | --- | --- | --- | --- |
| zsh | VT fixture `zsh_bash_fish_prompt_color_clear_and_multiline_stay_one_authority`; PTY `zsh_bash_and_optional_fish_print_through_one_pty_vt` | `/bin/zsh` present | Interactive editing/history/resize: headed ledger | Automated PASS |
| bash | same VT + PTY | `/bin/bash` present | same | Automated PASS |
| fish | same VT; PTY when `fish` exists | `fish` present | same | Automated PASS |
| SSH / nested SSH | VT `ssh_and_nested_ssh_keep_a_single_terminal_state` — DA + stacked OSC titles on one `TerminalState`, **not** a live `ssh(1)` child | `ssh` present; no dedicated remote host | Interactive remote + nested SSH | Automated PASS for single-authority stand-in; live `ssh` `ENVIRONMENT_UNSUPPORTED` |
| Vim / Neovim | VT `vim_neovim_alternate_screen_unicode_mouse_and_keys_restore_primary` | `vim` and `nvim` present | Insert/search/splits/mouse/keys | Automated PASS for alt-screen restore; interactive TUI headed/manual |
| tmux-as-child | VT `tmux_as_child_is_vt_bytes_not_seyal_panes`; PTY `tmux_as_child_owns_one_pty_when_present` | `tmux` present | windows/panes/copy-mode/detach inside tmux | Automated PASS: one Seyal PTY/child; tmux hierarchy is not Seyal panes |
| htop / watch / ncurses | VT `htop_watch_ncurses_restore_primary_after_alt_screen` | `watch` present; `htop` **absent** | keyboard+mouse+alt-screen return | Automated PASS for sequences; `htop` `PLATFORM_LIMITED` |
| git | VT ANSI/progress fixture; PTY `git --version` through `TerminalState` | `git` present | long/color `git log`/`status` | Automated PASS |
| Docker | VT ANSI fixture; PTY `--version` | `docker` present | build/progress TTY | Automated PASS for version/ANSI; interactive build headed/manual |
| kubectl | VT ANSI fixture; PTY client version | `kubectl` present | live cluster | Automated PASS for client/ANSI; cluster `ENVIRONMENT_UNSUPPORTED` |
| terraform | VT ANSI `Plan:` fixture | **absent** | plan/apply TTY | VT PASS; binary `PLATFORM_LIMITED` |
| high-volume logs | VT 400-line bounded searchable history; PTY 200-line `hv-done` | n/a | type/scroll while flooding | Automated PASS |
| CLI-agent TUI | VT `cli_agent_tui_equivalent_negotiates_supported_subset_only` (alt-screen, kitty 1\|2, SGR mouse, title) | n/a | real Claude Code / Codex | Deterministic equivalent PASS; live agent TUI headed/manual |
| alt-screen + Unicode + history/reflow + selection + mouse + keyboard | VT `alt_screen_unicode_history_selection_mouse_and_keyboard_stay_coherent` | n/a | headed search/copy after resize | Automated PASS |
| terminfo honesty | `terminfo_truth` 3/3 | n/a | n/a | PASS; sixel/Ms/Smolx/rmolx/kitty/RGB remain unadvertised |
| parser/state/protocol fuzz | `python3 scripts/fuzz-smoke.py` — 10 active targets, all retained seeds passed | n/a | long campaign | Smoke PASS; not milestone-length campaign |
| OSC/input/mouse/query security | VT `hostile_osc_query_paste_and_mouse_stay_bounded_and_non_executing` + `docs/evidence/m002-824-security-review.md` | n/a | n/a | Automated PASS |
| M001 detach/reconnect | existing `pass8_resync_reattach` / `macos_runtime` detach tests; XCUI `testGuiRelaunchReconnectsWithoutKillingExecution` | Runtime singleton | GUI close/reopen | Automated Runtime PASS; headed XCUI this session **INCONCLUSIVE** (foreign `control.sock`) |

## Commands run (this session)

```sh
cargo test --locked -p seyal-terminal --test m002_workload_matrix
# 10 passed

cargo test --locked -p seyal-exec --test m002_workload_pty
# 5 passed; terraform PLATFORM_LIMITED

cargo test --locked -p seyal-terminal \
  --test m002_vt_breadth --test m002_unicode --test m002_selection \
  --test m002_mouse --test m002_keyboard --test terminfo_truth \
  --test history_store_regressions --test fuzz_smoke
# all executed tests passed (fuzz_smoke ignored by design; smoke via scripts/fuzz-smoke.py)

python3 scripts/fuzz-smoke.py
# 10 active targets passed; campaign parity ok
```

`make check` passed on this working tree after the tmux-server cleanup (workspace
fmt/clippy/tests/failure-matrix). Headed `scripts/test-macos-ui.sh` was **not**
run: exclusive Runtime `control.sock` was occupied.

## Gaps that are not buried

No new module/ADR was required. Remaining Done gates are evidence, not architecture:

1. Headed Flow/Blocks XCUI under **exclusive** Runtime (`control.sock` was occupied by pid 45106 from another checkout this session).
2. Interactive SSH/nested SSH against a real host.
3. Interactive Vim/Neovim/tmux-inside-tmux/htop/agent TUI on Flow → TUI takeover → Flow.
4. `terraform` binary and `htop` binary on this host (`PLATFORM_LIMITED`).
5. #673 PHYSICAL_ARM64 five-cohort / 20-warmup / 100-sample matrix — **not started**.
6. Independent review.

## What this PR did not change

No VT/renderer/runtime replacement. No terminfo weakening. No M003 chrome.
No ADR create/amend. `TerminalState` remains sole terminal authority.
