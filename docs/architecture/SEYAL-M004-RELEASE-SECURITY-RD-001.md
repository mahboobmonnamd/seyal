# M004 Release Security Research — macOS distribution, update and recovery

- **Status:** R&D decision proposal; non-mergeable evidence for Issue #688
- **Date:** 2026-09-07
- **Issue:** [#688](https://github.com/mahboobmonnamd/seyal/issues/688)
- **Scope:** direct macOS distribution at M004, including the signed `Seyal.app`,
  its bundled `seyal-runtime` helper, update metadata, staging, recovery and
  release verification. This does not implement a distributor, updater, new
  persistence store, privileged helper, or a commercial service.
- **Platform evidence:** macOS 26.5.2 (25F84), Xcode 26.6 (17F113), SDK 26.5;
  the current target deploys to macOS 14.0. Apple API/process facts below are
  independently recheckable against the linked first-party documentation.

## Decision proposal

Adopt **direct Developer ID distribution of a notarized, stapled signed DMG**
and a **pinned Sparkle 2 update component** for the M004 update client. Sparkle
is an OSS dependency, not a hosted or commercial dependency. It must use its
Ed25519 update-archive signature and signed-feed verification in addition to
normal HTTPS and Apple code-signature verification. The stable channel must
not automatically downgrade. Update installation replaces only the application
bundle through the framework's atomic-safe mechanism; it must never write,
delete, or migrate Runtime-owned or durable user data.

This is an architecture/security decision, not an implementation authorization.
Before an ADR may be accepted, #687 must resolve and test the compatibility
contract in [Shared #687 contract](#shared-687-contract-before-adr-acceptance).
The accepted ADR must be a separate Architecture/R&D PR; its consumer #677
implementation must be separate.

### Required release shape

```text
release build inputs + source/SBOM/provenance
  -> Developer ID Application-sign each executable, framework and app bundle
  -> validate strict nested signatures and entitlement allow-list
  -> create and sign a Seyal.app-only DMG
  -> submit with notarytool; retain submission ID and notary log
  -> staple and validate the ticket on the DMG and app
  -> publish immutable archive + checksum + signed Sparkle feed over HTTPS

installed Seyal.app
  -> embedded Sparkle public update key + signed-feed requirement
  -> fetch channel feed (untrusted until verified)
  -> validate feed, archive size/hash/Ed25519 signature, then extracted app
     identity / Developer ID / Team ID / hardened runtime
  -> atomic app-bundle replacement only after explicit UX-safe point
  -> relaunch GUI; it attaches only after Runtime/protocol compatibility check
```

The initial DMG contains one app bundle and installs into a user-writable
location. M004 must not install a privileged helper, launch daemon, system
extension, login item, or post-install script. Therefore no authorization
prompt is a normal update requirement. If a later product genuinely needs a
privileged component, it requires a fresh security architecture decision and a
separately signed/verified installer path; it is out of #688.

## Why this choice

Apple requires Developer ID signatures, hardened runtime, secure timestamps
and notarization for direct distribution; `notarytool` is the current command
line route, while `altool` is no longer accepted. A ticket may be stapled and
is also published for Gatekeeper lookup. A signable DMG protects all its
included content from post-sign modification, whereas a ZIP itself cannot be
signed. These facts make a notarized/stapled DMG the appropriate clean-machine
installer and recovery artifact.

Sparkle 2 is the smallest mature open implementation that already supplies the
required macOS-specific update lifecycle: Apple code-signing plus EdDSA update
verification, APFS-oriented atomic-safe installs, channels and rollback-safe
failure behavior. Its signed-feed option and pre-extraction verification are
mandatory here: HTTPS authenticates a transport connection, but cannot by
itself make a compromised feed origin trustworthy. A custom feed/replacer
would need to reproduce signed metadata, key rotation, replay/downgrade
resistance, extraction validation, atomic install/relaunch and recovery before
it could meet the launch gate; no evidence supports taking that risk for M004.

### Alternatives compared

| Alternative | Benefits | Blocking concern / decision |
|---|---|---|
| Sparkle 2, signed feed and Ed25519 archives | Mature OSS macOS installer/relaunch path; Apple code-signature cross-check; atomic-safe installation; no Seyal update service | **Select, conditional on dependency, license, SPM pin/provenance, API and adversarial-harness review.** Do not enable deltas until full-archive behavior is green. |
| Custom signed manifest + staged replacement | Maximum protocol/control freedom; can later adopt TUF-like threshold/offline-root design | **Reject for M004.** Seyal would own a new high-risk updater security boundary and must prove all of the above properties. Reconsider only if Sparkle cannot meet the resolved #687 compatibility contract. |
| Package-manager/manual-only | Lowest in-app code and works for Homebrew/power users | **Reject as the only M004 path.** It does not provide the required public signed-update/recovery UX for direct users. Keep a signed/notarized DMG and checksum as a documented manual recovery/offline path; package-manager formulae are optional, independently maintained channels. |

## Trust, signing and release policy

1. The release automation obtains a **Developer ID Application** identity only
   from an approved protected signing environment. It signs every nested Mach-O
   component before the outer app, includes a secure timestamp, enables hardened
   runtime and rejects unexpected entitlements. Test/ad-hoc/local-development
   identities are structurally useful but are never distribution evidence.
2. The app and bundled helper have fixed identifiers and the same Developer
   Team ID. The existing helper validation already checks strict code validity,
   helper identifier and same-Team requirement; release packaging must preserve
   that invariant after final signing and after extraction from the update
   archive.
3. An offline/strongly access-controlled Ed25519 update signing key has its
   **public** half embedded in the released app. It is distinct from the Apple
   signing identity. The private half is never in the repository, app bundle,
   CI logs, appcast, shell history, fixtures or issue text. Require signed
   feeds and verification before extraction; release notes must be treated as
   untrusted unless covered by the signed-feed mode.
4. Production, beta and nightly channels have distinct feed endpoints and
   signing/release controls. Production clients never follow a beta/nightly
   feed merely because it has a higher version. The URL/channel is a signed
   release configuration, not mutable user data. Channel switching requires an
   explicit user choice plus restart and is recorded locally without secrets.
5. Archive locations are immutable/versioned. Publish the DMG, its SHA-256,
   notarization submission ID/status, stapler validation result, signing
   identity subject/Team ID (not private material), source SHA, build profile,
   dependency/SBOM/provenance digest and exact app version in a release record.
6. `CFBundleVersion` is a monotonic, non-reused update ordering value and
   `CFBundleShortVersionString` is the user-visible version. Stable clients
   reject lower/equal versions by default. Emergency rollback is an explicit,
   user-confirmed recovery action using a retained prior signed/notarized DMG;
   it is never an invisible server-directed downgrade.
7. A release has two independent approvals: code-release approval and update
   publication approval. Key compromise, Apple certificate revocation,
   unsigned artifact, failed notarization, failed staple, unexpected
   entitlements, or a release-record mismatch fails closed and blocks publish.

## Update lifecycle and failure semantics

1. On launch or an appropriately rate-limited background check, the update
   client fetches the selected HTTPS feed. Network, TLS, DNS, feed parsing,
   signature, expiration/version/order, size, hash or archive-signature
   failures leave the running app untouched, show a bounded actionable status,
   and back off. They never retry on the terminal hot path.
2. The client downloads to a private, bounded staging directory on the same
   volume when possible. It validates the signed feed and archive before
   extraction; it validates the extracted app's nested code signatures,
   expected bundle identifier, Team ID, hardened runtime and helper identity
   before any replacement. Path traversal, symlinks, duplicate bundle members,
   unexpected nested executable/bundle types, resource exhaustion and partial
   writes are failures with cleanup, not installer input.
3. No GUI process claims it updated a live Runtime. A GUI update may quit and
   relaunch while an old bundled Runtime process remains alive; that Runtime
   retains its own executable mapping and terminal/PTY authority. The new GUI
   first performs the negotiated compatibility check. It attaches only when
   compatible; it does not kill/restart/replace the live Runtime, replay PTY
   bytes, reconstruct canonical state or imply that the helper itself changed.
4. If the new GUI and live Runtime are incompatible, the UI presents an honest
   blocked/recovery state: keep/reattach with a compatible GUI where possible,
   wait for all executions to finish, or have the user explicitly terminate
   them before restarting the Runtime. There is no forced process kill,
   automatic data migration, hidden compatibility shim or false survival claim.
5. The updater's app-bundle backup is only an installation rollback aid. It is
   not a database rollback and must not be used to bypass persistent-schema
   compatibility requirements. A failed replacement/relaunch preserves the
   previous valid app and all user data; documented manual recovery is to
   install the exact retained prior signed/notarized DMG, then follow the #687
   schema recovery procedure.

## Shared #687 contract before ADR acceptance

Issue #687 owns durable metadata/history/layout transaction and recovery
semantics; #688 owns application distribution/update trust. Neither can define
the other by implication. The final ADR must link a jointly accepted,
versioned **Executable–GUI–Runtime–Persistence Compatibility Contract** with
at least these fields and rules:

| Contract element | Required rule |
|---|---|
| Release identity | `release_id`, source SHA, `CFBundleVersion`, build profile, app bundle ID, Developer Team ID, Runtime helper build ID and update channel are observable but contain no secrets. |
| GUI↔Runtime protocol | Handshake carries protocol major/minor plus minimum/maximum peer versions and feature capabilities. Major mismatch blocks attach; a negotiated minor/feature subset is allowed only when specified and tested. No implicit bootstrap/replay/fallback creates a second state. |
| Runtime executable lifetime | Existing Runtime records its own build/protocol ID. Updating `Seyal.app` never replaces an executing helper. A new GUI may attach only after the handshake; Runtime replacement waits for a state defined by #687 that cannot sever a live PTY. |
| Durable schema | Every durable store/journal/snapshot carries `format_id`, schema version, writer version, minimum reader and minimum writer. The Runtime/store is the authority for migration ownership and atomic commit/recovery; the GUI/updater never mutates it directly. |
| Compatibility grid | For each released GUI/Runtime/schema triple, publish: attach allowed, read allowed, write allowed, migration permitted, rollback permitted, and user-visible recovery action. Unsupported triples fail closed without destructive migration. |
| Migration and rollback | Forward migrations are crash-safe, idempotent/recoverable and only run at #687's declared owner/quiescence boundary. A newer schema cannot be opened by an older executable unless an explicit backward-compatible reader is tested. App rollback never promises data rollback. |
| Retention/privacy | Update diagnostics and release records never collect terminal content, shell environment, history or user paths by default. #687's bounded/redacted data rules apply through failed updates and manual recovery. |
| External/manual install | A manually installed valid app must execute the same compatibility check and cannot bypass update ordering, schema gates or runtime ownership merely because it did not arrive through Sparkle. |

This contract is the current blocker: #687 is an active R&D issue with no
accepted storage/migration model. The selected update mechanism is a proposal,
not a final ADR, until the grid and crash/rollback evidence are available.

## Security threat review

| Asset | Threat / entry point | Required mitigation and evidence |
|---|---|---|
| App, helper and user trust | Tampered DMG/archive; compromised CDN/feed/TLS endpoint | Developer ID + hardened runtime + notarization/staple; Ed25519 archive and signed feed; expected identifier/Team checks; immutable archive and release record. |
| Update signing authority | Stolen update key; mistaken channel publication | Separate protected key and publication approval; embedded public key; key-rotation drill; immediate channel freeze/revoke procedure; never expose key material. |
| Runtime/PTY/canonical state | GUI replacement treats old Runtime as updated or kills it | Runtime is independent authority; version handshake; no automatic restart/kill/replay; explicit recovery UX and #687 grid. |
| Durable workspace/history | App or schema rollback corrupts/misreads user state | Updater does not touch data; #687 migration transaction/recovery owns writes; pre/post migration and downgrade tests. |
| Availability/disk | Huge, partial, corrupt or malicious archive; retry storm | Signed length/digest, download/stage quotas, path validation, bounded backoff, cleanup and no hot-path network work. |
| Privacy | Update telemetry/logs disclose terminal content or paths | Offline-capable behavior; local bounded error codes; redact URL/query/user paths; opt-in telemetry only after a separate decision. |
| Privilege | Installer/updater escalates or installs persistent privileged code | App-only user-space install at M004; no privileged helper or scripts. |

## Adversarial update and rollback matrix

All rows run against a signed RC candidate on a clean macOS 14+ account and
record source SHA, app/runtime/schema versions, channel, artifact digest,
Gatekeeper result, logs with secrets/content redacted, and before/after hashes
of app and fixture data. `PASS` means the stated observation; no row is closed
by a unit test alone.

| ID | Injection / setup | Required result |
|---|---|---|
| U01 | Fresh downloaded stapled DMG, online | Gatekeeper accepts; app/helper strict signatures and Team IDs verify; clean install launches. |
| U02 | Fresh downloaded stapled DMG, offline | Stapled ticket validates and install behavior is documented; no account/network dependency after download. |
| U03 | Flip one byte in DMG | Signature/notarization assessment fails; app is not installed/launched. |
| U04 | Replace nested helper or alter entitlement | Nested signature/Team/entitlement validation fails before publish and before update install. |
| U05 | Test/ad-hoc signed build | May prove local harness wiring only; cannot pass distribution/Gatekeeper/notarization acceptance. Release record labels it non-production. |
| U06 | Notary rejection/warning or stapler failure | Publication blocks; retain log/status, do not substitute a local `codesign` success. |
| U07 | DNS/TLS failure, captive portal, HTTP redirect | No install/state mutation; bounded retry/backoff and actionable offline status. |
| U08 | Feed signature invalid/expired/wrong channel | No download/install; no release notes/links are trusted; status is bounded. |
| U09 | Signed feed replays lower/equal stable version | Automatic update rejects it; no silent downgrade. |
| U10 | Archive signature/hash/length wrong | Delete/quarantine stage; old app/data/Runtime remain usable. |
| U11 | Archive contains traversal path, symlink escape, duplicate app or unexpected executable | Extraction/install rejects it without writing outside staging/app replacement scope. |
| U12 | Disk full, permission denied, read-only volume | Transaction fails recoverably; prior app is launchable and data untouched. |
| U13 | Process kill/power loss during download | Incomplete stage is not chosen on next launch; old app/data survive; bounded cleanup. |
| U14 | Process kill/power loss during extract/swap/relaunch | Atomic-safe outcome: either old valid app or new valid app, never a half-valid selected app; data untouched. |
| U15 | New app crashes before first usable window | Offer deterministic relaunch/recovery; retain usable prior app; never loop/retry indefinitely. |
| U16 | Current GUI has live PTY/Runtime during GUI update | GUI exits/relaunches without claiming Runtime update; exact same Runtime/Execution ID attaches only when grid permits. |
| U17 | New GUI protocol-major mismatch with live Runtime | Attach blocked without killing/replaying; honest options only; PTY continues if alive. |
| U18 | Minor/capability-compatible GUI–Runtime pair | Negotiated feature subset attaches; controller/input authority and authoritative snapshot rules still pass. |
| U19 | Runtime process crashes/reboots before/during update | No false execution resurrection; #687 recovery state and update state agree. |
| U20 | New app meets protocol but needs newer persistent schema | No automatic migration until #687 ownership/quiescence rule; prompt/recovery describes consequence. |
| U21 | Crash during schema migration paired with app update | #687 journal/snapshot recovery reaches a declared valid state; neither updater backup nor old app bypasses it. |
| U22 | User manually reinstalls previous signed/notarized DMG after newer schema | App verifies; compatibility grid either permits read/attach or blocks safely with documented recovery; no corruption. |
| U23 | Key rotation / Developer ID certificate rotation drill | Existing client accepts only a transition authorized by its currently trusted signing path; old key/cert alone cannot redirect updates. |
| U24 | High-latency/slow network plus terminal high output | Update work is bounded/background; PTY → VT → state → damage latency/queues remain within #673 budgets. |
| U25 | Update check disabled/offline for extended period | App remains fully local and functional; no expiry bricks terminal; next check validates ordering/feed normally. |
| U26 | Rollback after discovered bad release | User-confirmed retained artifact reinstalls; release record identifies version; Runtime/schema grid governs attach, not marketing version alone. |
| U27 | Diagnostics after every failure class | Contains release IDs/error codes only; no shell output, environment, history, private filesystem paths or credential material. |

## Manual release verification matrix

The release owner must retain the exact output rather than tick boxes from a
different head. Commands below are a contract, not commands run with any
identity in this spike.

| Gate | Evidence required |
|---|---|
| Build provenance | Exact source SHA, clean checkout status, Xcode/SDK/Rust versions, architecture, build script digest, dependency lock/SBOM. |
| Signing | `codesign --verify --strict --deep` is not sufficient alone: retain verbose designated requirements, nested signing order, timestamp, Team/identifier and entitlement allow-list checks for app/helper/frameworks. |
| Notarization | `xcrun notarytool submit ... --wait`, submission ID/status and complete log show accepted; no warning is silently waived. |
| Ticket | `xcrun stapler staple` then `xcrun stapler validate` on both shipping DMG and app where applicable; repeat offline clean-account check. |
| Gatekeeper | `spctl --assess --type open -vv` on the downloaded artifact/app from a clean non-developer account. Record OS/build and policy result. |
| Installer | Mount DMG, drag/install to a user-writable Applications location, launch, close/reopen, uninstall; verify no privileged/background component appears. |
| Update | Stable-to-next stable full archive with normal, interrupted, corrupt, disk-full, signature-invalid, feed-invalid and manually restored prior-version cases from U07–U26. |
| Live Runtime | Update GUI while idle and while a real shell/TUI executes. Demonstrate protocol-compatible reattach and deliberate mismatch block; never call this helper hot-swap. |
| #687 durability | Run the jointly accepted schema/migration/restart/recovery grid before declaring update rollback safe. |
| Performance | Record package bytes, cold/warm launch, update-check wall/CPU/network, download/install/relaunch time, retry count and background resource use against predeclared budgets. |
| Privacy/security | Independent security review verifies threats above, signing/key custody procedure, redaction and absence of secret material in artifact/logs/repository. |
| Fresh documentation | Independent user follows install/update/recovery/uninstall/offline instructions on a fresh machine and reports exact deviations. |

## Current isolated evidence and limitations

At base `baa7878f78bc50b8b4afd7715275841541f0e007`, `scripts/build-macos.sh`
does build the release-shaped app/helper and **fails closed** without
`SEYAL_CODESIGN_IDENTITY`; a local run produced an unsigned 4,056 KiB app that
failed strict `codesign` verification. This is useful evidence that the
repository does not accidentally label an unsigned release as signed. It is
not Developer ID, notarization, Gatekeeper, updater, performance or clean-
machine evidence. The current base also has bundle version `1` / display
version `0.0.0`, no update framework/feed, no DMG, no signed release record,
and no notarization submission path.

No signing identity, notary credential, update private key, account token or
customer data was requested, read or written for this research.

## Sources

- Apple, [Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- Apple, [Creating distribution-signed code for macOS](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac)
- Apple, [Packaging Mac software for distribution](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- Apple, [Customizing the notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)
- Apple, [Hardened Runtime](https://developer.apple.com/documentation/security/hardened-runtime)
- Sparkle, [documentation and EdDSA/signed-feed setup](https://sparkle-project.org/documentation/)
- Sparkle, [source and release feature record](https://github.com/sparkle-project/Sparkle)

## ADR handoff

Create a separate ADR after #687 resolves the shared contract. It must contain
`## Decision`, `## Rationale`, and `## Alternatives Considered`; name a pinned
Sparkle version and license/provenance review, define key rotation/revocation
and publication roles, formally adopt the compatibility grid, and bind #677's
implementation/validation gates to this matrix. Do not copy this R&D document
into a production implementation PR or accept it as proof that distribution is
ready.
