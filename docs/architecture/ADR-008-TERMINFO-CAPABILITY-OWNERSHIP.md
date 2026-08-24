# ADR-008 — Local terminal capability advertisement and terminfo ownership

- **Status:** Accepted for M001 upon merge
- **Date:** 2026-08-24
- **Issue:** #84
- **Scope:** ownership and honesty of `TERM`/terminfo for local Seyal-created terminal executions

## Context

`seyal-exec` deliberately does not inject `TERM`. That is correct: PTY creation is a kernel/process-lifecycle concern and must not silently claim terminal-emulation capabilities.

M001 nevertheless requires an end-to-end headless Runtime that can be launched from Finder/launchd or another environment where inheriting a useful terminal `TERM` value is not guaranteed. M001 also implements only a deliberately narrow VT subset, so setting `TERM=xterm-256color` would advertise capabilities Seyal has not yet implemented or conformance-tested.

Terminal capability advertisement therefore needs one explicit product/runtime owner and must stay coupled to actual VT evidence.

## Decision

### 1. Runtime/product composition owns capability advertisement

The layer that composes a local Seyal terminal execution chooses the child terminal capability environment.

```text
Runtime/product execution policy
→ selects validated terminal capability profile
→ CommandSpec environment override
→ TerminalExecution / PTY spawn
```

`TerminalEndpoint` and the PTY platform layer remain policy-neutral. They do not invent or rewrite `TERM`, `TERMINFO`, or `TERMINFO_DIRS`.

### 2. M001 uses a milestone-scoped project-owned terminfo entry

The M001 local vertical slice uses:

```text
TERM=seyal-m001
```

with a bundled terminfo database/source that is available to the spawned local shell through the appropriate `TERMINFO`/`TERMINFO_DIRS` lookup configuration.

`seyal-m001` is intentionally not a public compatibility promise. It exists so the M001 shell can ask terminfo for exactly the capabilities Seyal actually implements instead of inheriting an unrelated parent's terminal identity or pretending to be a broader standard terminal.

### 3. Capability advertisement cannot outrun implementation

The bundled M001 entry may advertise only behavior classified `SUPPORTED M001` and backed by the retained terminal fixtures/conformance evidence.

A change that adds a terminfo capability must include or cite the corresponding implemented/tested Seyal terminal behavior. Documentation or terminfo data alone cannot promote a deferred VT feature into supported behavior.

### 4. Do not claim `xterm-256color` in M001

M001 does not set `TERM=xterm-256color` merely because it is widely installed. That entry describes a larger compatibility surface than Seyal currently claims.

Using a standard identity is acceptable only when the implementation intentionally meets the advertised capability contract and regression fixtures prove it.

### 5. Stable public identity is deferred to measured compatibility work

The eventual public Seyal terminal should use a stable project-owned terminfo identity once the emulator is broad enough for real application compatibility. An `xterm-seyal`-style name is a plausible future direction because real applications sometimes special-case the `xterm` family, but the name is **not accepted by this ADR** and must not be shipped until corresponding compatibility evidence exists.

The public decision must cover:

- exact stable `TERM` value;
- packaged/system terminfo distribution;
- fallback rules;
- shell integration;
- `sudo`/environment filtering;
- SSH/nested SSH propagation and remote terminfo installation/fallback;
- compatibility tests for the capabilities the chosen entry advertises.

Those are later terminal-compatibility/product decisions, not M001 scope.

## Why this matches the architecture

- PTY ownership remains unchanged.
- VT capability truth remains anchored in Seyal's own terminal implementation/tests.
- Runtime already owns execution creation policy and is the correct layer to construct the child environment.
- The terminal hot path never consults terminfo; terminfo is startup/application capability metadata.
- A Finder/launchd-started Runtime does not accidentally depend on a parent shell's terminal identity.
- M001 can remain deliberately small without lying to child applications about unsupported capabilities.

## Alternatives rejected for M001

### A. Inherit parent `TERM`

Rejected. A headless Runtime may not have been launched by a terminal, and inherited values can describe another emulator entirely.

### B. Always set `TERM=xterm-256color`

Rejected for M001. Compatibility-by-name is useful only when Seyal actually satisfies the advertised capability set; otherwise applications may emit sequences that the M001 emulator intentionally defers.

### C. Let the PTY layer choose `TERM`

Rejected. PTY creation does not own VT capability policy. Putting product capability claims in `seyal-exec` endpoint/platform code would couple unrelated responsibilities and make future profiles/remote behavior harder to reason about.

### D. Set `TERM=dumb`

Rejected for the M001 vertical slice because it would intentionally disable the color/cursor/alternate-screen behavior the milestone is supposed to prove.

## Current external evidence

Reviewed as product/compatibility evidence, not Seyal authority:

- Ghostty ships a project-owned `xterm-ghostty` terminfo entry and sets that `TERM` when the entry is available; its SSH tooling installs the entry remotely or falls back when necessary: https://ghostty.org/docs/help/terminfo and https://ghostty.org/docs/features/ssh
- WezTerm defaults to `xterm-256color` specifically because it aims to be compatible with that terminfo contract, while also offering a project-owned `wezterm` entry for richer capability advertisement: https://wezterm.org/config/lua/config/term.html
- Kitty similarly uses a project-owned `xterm-kitty` entry and documents remote terminfo installation: https://sw.kovidgoyal.net/kitty/faq/

The lesson is not to copy another terminal's exact name. The lesson is that `TERM` is an explicit capability contract with distribution/remote consequences.

## M001 validation requirements

Before the M001 end-to-end shell gate is accepted:

1. a source-controlled `seyal-m001` terminfo definition exists;
2. build/package tooling makes it resolvable without relying on global machine state;
3. Runtime-created local shells receive `TERM=seyal-m001` and the required lookup path;
4. a clean environment with no inherited `TERM` still launches the shell successfully;
5. `infocmp`/equivalent validation confirms the bundled entry can be loaded;
6. a repository check maps every advertised capability to supported/tested M001 behavior;
7. PTY-only tests continue to prove that `seyal-exec` itself does not inject terminal environment policy.

## Security/privacy

`TERM`/terminfo values are non-secret capability metadata. This ADR does not authorize logging arbitrary child environment variables or weakening the existing redacted `CommandSpec` diagnostics.

## Revisit conditions

Create a new compatibility decision when Seyal is ready to choose a stable public terminal identity, when SSH/remote terminal propagation becomes active scope, or when the implemented VT surface is intentionally compatible with a standard/project terminfo entry broad enough to replace `seyal-m001`.
