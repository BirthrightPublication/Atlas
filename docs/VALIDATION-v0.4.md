# Version 0.4 validation

Atlas 0.4 is developed on `codex/atlas-v0.4-booking-automation`, based on exact
v0.3 head `da1d9210d492821b5dd8e3c2b19e55433f6b3c64`.
[Draft PR #4](https://github.com/BirthrightPublication/Atlas/pull/4) targets the
existing v0.3 branch and preserves prior history. This is a development candidate;
no stable release was merged or published.
[Candidate release notes](RELEASE-NOTES-v0.4.md) describe its behavior.

Local application tests and packages used
`401cc6d6b746fc4bfbab327b57b200bef563d1a5`. Native CI uses
`818fc88af39c8569032a97be59f7007fe7bd671f`, which changes only validation scripts,
CI configuration and contributor instructions. It checks the pinned v0.3 Windows
baseline and reads the controlled feed version from the signed test build.
Application sources are identical. Later documentation-only commits retain these
exact build and test references instead of implying rebuilt binaries.

## Local services and interface

All **137 Rust tests** passed: 101 core unit tests, four SQLite acceptance
journeys, eight v0.4 automation tests, eleven v0.3 planning tests, six boundary
tests and seven companion-host tests. TypeScript and production bundling passed.
Five Node exact-money/signature/controlled-HTTPS-feed tests and two Python
model-installer security tests passed. The feed regression check verifies all
five scenarios, dynamic build version metadata and rejection without the test CA.
CI runs this Windows updater fixture on the Windows runner; cross-platform
money/signature checks remain enabled on every runner. Empty binary/doc-test targets are excluded from the test count.

Coverage includes account-scoped deduplication, failed-batch rollback, header
filters and cursor persistence, selective booking revisions, source/history
retention, cancellation previews, expected and partial received refunds,
reminder recalculation and snooze preservation, readiness unknowns, saved-offer
scenarios and watchlist evidence. A real stalled TLS socket check confirms email
connection work does not hold the main SQLite mutex. No live mailbox
credentials, native keychain authorization or model inference are implied.

All **20 Chromium journeys** passed against the real local host and SQLite
services. Existing Calendar, Budget, Settings, utility panel, import, companion
queue and planning journeys remain covered. New journeys exercise focused import
review, selected fields, connections navigation,
Trip Readiness, saved-offer comparisons and noncommitting scenarios.

The five required sizes were captured and inspected: 2560×1440, 1920×1080,
1366×768, 1024×768 and 390×844. The neutral theme, bottom dock, independent utility
panels and serif number face remain. Times New Roman is used when installed;
Linux can fall back to Liberation Serif/Times. No proprietary font was bundled.
Synthetic evidence with a future check date is visibly flagged as invalid rather
than presented as a current verified price.

- [Import review, 2560px](screenshots/v0.4/v040-2560-imports.png)
- [Saved-offer comparison, 1920px](screenshots/v0.4/v040-1920-compare.png)
- [Trip Readiness, 1366px](screenshots/v0.4/v040-1366-readiness.png)
- [Import review, 1024px](screenshots/v0.4/v040-1024-imports.png)
- [Comparison, 390px](screenshots/v0.4/v040-390-compare.png)
- [Readiness, 390px](screenshots/v0.4/v040-390-readiness.png)
- [Mailbox filters](screenshots/v0.4/v040-email-filters.png)

## Migration and recovery

The actual retained v0.3 host seeded isolated populated schema-4 profiles.
The v0.4 host and packaged native app migrated them to schema 5 with a retained
pre-migration backup. Six records, stable IDs, versions, complete history,
preferences/presets, one document and its SHA-256, conversations and notifications
survived. Exact accounting remained USD 600/300 for the two months of use and
USD 900 for the separate payment month. SQLite integrity checks passed.

The automated schema-4 restore check also retains documents, conversations,
notifications and companion pairing files. Additional regression checks cover
older supported schemas, checksum rejection, currencies, exclusive checkout,
DST, date-line travel and immutable budget scenarios.

## Packaged Linux validation

The final local AppImage was extracted and its packaged `AppRun` launched on
Debian 13/Xvfb in the verified read-only GTK/WebKit SDK namespace. WebKit
sandboxing remained enabled. **Three native launches and three clean closes**
all returned exit code 0. Calendar, Import Center, Trip Readiness and the
Assistant were inspected. A readiness question sent through the native panel
reopened after restart; ordered messages and notification rows matched exactly.
The original v0.3 records, document and accounting remained unchanged.

[Native smoke evidence](validation/v0.4-linux-smoke.json),
[local artifact manifest](validation/v0.4-linux-local-artifacts.json),
[native Import Center](screenshots/v0.4/native-linux-imports.png),
[native readiness](screenshots/v0.4/native-linux-readiness.png) and
[reopened readiness conversation](screenshots/v0.4/native-linux-restarted.png)
are retained. These local packages do not establish older Linux distribution
compatibility. The deb was built but not installed in this cloud workspace.

| Local package | Bytes | SHA-256 |
| --- | ---: | --- |
| `Birthright-Atlas_0.4.0_linux_x86_64.AppImage` | 124049912 | `93207a21698773a196548aacc54db75840a38ef631259a9eb5f8e909384d8e53` |
| `Birthright-Atlas_0.4.0_linux_x86_64.deb` | 22403288 | `b522a074afaee0c07f78840c5e3a8e51b72e51db5acfdf6a61ca01ccb464b7f2` |

## Native CI and Windows delivery

All four native jobs passed in [candidate run 40039373930](https://github.com/BirthrightPublication/Atlas/actions/runs/40039373930)
from source `818fc88af39c8569032a97be59f7007fe7bd671f`. The Windows NSIS installer passed actual installation,
packaged resource and Start Menu launcher checks, two native launches/clean
closes, SQLite integrity and the populated manual **v0.3→v0.4 upgrade**.
The synthetic profiles retained records, versions, history, preferences, presets,
documents, chats, notifications, exact accounting and a backed-up schema migration.

The controlled signed-update test passed all five cases: older release, wrong
platform, invalid signature, interrupted download and valid update. The valid
case installed the signed NSIS package and automatically restarted the real app
at **0.4.1-update-test.1**, retaining populated data. The test exercised managed
companion shutdown/restart and retained its disposable pairing state. It used a
separate HTTPS CA and disposable signing keys; TLS and signature checks stayed
enabled. Test installers are separate from the ordinary candidate installer.

The [Windows installation report](validation/v0.4-windows-installation-smoke.json)
and [controlled update report](validation/v0.4-controlled-update-smoke.json) are
retained from the synthetic CI reports. Native windows became ready in
269 ms and 264 ms on the disposable Windows runner;
these are smoke timings, not a general performance guarantee.

[Download the Windows candidate archive](https://github.com/BirthrightPublication/Atlas/actions/runs/40039373930/artifacts/12539447648)
and extract `windows-x86_64/Birthright-Atlas_0.4.0_windows_x86_64-setup.exe`.
The installer is **9,872,603 bytes**, with SHA-256
`aa4961bbf9ffc1d9171ea52a47bf39a83bcc241ae52e14ddd04f7d8412398adb`.
It includes the app, companion resources and required runtime setup; users do
not need Rust, Node.js or a development environment. This is an unsigned
publisher preview; production Authenticode signing is not claimed.

The [complete CI artifact record](validation/v0.4-native-ci.json) retains job
URLs, archive digests, package filenames, lengths and SHA-256 values. Reports and
package manifests were retrieved through GitHub check annotations; archive
metadata came from the GitHub Actions API. GitHub retains these artifacts for
14 days. They are development downloads, not published stable releases.

| Platform | Candidate archive | Executed validation |
| --- | --- | --- |
| Windows x64 | [NSIS installer](https://github.com/BirthrightPublication/Atlas/actions/runs/40039373930/artifacts/12539447648) | Services, packaging, installation, manual v0.3 upgrade and controlled signed restart |
| Linux x64 | [AppImage and deb](https://github.com/BirthrightPublication/Atlas/actions/runs/40039373930/artifacts/12539231460) | Services and packaging; separate local native smoke above |
| macOS Apple Silicon | [DMG](https://github.com/BirthrightPublication/Atlas/actions/runs/40039373930/artifacts/12538907161) | Services and packaging |
| macOS Intel | [DMG](https://github.com/BirthrightPublication/Atlas/actions/runs/40039373930/artifacts/12539237877) | Services and packaging |

Linux CI packages were built on Ubuntu 22.04. Both macOS architectures produced
DMGs; actual macOS installation/startup and notarization remain unverified.
The Linux launch evidence above uses the separately identified local package.

## External checks and release configuration

Live Proton Bridge/Gmail/Outlook/IMAP authorization and native credential-store
access with an owner's account remain unverified. The paid Proton Bridge route
is supported by the adapter; credentials must be entered in the native app.
Physical iPhone pairing/private HTTPS, optional model inference/CPU benchmarking
and macOS installation/startup remain separate unverified checks. Saved-offer
comparison and manual watchlists do not imply live travel inventory or scheduled
provider monitoring.

Production signing and the stable HTTPS update feed are **not configured**.
Follow [the signing/release guide](RELEASE.md). A preview lacking the production
verification key must first receive a manually installed build containing that
key before later signed releases can use Update Now. Disposable test signing
credentials do not activate production updates. The ordinary preview does not
claim Windows publisher signing or Apple notarization. TLS and signature
verification remain enforced.

All committed reports and screenshots use synthetic profiles. Personal files,
mailbox credentials, signing keys, databases and models remain outside Git.
