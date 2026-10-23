# Version 0.3 validation

Atlas 0.3 is developed on `codex/atlas-v0.3-planning`, stacked on the unmerged
`codex/atlas-v0.2-interface` branch. [Draft PR #3](https://github.com/BirthrightPublication/Atlas/pull/3)
retains the v0.1/v0.2 history. The application identity and data directory are unchanged.
This is a development candidate, not a published stable release.
[Candidate release notes](RELEASE-NOTES-v0.3.md) describe the changes and upgrade path.

## Local service and interface results

The full local suite passed **124 Rust checks**: 96 core unit tests, four SQLite
acceptance journeys, eleven v0.3 planning/persistence tests, six service-boundary
tests and seven companion-host tests. Empty binary/doc-test targets are excluded.
TypeScript and production bundling passed. Four money/signature tests and two
local-AI installer security tests passed. Native Rust checks also compiled the
optional controlled-update test feature.

All **14 Chromium journeys** passed against the real host and SQLite services.
These cover the original imports/payments/revisions and companion offline queue,
Settings save/discard and restart persistence, shared utility panel toggling,
contextual saved chats, notification targets/read state, duplicate-as-draft,
linked documents and immutable transport-price scenarios. No mock database or
successful-response fallback was substituted.

The five required sizes were captured and inspected: 2560×1440, 1920×1080,
1366×768, 1024×768 and 390×844. Browser font inspection confirmed the numeric
face selected Times/Liberation Serif. Windows can use its installed Times New
Roman; Linux falls back to Liberation Serif. No proprietary font was bundled.

- [Calendar, 2560px](screenshots/v0.3/v030-2560-calendar.png)
- [Calendar, 1920px](screenshots/v0.3/v030-1920-calendar.png)
- [Assistant, 1366px](screenshots/v0.3/v030-1366-assistant.png)
- [Map, 1024px](screenshots/v0.3/v030-1024-map.png)
- [Settings, 390px](screenshots/v0.3/v030-390-settings.png)
- [Assistant, 390px](screenshots/v0.3/v030-390-assistant.png)
- [Budget scenario](screenshots/v0.3/v030-budget-scenario.png)

Published release history is fetched outside the database mutex and caches
successful results. A connection warning keeps cached metadata usable and never
becomes executable installation authority. The public GitHub repository currently
has no published releases. This cloud's application HTTP client could not reach
the release service; the interface showed the unavailable/no-cache state honestly.
The authenticated GitHub API separately confirmed the empty published release list.

## Data migration and recovery

The actual retained v0.2 host created an isolated populated schema-3 profile.
The v0.3 host migrated it to schema 4 with a pre-migration backup and retained
six records, IDs, versions, complete audit history, preferences, named presets,
one local document and its SHA-256, and exact accounting: USD 600/300 in the two
months of use and USD 900 in the separate payment month. Chat and notification
state survived another host restart exactly.

An actual schema-3 backup exported by the v0.2 executable was restored through
v0.3. Original records remained identical, both pre-restore and pre-migration
backups were retained, and the new conversation tables were usable afterward.
Separate regression checks cover schema 1 migration, checksum rejection, money,
currency history, exclusive checkout, DST, date-line travel, work conflicts,
unknown workspace evidence and non-mutating scenarios.

## Packaged Linux validation

Local packages were built from application source
`79215a1e8bb045cbc80d5022b06e7daa24c86e70`. The AppImage launched on Debian 13/Xvfb
using the verified read-only GTK/WebKit SDK namespace. WebKit sandboxing remained
enabled. There were three native launches and three clean closes, all exit code 0.
A conversation created through the native panel reopened after restart, and its
ordered messages and notification state matched exactly. The six v0.2 records,
document and accounting remained unchanged after native migration.

[Native smoke evidence](validation/v0.3-linux-smoke.json),
[Settings with an independent panel](screenshots/v0.3/native-linux-settings.png),
and [restored native chat](screenshots/v0.3/native-linux-restarted.png) are retained.
These local packages do not establish compatibility with older Linux distributions.

| Local file                                     |     Bytes | SHA-256                                                            |
| ---------------------------------------------- | --------: | ------------------------------------------------------------------ |
| `Birthright-Atlas_0.3.0_linux_x86_64.AppImage` | 123763192 | `d6f19e543cf2d7db458e5f0ecd858f5d37f93eb66854e9513471dbadf30fd873` |
| `Birthright-Atlas_0.3.0_linux_x86_64.deb`      |  22094086 | `f4c1673dbfa8cb324945baa644530f1e77dbe4f1dd31557721dba2f8aa731463` |

## Native CI and signed updates

The Windows job in [candidate run 39877897142](https://github.com/BirthrightPublication/Atlas/actions/runs/39877897142)
passed from source `1847bb6a6a95cb6b49e9f0dce9cbe7b38fa20278`.
The ordinary NSIS installer passed actual installation, packaged resource and
Start Menu launcher checks, two native launches/clean closes, SQLite integrity,
and the populated manual v0.2→v0.3 upgrade.

The separate controlled signed-update test passed all five cases: older release,
wrong platform, bad signature, interrupted download and valid update. The valid
case installed the signed NSIS package and automatically restarted the real
executable at `0.3.1-update-test.1`. Records, versions, history, preferences,
documents, exact accounting, chats and notifications were retained. The test kept
the managed companion running, created disposable pairing state through its real
service, and confirmed sidecar shutdown and unchanged pairing state after restart.

The [Windows installation report](validation/v0.3-windows-installation-smoke.json)
and [controlled update report](validation/v0.3-controlled-update-smoke.json) were
read from the SHA-256-verified candidate archive. The native windows became ready
in 315 ms and 254 ms on the disposable Windows CI runner; these are smoke timings,
not a general performance guarantee.

[Download the Windows candidate archive](https://github.com/BirthrightPublication/Atlas/actions/runs/39877897142/artifacts/12469832923)
and extract `windows-x86_64/Birthright-Atlas_0.3.0_windows_x86_64-setup.exe`.
The ordinary installer is **9,647,561 bytes**, with SHA-256
`8242b2bc5f00e4c5db165fe8c4dbcb18d4147ce1a1b26cc86a909ae873fe3acd`.
The archive also contains the installation and controlled-update reports; its
SHA-256 is `243c12ae4cb5ce0dc5395c42ba4fed714339ff79031b88a2a1e8d7868bbb1f61`.
GitHub retains these CI artifacts for 14 days; they are not published releases.

All four native jobs passed. Linux CI produced AppImage/deb packages on Ubuntu
22.04; both macOS builds produced DMGs. Actual macOS installation is unverified.
The [complete CI artifact record](validation/v0.3-native-ci.json) retains source,
job URLs, filenames, lengths and SHA-256 values for every package and archive.
The native Linux launch evidence above uses the separately identified local build.

| Platform | Candidate archive | Executed validation |
| --- | --- | --- |
| Windows x64 | [NSIS installer](https://github.com/BirthrightPublication/Atlas/actions/runs/39877897142/artifacts/12469832923) | Services, packaging, installation, native restart, manual upgrade and controlled signed update |
| Linux x64 | [AppImage and deb](https://github.com/BirthrightPublication/Atlas/actions/runs/39877897142/artifacts/12470021957) | Services and packaging; separate local native evidence above |
| macOS Apple Silicon | [DMG](https://github.com/BirthrightPublication/Atlas/actions/runs/39877897142/artifacts/12469168764) | Services and packaging |
| macOS Intel | [DMG](https://github.com/BirthrightPublication/Atlas/actions/runs/39877897142/artifacts/12469258655) | Services and packaging |

The controlled feed uses disposable signing keys and a separate HTTPS certificate
authority. Local TLS checks accepted the explicit disposable CA and rejected the
same endpoint without that trust root. No TLS or signature checks were disabled.
Private fixture keys were removed after local validation. CI test installers and
credentials are separate from the staged ordinary preview installer.

## Remaining owner and external checks

Production update signing and the published stable feed are **not configured**.
Follow [the exact signing/release setup](RELEASE.md). Install a build containing
the production verification key manually once; the old v0.2 preview cannot learn
that key through its own interface. Only then can subsequent appropriately signed
published releases use **Update Now**. Disposable test credentials do not activate
production updates. No stable release was merged or published during this work.

Live Proton Bridge authorization, trusted private HTTPS on a physical iPhone,
optional model inference/CPU benchmarking and macOS installation remain separate
unverified external checks. The ordinary preview does not claim Windows publisher
signing or Apple notarization. All committed fixtures and screenshots use synthetic
or empty profiles; personal files, credentials, databases and models remain outside Git.
