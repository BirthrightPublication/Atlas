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
See [the exact owner correction](OWNER-UPDATE-SETUP.md#correct-the-current-public-key-setting).

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

The new Windows helper reuses the owner's existing key, verifies the local
pair, and optionally uploads secrets by standard input and the public variable
through GitHub CLI. It refuses CI execution and never creates/replaces a key.
Its Windows syntax/guard validation is tracked separately from real owner use.

The new preflight security test passed locally: missing values, malformed
public key, wrong password, mismatched pair, a valid version-bound proof, public
artifact contents and credential redaction. These are release-tooling checks,
not another disposable update-installation acceptance claim. Prior application
and native validation remains in [the v0.5 report](VALIDATION-v0.5.md).

## Remaining dependencies and acceptance

1. Owner corrects the public environment variable using the existing `.pub`
   file. No key rotation or private material in chat is needed.
2. Rerun signing preflight and build the actual production-configured 0.5.0
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

**Production updater status: blocked on the malformed public setting and later
owner publication approval. Real in-app published-update acceptance is pending.**
No new unsigned candidate is delivered as a solution to this blocker.
