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

## 2026-09-10 cloud-agent headed attempt

- **Date:** 2026-09-10
- **Host:** Linux cloud agent (`hostname=cursor`, `uname=Linux 6.12.94+ x86_64`)
- **Classification:** `ENVIRONMENT_UNSUPPORTED`
- **Exact code head at attempt:** `16afe73`

A self-hosted macOS worker (`Mahboob's MacBook Pro (2)`,
`workerId=b1146e6c-9dea-5584-a5ef-152e2391887e`) was connected and idle, but
this cloud-agent process could not be routed onto that worker. Subagents
launched for headed GUI work also ran on Linux. No Debug `Seyal.app` was
launched, no Metal pixels were observed, and no checklist step is PASS.

| Manual step | Result |
| --- | --- |
| Thousands of numbered lines; scroll to old output | ENVIRONMENT_UNSUPPORTED |
| Long ASCII + CJK + emoji line; narrow/wide resize | ENVIRONMENT_UNSUPPORTED |
| Hard newline vs autowrap rejoin | ENVIRONMENT_UNSUPPORTED |
| vim/htop alternate-screen exclusion | ENVIRONMENT_UNSUPPORTED |
| Resize while output is arriving | ENVIRONMENT_UNSUPPORTED |

A later computer-use subagent (`bc-b67b1734-5a38-5312-9d06-5cb8eb6c1dd0`)
also ran on Linux (`privateWorkerId=null`) and recorded the same five-step
`ENVIRONMENT_UNSUPPORTED` ledger at `/tmp/m002-819-headed-ledger.md`. It did
not launch `Seyal.app`.

The 2026-09-09 partial AX/scrollbar observation above is unchanged as a
historical ledger. Headed production XCUITests for the five #819 manual steps
were recorded on 2026-09-11 in
[`m002-history-819-headed-macos.md`](m002-history-819-headed-macos.md).
Use that record, not this file, for current headed evidence.

## 2026-09-10 later computer-use abort (after `2d04f6d`)

- **Date:** 2026-09-10
- **Host:** Linux `6.12.94+` x86_64 (same cloud agent; `privateWorkerId=null`)
- **Exact production code head:** `2d04f6d0d0df1d4d83e2e291a98405b63e23b598`
- **Exact branch head at attempt:** `4cb206a`

A further computer-use subagent was launched after the source P1/P2 commit
and also ran on Linux. It stopped after `uname -a` and did not launch
`Seyal.app`. The five-step checklist remains `ENVIRONMENT_UNSUPPORTED`. Raw
note: `/tmp/m002-reviews/headed-macos-819-823.md`.
