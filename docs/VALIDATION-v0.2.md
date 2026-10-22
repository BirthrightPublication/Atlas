# Version 0.2.0 validation

This release changes presentation and interaction, retaining the existing Rust
domain services, schema 3, product identity and OS data directory. The original
v0.1 baseline is `bb557c443e154da3b7f045cf7b21c2e43b97e907`.

## Completed checks

- 111 substantive Rust tests passed, including money, timezone/DST, migrations,
  imports, reminders, authorization and offline companion services.
- 11 Chromium journeys passed against the real local service: import/revision
  review, objectives/reminders, stay creation/editing, transport, settings saves,
  keyboard focus, dock navigation and companion offline replay/conflicts.
- Responsive checks cover 2560×1440, 1920×1080, 1366×768, 1024×768 and 390×844.
  They verify neutral surface colors, full-width Calendar, five dock controls,
  a single icon-only Settings launcher, dialog bounds, no horizontal page
  overflow, focus restoration and content/dock separation.
- TypeScript and production bundling passed without CSS syntax warnings.
  Three exact-money tests, real update-signature verification and two secure
  download redirect tests passed.
- A separate synthetic profile was populated using the v0.1 host and reopened
  using v0.2. All six records, stable IDs, versions, history, preferences, named
  presets and one document survived unchanged. Schema stayed at 3, SQLite
  integrity passed, and the USD 900 prepayment still allocates USD 600/300 to
  December/January and USD 900 to November cash flow.

## Native packaging

Native packaging and the installed Windows upgrade check are being run. Results,
artifact links and exact SHA-256 digests will be recorded here when available.
The Windows upgrade harness validates the exact previous/new installers and
uses an isolated disposable runner profile. Browser checks do not establish
Windows installation or physical iPhone behavior.

## Manual upgrade from 0.1

1. In the installed app, save open edits and use Settings → Privacy & Data →
   Create a local backup. In 0.1, this category is called Data & recovery.
2. Close Atlas. Download the Windows x64 0.2.0 preview installer and verify its
   exact SHA-256 against the published artifact manifest.
3. Run the installer using the same Windows account and existing installation
   location. Do not delete the Atlas application-data directory.
4. Open Atlas from its normal launcher. Verify version 0.2.0 under Settings →
   About Atlas and confirm your records and preferences.

No Node or Rust toolchain is required on the user's Windows computer. Preview
installers remain unsigned by a publisher. Protected updater signing keys and
a production feed are still required for automatic updates; signature
verification remains enforced.

## External validation limits

Live Proton Bridge authorization, a physical iPhone/private HTTPS route and
macOS installation remain external checks. No new live mailbox, AI inference,
map inventory or device claims are made. The official model CDN was blocked in
the cloud environment during v0.1 evaluation; no CPU inference benchmark is
claimed. Native startup and installer results are reported separately above.
