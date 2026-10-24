# Version 0.5 validation

This candidate is based on completed v0.4 commit
`190c4a1f57bad404bb9cb3b8e4b14ef698b2f008`. The integration PR targets `main` so
owner approval can place the application and manually runnable release workflow
on the default branch while preserving the stacked history. No merge or stable
publication is performed without that approval.

## Implemented and tested

Local checks pass 144 Rust tests, including reviewed journey persistence,
hypothetical cross-month costs, confirmed-date preservation, stale schedule
rejection, explicit offline documents, pinned catalog approval gates, forged
inventory rejection and unsafe archive paths. The frontend production build,
five Node money/signature/controlled-feed tests and two Python helper security
checks pass. All 25 Chromium journeys passed. The five new visual journeys were
rerun after correcting the narrow-screen summaries; all passed. Native artifact
records are linked below. The charcoal theme, dock, utility panels and numeric serif
face were inspected at all five required viewport sizes.

The actual retained v0.4 host seeded nine records, a document, conversations,
notifications, settings and a model-cache marker. The v0.5 host retained them
exactly, with stable IDs, versions/history, model-cache marker SHA-256, schema 5,
USD 600/300 month-of-use accounting and USD 900 payment-month cash flow. SQLite
integrity passed. No schema migration is required for this version.

## Packaged Linux checks

The final local AppImage launched three times and closed normally three times,
all exit code 0. Native Model Manager, journey form and the reopened Planner
conversation were inspected. After every close the populated v0.4 profile
retained its nine records, document hash, IDs/versions/history, settings, chats,
notifications, model-cache marker and exact accounting; SQLite integrity passed.
See [native evidence](validation/v0.5-linux-smoke.json), [local artifact hashes](validation/v0.5-linux-local-artifacts.json),
[native model controls](screenshots/v0.5/native-linux-local-ai.png),
[journey form](screenshots/v0.5/native-linux-journey.png) and
[restored chat](screenshots/v0.5/native-linux-restored-chat.png).
WebKit sandboxing stayed enabled. This Debian 13 SDK check does not establish
older distribution compatibility; the deb was built but not installed here.

## Installed and tested on Windows

All four native jobs passed in [run 40061296428](https://github.com/BirthrightPublication/Atlas/actions/runs/40061296428) from source
`107e7260abeaa65aa5414c67bb3607f4fdad7c53`. Local packages used application source
`06a739d075779ca9056b5de721b798a490538ea7`; the later CI commit changes only the
release workflow, draft packaging and owner documentation. Application code is
identical. Later evidence-only commits retain these exact source references.

The exact Windows NSIS installer passed installation, packaged resources, Start
Menu launcher, native startup/clean close/restart, SQLite integrity and the
populated v0.4→v0.5 manual upgrade. Nine records, documents, conversations,
notifications, preferences, financial history and the model-cache marker were
retained. See the [installation report](validation/v0.5-windows-installation-smoke.json).

[Download the Windows development candidate](https://github.com/BirthrightPublication/Atlas/actions/runs/40061296428/artifacts/12548057676) and extract
`windows-x86_64/Birthright-Atlas_0.5.0_windows_x86_64-setup.exe`.
The installer is 10,190,568 bytes; SHA-256:
`7d94963d1223cc16fc6759aaa126420b795db073c088bc6f57a6c6296dcf9614`.
It needs no Rust, Node.js or Python development environment. It is an unsigned
review candidate, not the production-key-bearing bootstrap. That bootstrap is
built by the protected signed-release workflow after owner activation.

| Platform | Candidate archive | Executed evidence |
| --- | --- | --- |
| Windows x64 | [Download](https://github.com/BirthrightPublication/Atlas/actions/runs/40061296428/artifacts/12548057676) | Installer, services, populated upgrade and controlled signed restart |
| Linux x64 | [Download](https://github.com/BirthrightPublication/Atlas/actions/runs/40061296428/artifacts/12547158832) | Services and Ubuntu 22.04 AppImage/deb packaging; separate local native checks above |
| macOS Apple Silicon | [Download](https://github.com/BirthrightPublication/Atlas/actions/runs/40061296428/artifacts/12547971497) | Services and DMG packaging |
| macOS Intel | [Download](https://github.com/BirthrightPublication/Atlas/actions/runs/40061296428/artifacts/12547293865) | Services and DMG packaging |

The [complete CI record](validation/v0.5-native-ci.json) includes every package
filename, length, SHA-256, archive digest and job URL. Reports/manifests came
from authenticated GitHub check annotations and Actions metadata. Artifacts
are retained for 14 days and are not the stable update feed. macOS installation
and notarization remain unverified.

## Verified against a real published release

**Pending.** The repository default branch initially lacked the workflow;
`release-signing` initially did not exist, and no stable Atlas release existed.
Owner setup, approved integration, production-key-bearing bootstrap and approved
subsequent publication are required. `scripts/verify-published-release.mjs`
verifies the public manifest, exact artifact mapping, SHA-256 and signatures;
actual Windows Update Now acceptance is still separate. No disposable test or
ordinary unsigned candidate establishes production activation.

## Tested only with disposable development credentials

The [controlled native report](validation/v0.5-controlled-update-smoke.json)
passed all seven cases: older release, wrong platform, invalid signature,
interrupted download, malformed manifest, missing release and valid update.
The valid signed NSIS package installed and restarted the real application at
0.5.1-update-test.1, preserving populated data and managed companion pairing.
Private test keys were disposable; TLS and signature validation stayed enabled.
These results do not activate production signing or establish a public feed.

## Requires owner authorization/configuration

Follow [OWNER-UPDATE-SETUP.md](OWNER-UPDATE-SETUP.md). Owner private keys stay on
the trusted owner machine and in protected GitHub environment secrets. Integrate
the tested PR, run signed release preparation, review the draft, install the
trusted bootstrap once, approve publication and perform the actual 0.5.0→0.5.1
Update Now acceptance with populated data. Physical iPhone, live mailbox and
macOS installation remain separate external checks.

## Local AI evidence and limits

The first native attempt exposed missing OS certificate roots. The final build
uses the normal OS trust store alongside the embedded public roots; TLS and
hostname verification remain enforced. The actual pinned 639,446,688-byte Qwen
0.6B model and runtime then downloaded, verified and installed through Rust.
[Native installation/explanation report](validation/v0.5-local-ai-evaluation.json)
records successful cold and warm responses and explicit process shutdown.

The release-mode [extraction/lifecycle benchmark](validation/v0.5-ai-extraction-benchmark.json)
ran three synthetic confirmations: 11,449 ms cold, 8,802 ms and 9,359 ms warm
wall time. Cold means a stopped runtime; files may already be OS-cached.
Automatic idle shutdown at the default 120-second limit passed.
Cancellation returned an error and stopped the runtime in 110 ms. This cloud
has a four-CPU quota; inference remained capped at two threads. Peak RSS could
not be read in this namespace and is not reported as zero memory usage. The
separate debug-host explanation timings are not release performance estimates.

These checks establish actual inference and lifecycle behavior, not reliable
booking extraction. The small model proposed incorrect reservation IDs and
unsupported amounts; validation rejected them. Ambiguous field classifications
still require review. Ordinary parsers remain primary and a model cannot commit
facts. The 1.7B catalog entry is verified provenance, but not benchmarked here.
Windows CI separately passed native installation/inference and records its
own measured results below. Neither model weights nor a Python
toolchain are bundled in application releases.

## Not implemented

Offline routing/map tiles, invented live fares/availability, hosted AI and
closed-app iPhone push are not implemented. Local AI provides explanations,
not unrestricted tools or authoritative financial/time calculations. No Windows
Authenticode or Apple notarization identity is configured by updater signatures.

## Windows Local AI evaluation

Windows also downloaded, verified and enabled the real pinned model through the native host. Its synthetic cold and warm explanation wall times were 3,777 ms and 1,271 ms. Explicit shutdown reported running=false.
See the [native Windows AI report](validation/v0.5-ai-installation-benchmark.json).
These are CI-machine measurements, not owner-hardware performance promises.
