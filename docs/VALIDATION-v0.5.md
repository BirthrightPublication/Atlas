# Version 0.5 validation

This candidate is based on completed v0.4 commit
`190c4a1f57bad404bb9cb3b8e4b14ef698b2f008`. The integration PR targets `main` so
owner approval can place the application and manually runnable release workflow
on the default branch while preserving the stacked history. No merge or stable
publication is performed without that approval.

## Implemented and tested

Local checks pass 143 Rust tests, including reviewed journey persistence,
hypothetical cross-month costs, confirmed-date preservation, stale schedule
rejection, explicit offline documents, pinned catalog approval gates, forged
inventory rejection and unsafe archive paths. The frontend production build,
five Node money/signature/controlled-feed tests and two Python helper security
checks pass. Browser results and native artifact records are being completed.

The actual retained v0.4 host seeded nine records, a document, conversations,
notifications, settings and a model-cache marker. The v0.5 host retained them
exactly, with stable IDs, versions/history, model-cache marker SHA-256, schema 5,
USD 600/300 month-of-use accounting and USD 900 payment-month cash flow. SQLite
integrity passed. No schema migration is required for this version.

## Installed and tested on Windows

Pending completed candidate CI. Its Windows job performs the exact NSIS
installation, populated v0.4 manual upgrade, native launcher/restart checks and
seven controlled update cases. A workflow definition is not execution evidence.
Exact run IDs, installer SHA-256 and reports belong here after completion.

## Verified against a real published release

**Pending.** The repository default branch initially lacked the workflow;
`release-signing` initially did not exist, and no stable Atlas release existed.
Owner setup, approved integration, production-key-bearing bootstrap and approved
subsequent publication are required. `scripts/verify-published-release.mjs`
verifies the public manifest, exact artifact mapping, SHA-256 and signatures;
actual Windows Update Now acceptance is still separate. No disposable test or
ordinary unsigned candidate establishes production activation.

## Tested only with disposable development credentials

Controlled native update evidence will be recorded after the Windows job.
Its CA/signing identity is separate from production, and normal TLS/signature
verification remains enabled. No temporary Actions download is a stable feed.

## Requires owner authorization/configuration

Follow [OWNER-UPDATE-SETUP.md](OWNER-UPDATE-SETUP.md). Owner private keys stay on
the trusted owner machine and in protected GitHub environment secrets. Integrate
the tested PR, run signed release preparation, review the draft, install the
trusted bootstrap once, approve publication and perform the actual 0.5.0→0.5.1
Update Now acceptance with populated data. Physical iPhone, live mailbox and
macOS installation remain separate external checks.

## Local AI evidence and limits

The native cloud download was actually attempted with explicit approval and
failed at the pinned download endpoint. No unverified bytes were installed and
no local inference latency is claimed. Windows CI separately evaluates the
real native installer with a synthetic prompt and records measured results or
an explicit blocked status. It does not require Python and does not bundle
weights in releases. Model installation code/fixtures are distinct from actual
successful inference.

## Not implemented

Offline routing/map tiles, invented live fares/availability, hosted AI and
closed-app iPhone push are not implemented. Local AI provides explanations,
not unrestricted tools or authoritative financial/time calculations. No Windows
Authenticode or Apple notarization identity is configured by updater signatures.
