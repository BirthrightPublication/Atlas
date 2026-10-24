# Version 0.6 updater activation

## Actual root cause and repository state

The owner's installed 0.5.0 was the ordinary unsigned review installer. Its
build intentionally had no compiled permanent public key or update endpoint.
Adding GitHub credentials cannot change a previously installed executable.
It requires one production-configured bootstrap installation over the existing
Atlas directory. The application identity remains `org.birthright.atlas` and
its SQLite schema stays 5. Never uninstall/delete the existing profile to update.

The owner merged PR #5 into `main` at
`1b5edaef17e5d2bbf466787b18f76a46378677ac`. The signed-release workflow is now
available on the default branch, and `release-signing` exists. The owner's
[first signed-release attempt](https://github.com/BirthrightPublication/Atlas/actions/runs/40071158255)
failed during preflight; it produced no installer or draft.

The [diagnostic run](https://github.com/BirthrightPublication/Atlas/actions/runs/40072004529)
at `e95c6638ae0b6e8eac7a3b9f5ab1dca22def27cb` identified the precise blocker:
`ATLAS_UPDATE_PUBLIC_KEY` is present but is not a valid Tauri public-key
envelope. Both required secret values are present; their correctness/match has
not been established. Their contents were never read through this workspace.
The [later signing check](https://github.com/BirthrightPublication/Atlas/actions/runs/40072472572)
at `c386f296a4d4c99e01032687b3d5cf22eb252664` reported the same malformed public
setting and stopped before native builds. These are historical attempts; the
owner subsequently corrected the public-key format.

The requested [rerun, attempt 2](https://github.com/BirthrightPublication/Atlas/actions/runs/40072472572/attempts/2)
passed the public-key format check but could not sign with the stored private
key/password. A [more precise diagnostic run](https://github.com/BirthrightPublication/Atlas/actions/runs/40072999050)
at `50f792ba1225070fbf00b06f17e7f0d39900d014` confirmed the signing CLI rejects
the stored password for that private key. Only a constant, redacted category
was exposed; raw signing output and credentials were withheld. No installer or
draft was created. See [the password correction](OWNER-UPDATE-SETUP.md#correct-the-signing-key-password).

After the owner corrected the password, [attempt 2 of the diagnostic run](https://github.com/BirthrightPublication/Atlas/actions/runs/40072999050/attempts/2)
passed preflight using the existing permanent identity. A real version-bound
signature verified successfully. The public-key packet SHA-256 is
`667267e572c171192ff432236e4db84f70e60d5348f8dbe81de8a9ec875c43ea`.
The production-configured 0.5.0 bootstrap is building from
`50f792ba1225070fbf00b06f17e7f0d39900d014`. Private contents remain in protected
GitHub Actions; no permanent key was generated or changed in this workspace.

No stable or draft GitHub release exists. Consequently the public
`https://github.com/BirthrightPublication/Atlas/releases/latest/download/latest.json`
endpoint cannot provide a production update yet. No preview artifact is
substituted for that endpoint.

## Independent engineering work

Preflight now reports each missing/malformed setting as a GitHub error
annotation and step summary. Once settings are valid, it uses the existing
private key and password to sign a synthetic, version-bound proof and verifies
it against the configured public key before any long native build. Child
output is withheld. Only the public key and its SHA-256 identity record become
artifacts; they accompany the eventual draft so both release versions can be
compared against the same identity. This check neither generates a permanent
key nor publishes or installs an update.

Private-key paths, malformed key contents and a password that does not unlock
the key now receive separate actionable annotations. The expanded local security
test passes these cases and checks that paths, private contents and passwords
never enter diagnostic output. CLI messages are matched internally and are
never forwarded to logs. The actual protected run established the current
password failure; the correct secret value requires the owner's original
password and was corrected by the owner in GitHub.

The new Windows helper reuses the owner's existing key, verifies the local
pair, and optionally uploads secrets by standard input and the public variable
through GitHub CLI. It refuses CI execution and never creates/replaces a key.
Its [Windows tooling check](https://github.com/BirthrightPublication/Atlas/actions/runs/40072290281)
passed at `b805bf5f922e2fa362a3a597d23af6de2bc7d1c3`: the actual Windows runner
passed the signing-preflight security test, parsed the PowerShell helper and
proved it refuses CI execution. Real owner upload is a separate operation.
The [final Windows tooling check](https://github.com/BirthrightPublication/Atlas/actions/runs/40072457720)
also passed at `c386f296a4d4c99e01032687b3d5cf22eb252664`, including the added
public-key whitespace normalization/configuration test. The release wrapper
now gives the generated Tauri configuration and compiler child process the same
trimmed public key; copied trailing newlines cannot create inconsistent trust.

The new preflight security test passed locally: missing values, malformed
public key, wrong password, mismatched pair, a valid version-bound proof, public
artifact contents and credential redaction. These are release-tooling checks,
not another disposable update-installation acceptance claim. Prior application
and native validation remains in [the v0.5 report](VALIDATION-v0.5.md).

## Remaining dependencies and acceptance

The pinned bootstrap source remains 0.5.0. The current target metadata is 0.6.0
across npm, Cargo and Tauri. The target adds separate missing-manifest,
incompatible-platform, transfer, signature and installation diagnostics. Two
native updater tests, all 146 workspace tests, the retained test-feature compile
and the production frontend build passed locally. The
ordinary planning/data services and schema remain unchanged.

1. Signing configuration is verified; reuse this exact permanent identity for
   both versions and all subsequent releases.
2. Finish the actual production-configured 0.5.0
   bootstrap. Verify signatures, checksums and its populated Windows upgrade.
3. Build the newer 0.6.0 release with the same verified permanent identity and
   review the complete drafts. Stable publication needs owner approval.
4. After approved publication, verify the actual public HTTPS manifest and all
   referenced signed assets. Install the bootstrap once, open the native Updates
   panel and click **Update Now**. Record actual transfer, signature check,
   installer/restart and 0.6.0 version, plus data/settings/documents/model-cache
   and companion/email-state retention.
5. Record actual no-new-release, offline, interruption, invalid signature,
   platform, installation-failure, concurrent-update and backup-recovery results
   separately. Previous controlled checks do not establish published acceptance.

**Production updater status: signing verified and production builds running;
owner publication approval and real in-app published-update acceptance remain
pending.**
No new unsigned candidate is delivered as a solution to this blocker.
