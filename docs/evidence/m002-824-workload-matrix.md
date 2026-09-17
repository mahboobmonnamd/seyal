# M002 #824 workload matrix — current-head inventory

| Field | Value |
| --- | --- |
| Owning Issue | #824 (parent #672) |
| Classification | production validation / focused gap closure |
| Exact PR head | `ad50bd193f7f840b34d45c9efa99b9ebfa78c6dc` (code + remediation). Evidence ledger sync may land as a docs-only follow-up commit on `issue/824`. |
| Prior automated pin | `dcbb90faaf870fdabbc543e6947adff2af48c8a3` remains the first retained matrix SHA; intermediate re-runs on `f0e8c01` plus live PTY expansions |
| Host | Darwin arm64 / macOS 26.5.2 / Rust 1.98.0 (local matrix); hosted CI macOS-15 / Xcode 16.4 for exact-head gates |
| Date (UTC) | 2026-09-17 |

This is retained automated + classified native/manual evidence for the #672
workload matrix. It does **not** close #824 or #672. Comparative/release
performance remains #673 authority (`performance_claim=false` here). PTY
smokes spawn with `TERM=seyal-m001` and a `tic`-compiled bundled terminfo
directory, not `xterm-256color`.

**Current-head gates (`ad50bd1`):** hosted Foundation Quality
([run 35242876394](https://github.com/seyal-org/seyal/actions/runs/35242876394))
PASS — `make check` (including `pass7_local_ipc`), component 26/26, XCUI
19 executed / 0 failures / 1 ABC-layout skip, Block selection PASS, workload
4/4 PASS. See [headed ledger](m002-824-headed-manual.md). Historical local
44/45 Block-selection FAIL and pre-isolation `make check` IPC FAIL are retained
as superseded history, not the current claim.

Earlier matrix rows below remain historical workload observations. They do
**not** claim that all ten interactive manual steps have been executed in the
Seyal GUI.

## Matrix

| Workload | Automated evidence | Hosted binaries this run | Headed / manual | Result |
| --- | --- | --- | --- | --- |
| zsh | VT fixture `zsh_bash_fish_prompt_color_clear_and_multiline_stay_one_authority`; PTY `zsh_bash_and_optional_fish_print_through_one_pty_vt` | `/bin/zsh` present | Interactive editing/history/resize: headed ledger | Automated PASS |
| bash | same VT + PTY | `/bin/bash` present | same | Automated PASS |
| fish | same VT; PTY when `fish` exists | `fish` present | same | Automated PASS |
| SSH / nested SSH | VT `ssh_and_nested_ssh_keep_a_single_terminal_state`. Live PTY `live_ssh_and_nested_ssh_use_one_pty_vt_when_orbstack_present`: `ssh(1)` to OrbStack `nt-ssh@orb`, remote `vim -c qa`, nested hop into ephemeral Docker sshd | `ssh` present; OrbStack `nt-ssh@orb` reachable; remote `/usr/bin/vim` present | Interactive remote + nested SSH in the Seyal GUI | Automated PASS for live `ssh(1)` + remote vim + nested hop on one Seyal PTY; headed GUI still blocked (exclusive Runtime occupied) |
| Vim / Neovim | VT `vim_neovim_alternate_screen_unicode_mouse_and_keys_restore_primary`; live PTY `live_vim_and_neovim_restore_primary_after_alternate_screen` (`:qa!` restores primary) | `vim` and `nvim` present | Insert/search/splits/mouse/keys in GUI | Automated PASS for live alt-screen enter/restore on one PTY; interactive TUI headed/manual still blocked |
| tmux-as-child | VT `tmux_as_child_is_vt_bytes_not_seyal_panes`; PTY `tmux_as_child_owns_one_pty_when_present`; live split `live_tmux_split_stays_one_seyal_pty` | `tmux` present | windows/panes/copy-mode/detach inside tmux in GUI | Automated PASS: one Seyal PTY/child including split-window; tmux hierarchy is not Seyal panes |
| htop / watch / ncurses | VT `htop_watch_ncurses_restore_primary_after_alt_screen`; live PTY `live_htop_and_watch_restore_primary_after_ncurses` | `watch` and `htop` 3.5.3 present | keyboard+mouse+alt-screen return in GUI | Automated PASS for live htop/watch alt-screen restore |
| git | VT ANSI/progress fixture; PTY `git --version`; live `git log --oneline -n 5` color through `TerminalState` | `git` present | long/color `git log`/`status` in GUI | Automated PASS including color log |
| Docker | VT ANSI fixture; PTY `--version`; live `docker ps` stays one `TerminalExecution` | `docker` present | build/progress TTY in GUI | Automated PASS for version/ps; interactive build headed/manual |
| kubectl | VT ANSI fixture; PTY `kubectl version --client` | `kubectl` present | live cluster | Automated PASS for client/ANSI; cluster not exercised |
| terraform | VT ANSI `Plan:` fixture; PTY `terraform version` | `terraform` v1.9.8 present (`/opt/homebrew/bin/terraform`) | plan/apply TTY in GUI | Automated PASS for binary version + VT Plan fixture |
| high-volume logs | VT 400-line bounded searchable history; PTY 200-line `hv-done` | n/a | type/scroll while flooding | Automated PASS; headed Flow/Blocks XCUI PASS (prior exclusive-Runtime session) |
| CLI-agent TUI | VT `cli_agent_tui_equivalent_negotiates_supported_subset_only` (alt-screen, kitty 1\|2, SGR mouse, title) | n/a | real Claude Code / Codex | Deterministic equivalent PASS; live agent TUI headed/manual not run |
| alt-screen + Unicode + history/reflow + selection + mouse + keyboard | VT `alt_screen_unicode_history_selection_mouse_and_keyboard_stay_coherent` | n/a | headed search/copy after resize | Automated PASS; headed Flow/Blocks Unicode+resize XCUI PASS (prior session) |
| terminfo honesty | `terminfo_truth` 3/3 | n/a | n/a | PASS; sixel/Ms/Smolx/rmolx/kitty/RGB remain unadvertised |
| parser/state/protocol fuzz | `python3 scripts/fuzz-smoke.py` — 10 active targets, all retained seeds passed | n/a | long campaign | Smoke PASS; not milestone-length campaign |
| OSC/input/mouse/query security | VT `hostile_osc_query_paste_and_mouse_stay_bounded_and_non_executing` + `docs/evidence/m002-824-security-review.md` | n/a | n/a | Automated PASS |
| M001 detach/reconnect | existing `pass8_resync_reattach` / `macos_runtime` detach tests; XCUI `testGuiRelaunchReconnectsWithoutKillingExecution` | Runtime singleton | GUI close/reopen | Automated Runtime PASS (12 tests); headed XCUI PASS (prior exclusive-Runtime session) |
| SPEC-011 IME 37–41 | Eight native callback cases plus real ABC system-input-source XCUI, including exact PTY byte capture, candidate cursor geometry and AppKit detach callbacks | native macOS / real PTY | Physical hardware, multilingual candidate-popup and mixed-display-scale behavior not exercised | Native callbacks and system dead-key commit/cancel PASS within the explicit boundaries in the headed ledger |

## Commands run (this session, `f0e8c01` + uncommitted PTY/IME)

```sh
cargo test --locked -p seyal-terminal --test m002_workload_matrix
# 10 passed

cargo test --locked -p seyal-exec --test m002_workload_pty
# 10 passed (zsh/bash/fish, live ssh+vim+nested, vim/nvim alt-screen,
# tmux child + split, htop/watch, git/docker/kubectl/terraform, high-volume)

cargo test --locked -p seyal-terminal \
  --test m002_vt_breadth --test m002_unicode --test m002_selection \
  --test m002_mouse --test m002_keyboard --test terminfo_truth \
  --test history_store_regressions --test fuzz_smoke
# executed tests passed (fuzz_smoke ignored by design; smoke via scripts/fuzz-smoke.py)

cargo test --locked -p seyal-runtime --test pass8_resync_reattach --test macos_runtime
# 1 + 11 passed (isolated runtime dirs; local IPC disabled)

python3 scripts/fuzz-smoke.py
# 10 active targets passed; campaign parity ok
```

`scripts/test-macos-ui.sh` was **not** re-run in the pre-remediation local
session when exclusive `seyal-runtime` pid 99338 owned `control.sock`. Exact-head
hosted `make test` on `ad50bd1` later executed the full native XCUI suite
(Block selection PASS; workload 4/4 PASS; one ABC layout skip). Prior exclusive-
Runtime local session: 18/18 PASS.

## Gaps that are not buried

### Closure follow-up validation (2026-09-17)

These runs preceded committing the patch on `f0e8c01`; they are not a clean
milestone qualification freeze:

- The eight terminal suites below executed **83 tests: 83 passed, 0 failed,
  0 ignored, 0 filtered**:

  ```sh
  cargo test --locked -p seyal-terminal \
    --test m002_workload_matrix --test m002_vt_breadth --test m002_unicode \
    --test m002_selection --test m002_mouse --test m002_keyboard \
    --test terminfo_truth --test history_store_regressions
  ```

- `cargo test --locked -p seyal-exec --test m002_workload_pty --no-run`
  passed. This verifies compilation, not another execution of the live PTY rows.
- The subsequent `make check` rerun executed this PTY suite: **10/10 passed**,
  including live SSH/nested SSH, Vim/Neovim, tmux and htop/watch. A later local
  Runtime IPC failure existed at that time; it is **superseded** by harness
  isolation on `ad50bd1` (hosted `make check` PASS).
- ARM64 `xcodebuild build-for-testing` passed after the Block selection test
  was given a per-run unique command label. This verifies compilation, not
  execution of the corrected XCUI test.
- The native IME evidence descriptions now explicitly distinguish document
  invariants from the still-INCONCLUSIVE native fixture outcomes.
- Runtime PID 99338 still owned `control.sock` at the safety check. No headed
  test was launched and no existing Runtime was stopped by this follow-up.

### Remaining acceptance

No new module/ADR was required. Remaining Done gates are evidence, not architecture:

1. Interactive SSH/nested SSH **in the Seyal GUI** (PTY live hop is retained; many headed interactive rows remain ENVIRONMENT_UNSUPPORTED / not fully exercised as manual steps).
2. Interactive Vim/Neovim/tmux-inside-tmux/htop/agent TUI on Flow → TUI takeover → Flow in the GUI (live PTY alt-screen restore is retained).
3. SPEC-011 IME 37–41 have native callback/PTY evidence in the headed ledger. Hosted CI skips the ABC system-input-source case; local ABC PASS is retained. Physical input-source/candidate-popup, mixed-display-scale and live-composition reconnect breadth remain unclaimed. #836 not pulled.
4. #673 PHYSICAL_ARM64 five-cohort / 20-warmup / 100-sample matrix — **not started**.
5. Independent close review after evidence/ledger honesty is current. This PR stays `Refs #824`.

Active remediation for Block selection and `pass7_local_ipc` isolation is
**complete on `ad50bd1`** (see [remediation](m002-824-remediation.md)); those
are no longer open product failures on the exact head.

## What this PR did not change

No VT/renderer/runtime replacement. No terminfo weakening. No M003 chrome.
No ADR create/amend. `TerminalState` remains sole terminal authority.
