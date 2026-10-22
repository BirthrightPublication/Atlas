# Release and signed update procedure

For the 0.2.0 preview and the verified manual upgrade procedure, see
[v0.2 validation](VALIDATION-v0.2.md). Product identity, installation mode and
the user-data directory remain unchanged from 0.1.0. The Windows CI job verifies
the old installer, launches it, populates synthetic records through its packaged
service, then installs 0.2.0 into the same directory and checks retained data.
Expired baseline artifacts are reproduced from pinned 0.1 source in a temporary
directory. The preview does not enable an unsigned automatic update feed.

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
