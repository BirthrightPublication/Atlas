# Development preview validation

Application and packaging source: `bb557c443e154da3b7f045cf7b21c2e43b97e907`.
Checks below were performed on 2026-10-22 with synthetic records. This is a
development preview, not a claim of installation on the owner's computer.

## Completed local checks

- 111 substantive Rust tests: 94 core unit tests, four SQLite acceptance
  journeys, six service-boundary tests and seven companion-host tests. The final
  service run had no failures or ignored tests. Empty binary/doc targets are
  excluded from this count.
- Five Chromium journeys through the real Rust service: reviewed/revised
  imports, calendar and accounting, persistent objectives/reminders, responsive
  desktop/phone layouts, pairing, offline documents, exactly-once expense replay
  and explicit edit conflicts.
- TypeScript checking and production frontend build; three exact monetary input
  tests, real cryptographic update verification, and two HTTPS artifact-redirect
  boundary tests.
- Full unprivileged cloud installation script, native SDK resolution, and
  development startup. The frontend and service health returned HTTP 200.
- Optimized native Linux build and actual AppImage launch. A stay created in
  the native UI survived close/reopen; the final package displayed the saved
  stay and closed normally with exit code 0. WebKit's sandbox remained enabled.
  The cloud display used Xvfb and its installed SDK graphics libraries.
- Packaged companion executable, frontend/service worker, icons and retained
  third-party notices were present with usable file permissions. The packaged
  companion sidecar was copied from the newly built host containing the Bridge
  trust-certificate fix; the packaged executable ran `--help` successfully.

## Local Linux artifacts

These unsigned packages were built on Debian 13 (glibc 2.41). They do not
establish compatibility with older distributions. The separate GitHub Linux
pipeline uses Ubuntu 22.04. Debian package metadata and contents were inspected;
it was not installed. The AppImage was actually executed.

| File in `release/linux-x86_64/` | Bytes | SHA-256 |
| --- | ---: | --- |
| `Birthright-Atlas_0.1.0_linux_x86_64.AppImage` | 123,521,528 | `779c3ecc4cb8426d016ffd9776cbb01d26160e09e23e02479ebc4557b607457d` |
| `Birthright-Atlas_0.1.0_linux_x86_64.deb` | 21,843,616 | `4e89afebb078e79929073e929535a2153853e3c3bc1dfa87c93eb8e173f57b55` |

The adjacent `artifacts.json` records exact filenames and digests. Linux
automatic updates target AppImage; Debian packages use manual package updates.
No stable update feed or production signing key has been configured.

## Native CI

[Final source build](https://github.com/BirthrightPublication/Atlas/actions/runs/39855440842)
builds Windows x64 NSIS, Linux AppImage/Debian, and macOS Apple Silicon/Intel
DMGs. Its Windows smoke installs the exact staged NSIS artifact, checks the
normal Start Menu launcher and bundled resources, opens and closes two native
windows, and verifies SQLite integrity and persistence in an isolated temporary
profile. All four jobs passed on the exact source revision, including tests,
native packaging, staging and artifact upload. The Windows installed-app smoke
also passed. macOS installation/startup remains unverified.

| Platform | CI artifact | Validation |
| --- | --- | --- |
| Windows x64 | [NSIS installer](https://github.com/BirthrightPublication/Atlas/actions/runs/39855440842/artifacts/12460321121) | Tests, packaging, installation, launcher, native restart and persistence |
| Linux x64 | [AppImage and Debian package](https://github.com/BirthrightPublication/Atlas/actions/runs/39855440842/artifacts/12458779356) | Tests and packaging on Ubuntu 22.04; separate Debian 13 AppImage execution described above |
| macOS Apple Silicon | [DMG](https://github.com/BirthrightPublication/Atlas/actions/runs/39855440842/artifacts/12461095789) | Native tests and packaging; installation unverified |
| macOS Intel | [DMG](https://github.com/BirthrightPublication/Atlas/actions/runs/39855440842/artifacts/12460736422) | Native tests and packaging; installation unverified |

The [Windows installer artifact](https://github.com/BirthrightPublication/Atlas/actions/runs/39855440842/artifacts/12460321121)
is an unsigned preview. The runner verified its exact staged digest before
installation. CI downloads are retained for 14 days; these are preview artifacts,
not a published GitHub release. GitHub's artifact/log redirect to
`productionresultssa18.blob.core.windows.net` is blocked in the current cloud
runtime, so independent archive download/inspection remains unavailable here.
An uploaded artifact's GitHub digest is distinct from an independently checked
installer digest.

## External checks still required

- Install and launch the macOS packages on Apple Silicon and Intel hardware;
  native runner tests and DMG packaging do not establish installed behavior.
- Authorize and exercise Proton Bridge on the owner's desktop, including its
  certificate, protected credential store and continuing incremental sync.
  Protocol fixtures do not establish a live mailbox connection.
- Install and pair the companion on a physical iPhone over the owner's trusted
  private HTTPS route; test offline/reconnect behavior in Safari/Home Screen.
- Download verified Qwen weights and run the prepared CPU benchmark. Official
  metadata/licenses and the Linux llama.cpp runtime are verified, but the model
  CDN remains blocked. No inference performance or memory result is claimed.
- Configure protected updater signing keys and OS publisher signing/notarization
  as appropriate, then validate the signed update/recovery path before enabling
  a stable feed. No verification bypass is provided.

The cloud setup/install/start instructions and narrow network additions are
saved in an environment draft. Review/save and publication are still needed to
activate that draft; fresh-task restoration has not been independently tested.
