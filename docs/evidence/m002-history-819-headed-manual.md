# M002 #819 headed manual evidence

- **Date:** 2026-09-09
- **App:** exact #819 build at `target/macos-derived-data/Build/Products/Debug/Seyal.app`
- **Production boundary:** AppKit shell, Metal terminal surface, separately owned Runtime; no preview fixture
- **Classification:** partial manual observation; not merge acceptance

## Observed workflow

1. Launched the exact #819 native app.
2. The initial surface reported `connection=disconnected`; activated the
   production reconnect control.
3. The same surface then reported `connection=usable` with non-zero Runtime,
   execution, and attachment identities.
4. Entered and executed:

   ```sh
   for i in $(seq 1 120); do printf 'history-%03d 😀 é\\r\\n' $i; done
   ```

5. The production command reached `Completed`; the transcript exposed an AX
   scroll area and scroll bar. Scrolling upward changed the scrollbar value
   from `1` to approximately `0.076`, demonstrating a real headed transcript
   scroll interaction.

## Limitations

- The Metal surface screenshot did not expose verifiable rendered line content
  and the accessibility tree exposed only the surface container, not the
  history rows. Therefore this run does **not** prove displayed history text,
  hard/soft lineage, grapheme atomicity, alternate-screen exclusion, eviction,
  resize reflow, or anchor preservation.
- A narrow-window drag did not produce a verifiable geometry change in the
  available headed session.
- This record is retained to show exactly what was observed and what remains
  unavailable; it must not be used as full headed acceptance evidence.
