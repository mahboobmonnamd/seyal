# Issue #686 R&D — trusted shell-integration boundaries

**Status:** isolated non-mergeable research; no production implementation is proposed by this artifact.
**Revision examined:** `9365fa9c746d0f1eee3889746160b6b20f8b6ce3`
**Scope:** M003 #686 only. This is a decision proposal for an ADR/spec refinement and a later Ready implementation Issue.

## Decision proposal

Treat shell integration as **optional, display-only metadata** from an explicitly enabled,
local, interactive zsh/bash/fish shell. It can make Flow/Blocks and the Pane composer
available only after a versioned handshake proves the exact integration is active. It must
never authorize input, alter PTY/VT/child lifecycle truth, expose secrets, or be inferred
from prompt text.

The selected initial policy is intentionally conservative:

```text
explicitly enabled direct local interactive zsh/bash/fish
  + accepted session/version/capability handshake
  + Runtime state machine accepts exact event sequence
  + no TUI/raw/secret/interactive-child/unknown state
  -> structured composer + metadata-only Block projection

all other states, including missing, stale, malformed or conflicting metadata
  -> same ExecutionId / PTY / TerminalState, Raw direct-terminal input
```

No automatic SSH propagation, remote installation, nested-shell discovery, prompt scraping,
or security/policy decision based on shell metadata is authorized. A command such as `ssh host`
may be one *local* running command Block until the local shell regains its prompt; all input
while it is foreground is Raw. Remote prompt/CWD/command metadata is not represented. A nested
shell/subshell is likewise not separately integrated; only the direct local shell's accepted
outer command boundary can be represented.

This changes the trusted-shell security/protocol boundary (not merely a local implementation
detail). Per `architecture-change`, it requires an Architecture/R&D Issue and accepted ADR/spec
refinement before production code.

## Existing implementation: useful narrow evidence, insufficient authority

The current zsh-only composer path is a limited predecessor, not an implementation of this
proposal:

* `crates/seyal-runtime/src/runtime/shell_integration.rs:171-198` recognises only `/bin/zsh`
  or `zsh`, injects an inline function/precmd hook, and emits OSC 133 C/D records containing a
  random 128-bit token and exit status.
* `crates/seyal-terminal/src/terminal.rs:603-642` bounds the parser candidate to 4 KiB at the
  parser boundary and to a 16-event queue, suppresses alternate-screen events, and exposes only
  typed C/D values rather than arbitrary OSC payload bytes.
* Runtime correlates a C only with a pending composer token and a D only with the active token
  (`shell_integration.rs:299-344`). This is a useful anti-accidental/replay correlation, but is
  not proof that arbitrary terminal OSC is authenticated or that metadata is security truth.
* No CWD event/schema exists: `rg -n -i 'cwd|current.?directory|\\bPWD\\b|working.?directory'`
  over the runtime/terminal/pass7 implementation found no integration contract. The only hit in
  the searched native shell model was a preview-side `FileManager.currentDirectoryPath`.
* `ComposerStatus`/`ComposerEligibility` is defined in `crates/seyal-protocol/src/pass7.rs:20-64`
  but has no Runtime/client/native producer or consumer. Consequently current shell recognition
  is not a negotiated proof of line-oriented prompt eligibility.
* Unsupported composer submission still writes `command + CR` to the PTY before returning
  `Unsupported` (`shell_integration.rs:238-245`). That conflicts with the desired contract:
  an unsupported shell should present Raw direct input, not a composer that silently forwards a
  command while retaining its draft.
* The inline zsh hook has no ownership marker, version handshake, update protocol, or removal.
  It permanently adds `_seyal_block_precmd`/`__seyal_block__` to the live shell for the session.
* It covers no bash/fish behavior, no CWD, no prompt readiness, no SSH/nested-shell policy, no
  secret/interactive-child signal, and no metadata quarantine state.

Therefore ADR-009's required “trusted shell integration capability and lifecycle events” and
SPEC-008's eligibility, quarantine, bounded protocol-capability requirements remain unsatisfied
for the #686 scope.

## Trust and authentication model

Terminal OSC is attacker-controlled terminal output. A syntactically valid OSC 133 event must
first be a *candidate*, not trusted metadata. Do not give it input, process, filesystem,
authorization, secret, policy, or remote authority.

The Runtime owns one bounded `ShellIntegrationSession` per `ExecutionId`:

```text
Disabled
  -> Installing(version, session_id, nonce)   # host sends an explicit activation command
  -> Active(prompt_epoch, capability set)     # only after exact Hello/Ready event
  -> CommandPending(command_id, nonce)
  -> CommandRunning(command_id, start LineId)
  -> Active | Quarantined(reason) | Disabled
```

Acceptance requirements for every candidate event:

1. exact protocol version, event kind and fixed/declared bounded fields;
2. exact Runtime-generated session id + one-time command nonce for the current `ExecutionId`;
3. valid state transition and monotonic prompt/command epoch; no duplicate, reordering, or
   cross-execution token may mutate metadata;
4. a canonical primary `LineId` snapshot only after the parser applied the marker; never a
   screen row or copied terminal text;
5. bounded, validated UTF-8 CWD only if it is explicitly emitted by the accepted local shell
   adapter; it is display metadata, not a filesystem authorization target;
6. no event acceptance while alternate screen is active; malformed, oversized, stale,
   conflicting, or unexpected events increment bounded diagnostics and move the integration to
   Quarantined rather than attempting repair or retry.

The nonce reduces accidental/foreign/replayed marker acceptance, but it cannot make a PTY text
stream a cryptographic security channel. The same local OS user and code executing in the shell
trust domain may emit terminal output. Treat accepted metadata only as shell-adapter attestation
for UI semantics. A future stronger guarantee would need a separately designed, authenticated
local channel and a fresh security/architecture review; it is not implied by OSC.

`Quarantined` immediately disables composer/Flow chrome for that execution, retains canonical
terminal rendering and direct input, discards the pending/active metadata projection, and sends
the client an explicit capability/status downgrade. It does not kill the shell, rewrite history,
scrape a prompt, or retry on a timer. Re-enable only by explicit user action or a new Runtime
incarnation/new direct local shell session, with a fresh handshake.

## zsh, bash and fish adapter shape

Ship separate, versioned adapter assets with a common minimal event schema. Adapters must use
the shell's documented interactive hooks, preserve the status as their first operation, chain
instead of overwrite user hooks, and be reversible without changing user shell semantics.

| Shell | Candidate documented hook surface | Required adapter behavior |
|---|---|---|
| zsh | `preexec`, `precmd`, `chpwd`, `zshexit`; `add-zsh-hook` supports adding/removing hooks | Emit Ready only after installation; C before an accepted composer command; D at the following prompt with captured status; CWD only at a known prompt epoch; remove only adapter-owned hook names. |
| bash | `PS0` after read/before execution and `PROMPT_COMMAND` before primary prompt | Preserve existing scalar/array `PROMPT_COMMAND`; avoid global `DEBUG` trap as a command-boundary source; capture status before any adapter command; handle multiline/secondary prompt without guessing. |
| fish | `fish_preexec`, `fish_postexec`, `fish_prompt` events | Register uniquely named event handlers; save `$status` first; do not persist with `funcsave`; emit command text only for Composer-originated command ids, never raw typed command lines. |

The adapter never writes user command text into OSC. The Runtime already has the composer command
through authenticated local IPC. CWD must be a capped value (recommend `PATH_MAX`/4 KiB maximum,
valid UTF-8, no NUL/control bytes) and must be omitted if it cannot be represented accurately.
Prompt decorations, prompt text and shell history remain out of the protocol.

## Installation, update and removal

1. **Default:** shell integration is disabled; Raw is fully functional. There is no automatic
   mutation of `.zshrc`, `.bashrc`, `config.fish`, remote hosts, `TERM`, or `SSH_*` environment.
2. **Explicit install:** a user-visible local action may write a versioned adapter under a
   Seyal-owned owner-only directory and print the exact one-line source instruction. The user
   opts in by adding that line (or launches a shell with an explicit one-session source command).
   Installer validates shell version, asset manifest/version/hash, file mode and source path.
3. **Update:** write a new version atomically in the owned directory; do not edit arbitrary
   user configuration. Existing sessions retain their reported adapter version until they
   explicitly reload/restart; runtime refuses unknown/mismatched versions and falls back Raw.
4. **Removal:** explicit remove deletes only the owned versioned asset after validating its
   manifest; it tells the user to remove the source line. Existing sessions are disabled by
   capability downgrade or shell exit, never by deleting/overwriting other hooks. A session
   unload function removes exactly the adapter's own hook/function names and restores no guessed
   prior shell state.

## Required failure behavior and tests before implementation

| Case | Required behavior |
|---|---|
| Unsupported/missing shell, uninstalled adapter, version mismatch | Raw direct input; composer not available; no command auto-forward from an unavailable composer. |
| Adapter install/Hello failure, invalid CWD, malformed OSC, nonce/epoch mismatch, queue overflow | Quarantine metadata only; same PTY/VT/process/live display continue; no retry loop. |
| `exec`, `exit`, HUP, shell crash, child final drain before D | Close the command projection as `UnknownCompletion` only after Runtime lifecycle truth, or discard the pending projection; never invent a shell exit status or leave a permanently Current block. |
| TUI, cbreak/raw application, password/auth prompt, REPL/nested shell, background job ambiguity | Composer unavailable; direct terminal route. Existing outer command metadata may remain a running/completed outer boundary only; no child/nested command inference. |
| SSH / mosh / container attach | Local outer command may be a single block; remote session has Raw input and no remote CWD/boundaries. No automatic remote install or protocol forwarding. |
| Client metadata conflict/connection failure | Follow SPEC-007 quarantine discipline: invalidate disposable metadata cache, reconnect without shell capability, preserve Runtime execution. |
| Adapter update/removal | New session handshake required; failed update/removal leaves previous valid asset/session behavior or Raw, never an unverified mixed adapter. |

Required deterministic coverage: parser malformed/truncated/alternate OSC fixtures; Runtime state-machine/property tests for all illegal transitions and bounds; each shell's real PTY tests (normal, failure, multiline, `cd`, subshell, `exec`, `exit`, background, TUI/REPL); local IPC capability downgrade/quarantine tests; native Raw fallback/accessibility/focus tests; no SSH network test until an explicit future remote seam exists. Fish coverage must run on a controlled macOS image with a pinned fish version; its absence on a developer host is a recorded platform limitation, not a pass.

## Reproducible evidence gathered

All commands were run from `/private/tmp/seyal-spike-686` on 2026-09-07 (macOS arm64):

```sh
git status --short --branch
cargo test --locked -p seyal-runtime --lib runtime::shell_integration::composer_wrapper_tests -- --nocapture
cargo test --locked -p seyal-terminal exposes_bounded_trusted_shell_events -- --nocapture
cargo test --locked -p seyal-terminal unbound_or_malformed_markers -- --nocapture
```

Results: the five existing zsh wrapper tests passed; the two parser candidate/malformed-marker
tests passed. The tests establish bounded C/D parsing and zsh `false`/`true` completion only.
They do not establish bash/fish support, prompt eligibility, CWD, install/update/removal,
quarantine, SSH, secret/interactive-child policy, or `exec`/`exit` recovery.

Local tool discovery reported zsh `5.9`, bash `5.3.15`, and no `fish`. A disposable no-profile
zsh hook fixture emitted C+D for `false` and `(exit 3)`, but emitted C without D for `exec` and
`exit`; this directly demonstrates why prompt-return cannot be the sole completion/failure path.

External primary documentation consulted:

* zsh Functions manual: `preexec` runs before execution, `precmd` before prompt, and
  `add-zsh-hook` can add/remove named hooks.
* GNU Bash Interactive Shell Behavior: `PROMPT_COMMAND` executes before primary prompt and PS0
  is expanded after command read/before execution.
* fish language manual: `fish_preexec` and `fish_postexec` bracket interactive commands;
  `fish_prompt` marks prompt display.

## Blockers for independent Sol/Astra review

1. Product/security authority must decide whether OSC-correlated metadata is explicitly
   non-security display metadata, or whether a separate authenticated local control channel is
   required. The latter is a larger ADR/security design.
2. Accept the local-only/remote/nested-shell policy and the `UnknownCompletion` representation;
   current SPEC-008 has only Running/Completed success/failure metadata and no schema for this.
3. Specify a shared, versioned event schema and adapter installation ownership before bash/fish
   code is written. Do not generalize the current zsh string wrapper by copy/paste.
4. Provide a controlled fish-capable macOS test environment and choose supported shell/version
   matrix.
5. Refine SPEC-008 acceptance tests for eligibility signals, capability downgrade/quarantine,
   CWD privacy/validation, no-autoforward Raw fallback, lifecycle completion without D, and
   update/removal.
