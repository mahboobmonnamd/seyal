---
name: terminal-conformance
description: Validate Seyal terminal behavior against explicit VT/conformance evidence, retained fixtures and real TUI workloads without importing another terminal engine as production authority.
---

# Terminal conformance

Use this skill when claiming terminal compatibility, closing a VT milestone, fixing a terminal regression, or changing behavior that can affect shells, TUIs, SSH, tmux-as-child, Unicode, scrollback or alternate-screen applications.

Read `AGENTS.md`, `docs/engineering/TESTING.md`, the applicable terminal specification/milestone, and `.agents/skills/vt-tdd/SKILL.md` first.

1. Define the exact behavior under test and its supported/deferred/unsupported status for the active milestone.
2. Record authoritative or reference evidence for control sequences and observable terminal behavior. Reference implementations and conformance suites are evidence, never production dependencies.
3. Add exact byte fixtures and expected canonical terminal-state effects before changing implementation.
4. Exercise split-read/chunk boundaries, parser continuity, mode transitions, cursor/grid effects, damage, main/alternate screen transitions and recovery after unknown/deferred sequences where applicable.
5. Include Unicode/grapheme/width cases when cell placement can change.
6. Run retained external conformance corpora or reference suites where available; record suite/version/provenance and distinguish expected known gaps from regressions.
7. Include realistic workload fixtures or smoke tests for relevant shells/TUIs such as zsh/bash/fish, Vim/Neovim, tmux as a child, htop/watch/ncurses, SSH/nested SSH and high-volume output.
8. A mismatch must become one of: implementation defect, test defect backed by evidence, explicit deferred capability, or documented reference disagreement. Never silently normalize a test to current implementation.
9. Minimize each newly discovered failure into a deterministic regression fixture before fixing it.
10. Run parser/state fuzz regression after conformance changes and link failures to `.agents/skills/rust-fuzzing/SKILL.md`.
11. Capture the conformance delta in the PR: newly passing cases, retained known gaps, regressions prevented, and the exact commands/corpus revisions used.

Do not add Ghostty/libghostty, another emulator, a second terminal state model, or renderer-side terminal semantics to make a conformance test pass.