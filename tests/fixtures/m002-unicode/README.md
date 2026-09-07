# M002 Unicode retained fixture seeds

Issue #815 freezes exact byte/state expectations that must become executable production `TerminalState` fixtures in #816.

These files are deliberately **pre-implementation fixture seeds**. They are not registered in the M001 VT manifest because the current M001 scalar terminal does not implement the M002 Unicode/grapheme/width contract. #816 must consume these exact inputs/expectations in its test-first production harness before implementing the behavior.

Rules:

- `input_hex` is the exact PTY byte stream; no shell/prompt inference is involved.
- coordinates in expected files are zero-based canonical terminal coordinates.
- expected text describes canonical lead text, not renderer glyph output.
- external implementations are behavioral evidence only; no implementation code is copied.
- the expectation may change only through a focused SPEC-011 compatibility refinement with replacement provenance.

Sources used by #815:

- Termux `testWideCharacterWithoutWrapping`: https://github.com/termux/termux-app/blob/3b66f8799635a4dba4a206563048ff0e6792c487/terminal-emulator/src/test/java/com/termux/terminal/UnicodeInputTest.java
- Windows Terminal `_WriteToBuffer`, DECAWM-disabled wide glyph at last column: https://github.com/microsoft/terminal/blob/093e49e29a9f806ff83025c49be5d0c970673b00/src/terminal/adapter/adaptDispatch.cpp

`manifest.toml` records provenance and the required #816 activation.