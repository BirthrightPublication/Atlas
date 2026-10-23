# Release and signed update procedure

For the 0.4.0 candidate and manual upgrade evidence, see
[v0.4 validation](VALIDATION-v0.4.md). Product identity, installation mode and the
user-data directory remain unchanged. Windows CI verifies the pinned v0.3
installer, populates synthetic records through its packaged service, installs
v0.4 into the same directory and compares retained data. Expired baseline
artifacts can be reproduced from pinned source in a temporary directory. The
preview does not enable an unsigned automatic update feed. Historical v0.1→v0.2
upgrade evidence remains in [the v0.2 report](VALIDATION-v0.2.md).

Development builds have an explicitly unconfigured update state. They never
accept unsigned updates. Production updates use Tauri's verifier, an HTTPS
platform manifest, and a compiled public key. An owner action is required before
installing. The updater creates a database/document backup first; an open edit
form must be saved before choosing Update. Failed verification never falls back
to an unsigned artifact. Program replacement preserves the OS data directory.

The packaging wrapper retains dependency license/notice texts in
`THIRD_PARTY_NOTICES.txt` inside the installed resources and attaches a copy for
each platform to the release. It includes the installed npm dependencies,
resolved Cargo dependencies and Rust distribution notices. This does not select
a license for Atlas. System libraries remain covered by their own distribution
notices.

## Signing setup

Create the updater key pair once on a trusted machine with
`npm run tauri -- signer generate --write-keys <secure-path>`. Store the private
key and its password in a protected recovery location outside the repository.
Never paste either into chat, an issue, source, or logs. The generated public key
is safe to distribute. Do not rotate it casually: existing installed builds
trust the old key.

Create the GitHub environment `release-signing`. Add the private key as secret
`TAURI_SIGNING_PRIVATE_KEY` and the password as
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Add the public key as environment variable
`ATLAS_UPDATE_PUBLIC_KEY`. Apply repository-appropriate access protection to that
environment. The release workflow compiles the public key and the repository's
HTTPS `releases/latest/download/latest.json` feed into the binary.

Updater signatures are distinct from Windows Authenticode and Apple Developer
ID signing/notarization. The workflow currently prepares updater-signed builds;
OS publisher signing is not configured. Configure those identities separately
before claiming publisher verification or a notarized macOS release. Do not
tell users to bypass their operating system's protection.

## Prepare and review

1. Update the versions consistently in the workspace, npm package/lock and Tauri
   configuration. Keep migration compatibility explicit. Run the validation
   journey and restore/upgrade tests against a populated earlier database.
2. Run **Prepare signed draft release** manually in GitHub Actions, supplying
   release notes. Native runners test and build all four exact platform/CPU
   combinations. Missing keys, failed tests, absent artifacts, bad signatures,
   or version mismatches fail the job. Ordinary CI creates unsigned installer
   artifacts for testing and cannot generate a production update feed.
3. The final job re-verifies each SHA-256 and Ed25519/minisign signature, including
   the signed release version, and constructs an explicit platform-to-artifact
   mapping. It creates a **draft** GitHub release containing the exact artifacts,
   signature sidecars, `SHA256SUMS.txt`, and `latest.json`. It never publishes the
   stable feed automatically.
4. Download and install the draft build on each claimed OS/architecture. Test
   launcher startup, persistence across restart, permissions, a real authorized
   mailbox, the selected physical phone, backups and upgrade from a populated
   prior release. Record skipped or blocked checks explicitly. Review artifact
   names, release notes, signatures and update compatibility, then publish the
   draft through GitHub when ready. A published stable release exposes the
   already complete manifest and artifacts together.

If a platform build fails, no complete stable manifest is produced. Do not omit
a platform silently. If database schema rollback is unsafe, restore an earlier
backup with a compatible app version instead of opening a newer database in an
older binary. Keep model weights/caches outside the replaceable program bundle.

`node --test scripts/release.test.mjs` generates a temporary synthetic key pair,
signs a synthetic artifact with the installed Tauri CLI, and checks rejection of
changed bytes, another key, a changed signed comment, and a mismatched version.
This verifies release-checking code; it is not evidence of an owner-signed build
or an installed upgrade on another OS.

## Version 0.3 controlled update validation

The Windows packaging job first validates the ordinary preview installer and a
populated v0.2→v0.3 NSIS upgrade. It then builds two separate disposable test
installers with the `update-smoke` feature, a generated update key and a local
HTTPS certificate authority. The feature is absent from ordinary packages.
Tests verify older-version handling, wrong-platform rejection, invalid signatures,
interrupted downloads, a valid signed NSIS install, automatic native restart into
`0.3.1-update-test.1`, and retained records, history, documents, chats and
notifications. The valid case starts the managed companion and creates disposable
pairing state, then verifies the old sidecar exits and that state survives.
TLS validation remains enabled with that explicit test trust root.
The test app refuses a non-runner-temporary profile. Private test keys stay under
RUNNER_TEMP and are removed afterward. Only the ordinary preview installer is
staged for delivery; test versions never enter the public release feed.

See the validation report for executed results and artifact digests. The v0.2
preview has no trusted production key, so its upgrade is a verified manual
installer transition. A production-key-bearing v0.3 build must be installed
manually once before subsequent owner-signed releases can use **Update Now**.
Complete the Signing setup above and verify the first published HTTPS manifest;
no preview or disposable test establishes that production has been activated.


## Version 0.4 candidate

The current pipeline pins the validated v0.3 source and Windows artifact as its
manual upgrade baseline. Ordinary v0.4 installers preserve the established app
identity and data directory. Disposable signed tests use 0.4.0 →
0.4.1-update-test.1, retain the TLS/signature checks and verify native restart and
managed companion shutdown. These disposable keys do not activate production.
See the v0.4 validation report for executed platform results and exact hashes.
