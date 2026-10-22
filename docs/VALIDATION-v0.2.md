# Version 0.2.0 validation

This release changes presentation and interaction, retaining the existing Rust
domain services, schema 3, product identity and OS data directory. The original
v0.1 baseline is `bb557c443e154da3b7f045cf7b21c2e43b97e907`.
The tested application source is `7c1b3d3a8a258e5cfceb0ae71dbb0f123d418709`;
subsequent documentation and screenshot-capture changes do not alter the app.

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

## Inspected screenshots

All captures use synthetic records. The desktop workspace scrolls independently
above the dock; content below the visible viewport remains reachable by scrolling.

| View | Capture |
| --- | --- |
| Calendar, 2560×1440 | [Wide desktop](screenshots/v0.2/v020-2560-calendar.png) |
| Calendar, 1920×1080 | [Desktop](screenshots/v0.2/v020-1920-calendar.png) |
| Calendar, 1366×768 | [Laptop](screenshots/v0.2/v020-1366-calendar.png) |
| Calendar, 1024×768 | [Compact desktop](screenshots/v0.2/v020-1024-calendar.png) |
| Calendar, 390×844 | [Mobile agenda](screenshots/v0.2/v020-390-calendar.png) |
| Discover | [Website library](screenshots/v0.2/v020-1366-discover.png) |
| Map | [Saved places](screenshots/v0.2/v020-1024-map.png) |
| Budget | [Monthly accounting](screenshots/v0.2/v020-1024-budget.png) |
| Settings | [Desktop modal](screenshots/v0.2/v020-1366-settings.png), [compact modal](screenshots/v0.2/v020-390-settings.png) |
| Companion | [Offline-capable budget](screenshots/v0.2/companion-budget-mobile.png) |
| Native Linux AppImage | [Calendar after restart](screenshots/v0.2/native-linux-restarted.png), [upgraded budget](screenshots/v0.2/native-linux-budget.png) |

## Native packaging

Native CI runs in [Validate and package, run 39864507582](https://github.com/BirthrightPublication/Atlas/actions/runs/39864507582).
The Windows upgrade harness validates the exact previous/new installers and
uses an isolated disposable runner profile. Browser checks do not establish
Windows installation or physical iPhone behavior.

Windows x64 and macOS Apple Silicon CI have completed successfully. The Intel
macOS package is still building, and Ubuntu 22.04 CI is still installing system
prerequisites at the time of this report; neither pending job is claimed as a
completed v0.2 package. Follow the run above for subsequent status. Local Linux
packaging and native execution are separately verified below.

The Windows x64 job passed on Windows Server 2022. It verified and installed the
previous 0.1.0 NSIS artifact, launched and closed its native window, and populated
six synthetic records plus preferences, history and a document using its
packaged service. It then installed 0.2.0 in the same location, verified the
normal Start Menu shortcut and packaged resources, launched/closed the native
app twice, and checked restart persistence and SQLite integrity. The real
packaged service verified unchanged records, IDs, versions, history, preferences,
presets, documents, schema 3 and exact USD 600/300/900 accounting. Both the
baseline-verification and manual-upgrade CI steps passed. The artifact includes
`windows-installation-smoke.json`; this is runner validation, not installation
on the owner's computer.

The local Linux AppImage and Debian package built successfully. The AppImage
opened an isolated profile populated by v0.1. Its native IPC displayed the old
stay/objective and the correct USD 600 December accommodation allocation. A new
objective created through the native form survived an application restart; all
six original records remained identical. Both launches closed normally with
exit code 0, schema stayed at 3 and SQLite integrity remained `ok`. See the
[smoke report](validation/v0.2-linux-smoke.json). WebKit sandboxing stayed enabled
inside the read-only GTK/WebKit SDK namespace and Xvfb. The Debian package was
built, not installed. These local packages use Debian 13/glibc 2.41 and do not
establish compatibility with older Linux distributions.

Local package digests were independently recomputed against `artifacts.json`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `Birthright-Atlas_0.2.0_linux_x86_64.AppImage` | 123525624 | `e6cb15ad1a6a2d7088be63f5cd31caccd8e924dd8e169297b79ffeaed0428535` |
| `Birthright-Atlas_0.2.0_linux_x86_64.deb` | 21843574 | `195037f0f6bd85e3b85aa38be8bef33822bea754d7569745f88d23c8417ba3c7` |

CI package digests below come from the staging step's SHA-256 annotations;
each download also includes the exact artifact manifest. They are distinct
from GitHub's enclosing ZIP archive digest. macOS
packages are built and tested at the service level; installation remains
unverified. GitHub artifact downloads require repository access and expire
after 14 days.
This cloud's attempt to download the Windows artifact ZIP returned HTTP 403
from artifact storage. The Windows runner verified and installed the artifact;
the cloud did not independently re-download its bytes.

| File | Artifact download | Installer bytes | Installer SHA-256 |
| --- | --- | ---: | --- |
| `Birthright-Atlas_0.2.0_windows_x86_64-setup.exe` | [Windows x64](https://github.com/BirthrightPublication/Atlas/actions/runs/39864507582/artifacts/12463818350) | 9450727 | `cbec6e6425c35b7d472cbf34408fc4d80d9e3f7bad720072ac1f3540b6436be4` |
| `Birthright-Atlas_0.2.0_macos_aarch64.dmg` | [Apple Silicon](https://github.com/BirthrightPublication/Atlas/actions/runs/39864507582/artifacts/12463014007) | 11872234 | `0aebc4ab80ac6ff9fe3461cce9a4791a8dcc03a29ec4b554a8868d844ef7fed3` |

## Manual upgrade from 0.1

1. In the installed app, save open edits and use Settings → Privacy & Data →
   Create a local backup. In 0.1, this category is called Data & recovery.
2. Close Atlas. Download the Windows x64 0.2.0 preview installer and verify its
   exact SHA-256 against the table above or bundled `artifacts.json`. Extract
   `windows-x86_64/Birthright-Atlas_0.2.0_windows_x86_64-setup.exe` from the
   downloaded artifact ZIP; the SHA-256 above applies to this `.exe`, not the ZIP.
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
