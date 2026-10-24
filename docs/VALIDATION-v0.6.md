# Version 0.6 production updater readiness

The existing permanent signing key/password pair is verified. Both the trusted
0.5.0 bootstrap and newer 0.6.0 target passed all four signed platform builds,
version-bound signature checks and populated Windows installation checks. The
exact unsigned-0.5.0 → trusted-0.5.0 transition also passed with synthetic data.
Schema 5 and application identity `org.birthright.atlas` are unchanged.

**Production Update Now acceptance remains pending.** No stable release has
been published by this work. GitHub Actions returns HTTP 403 when creating or
attaching the 0.6 release draft. The complete signed 0.6 payload is retained for
review. PR #6 still requires owner authorization before integration into `main`
and retrying the final assembly from the default branch. Publication requires
separate approval after its assets are attached and reviewed.

## Concrete builds and installer evidence

| Item                               | Actual result                                                                                                                                                                                           |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Permanent identity                 | Existing key/password signed a real version-bound proof; public key verified it                                                                                                                         |
| 0.5.0 trusted bootstrap            | [All signed jobs and draft succeeded](https://github.com/BirthrightPublication/Atlas/actions/runs/40074289786); source `f75bca66e2cf9d81747ffdf71b71249537671b4e`                                       |
| 0.6.0 target                       | [All four signed platform builds passed](https://github.com/BirthrightPublication/Atlas/actions/runs/40074275220); source `6f5b8b0a6c98b8a405743de1429ade1c5cc32154`; final draft creation denied       |
| Exact preview transition           | [Windows check passed](https://github.com/BirthrightPublication/Atlas/actions/runs/40075333953): delivered unsigned 0.5.0 installed/populated, then trusted 0.5.0 installed over it                     |
| Complete target payload            | [Verified signed release archive](https://github.com/BirthrightPublication/Atlas/actions/runs/40076217782/artifacts/12553053277), 173,839,999 bytes; no application rebuild or changed signing identity |
| Native source tests                | All 146 workspace tests and the retained test-feature compile passed locally; frontend production build passed                                                                                          |
| Windows release tools              | [Syntax, signing diagnostics and owner-helper guard passed](https://github.com/BirthrightPublication/Atlas/actions/runs/40075333905)                                                                    |
| Actual published feed / Update Now | Pending approved publication and execution of the prepared native acceptance workflow                                                                                                                   |

The permanent public-key packet SHA-256 is
`667267e572c171192ff432236e4db84f70e60d5348f8dbe81de8a9ec875c43ea`.
No permanent key was generated, replaced or read into this workspace. Signing
operations used the protected GitHub environment; diagnostic output never
contains private material.

The [complete readiness record](validation/v0.6-production-release-readiness.json)
contains real run/source IDs, all target package signatures/hashes, the actual
GitHub draft asset metadata and the review archive digest. The archive SHA-256
is `beae69146042fed05b20da4965bd053825e6ba023a2da6e0ca74f6baca90ecdb`.
It is a signed review payload and is not the stable update endpoint.

The trusted Windows 0.5.0 bootstrap is 10,193,296 bytes; SHA-256:
`e7edad26dd0046ccac90f943aa441c30a40c6798c2dac734088addd5a4705b12`.
The signed 0.6.0 Windows target is 10,192,764 bytes; SHA-256:
`f141273c289551118742615cf5032864fff2842c6448b35de290bfb4c13ff154`.
The old delivered unsigned 0.5.0 baseline was independently pinned to
`7d94963d1223cc16fc6759aaa126420b795db073c088bc6f57a6c6296dcf9614`.
These are distinct installers despite the bootstrap's intentionally equal version.

[Windows production installation](validation/v0.6-windows-production-installation.json)
and [exact same-version transition](validation/v0.6-bootstrap-transition.json)
reports record native launches/closes, launcher/resources, SQLite integrity and
retained populated records, IDs/versions/history, preferences, conversations,
notifications, documents and exact accounting. The model-cache marker survived;
this does not claim a live owner mailbox, physical iPhone or every model weight
was exercised in this transition. macOS packages built and signed; native Mac
installation/notarization remains unverified. Updater signatures are separate
from OS publisher identities.

## Root cause and fixes

The installed unsigned 0.5 preview cannot acquire a permanent public key by
changing GitHub settings. It needs one trusted bootstrap installed over the
existing location, preserving the application data directory. Do not uninstall
or delete the profile. The owner merged PR #5 and created `release-signing`;
then corrected the malformed public variable and the password rejected by the
existing private key. The final key pair verified successfully.

Signed Mac builds now explicitly select both `app` and `dmg`; DMG-only builds
were missing the required app updater archive. All four corrected signed jobs
passed. The 0.6 updater separately explains missing manifests, incompatible
platforms, transfer failures, rejected signatures and installation failures.
A failed check cannot establish that the app is up to date.

The final 0.6 draft creation/upload returns **HTTP 403: Resource not accessible
by integration** from GitHub Actions. The authorized repository API created an
empty exact-source 0.6 draft, but the Actions token still cannot attach its
assets. That placeholder is not a complete release and must not be published.
The 0.5 draft has all 20 expected assets. No unverified platform or unsigned
package is substituted to make the release appear complete.

The assembly workflow can reuse all four already successful signed jobs only
after checking the source/version and proving all application/build inputs are
identical. It re-verifies checksums and signatures, retains the complete payload,
and never overwrites a populated or published release. It may attach only to
an empty exact-source draft. This avoids expensive rebuilds while resolving
repository authorization. See [owner setup](OWNER-UPDATE-SETUP.md).

## Real published acceptance prepared, not yet executed

The manually dispatched [acceptance workflow](../.github/workflows/verify-production-update.yml)
uses the public key and read access. It requires the owner-approved 0.5.0 and
0.6.0 releases to be published, verifies the actual HTTPS manifest/assets and
same-identity bootstrap, and installs that exact bootstrap into an isolated
Windows profile. Playwright connects to its native WebView2 on loopback and
clicks the normal **Update Now** button. It uses no test update feature, mocked
IPC, alternate feed, private signing key or direct install-command invocation.

It captures real progress/screenshots, observes restart, verifies 0.6.0 and a
successful no-new-update check, compares retained synthetic records/history,
settings, documents, chats, notifications, exact accounting, model-cache marker
and companion metadata, and requires a new nonempty pre-install backup. A
failed/unresolved installer deliberately retains owned temporary junctions to
prevent a later child from touching real known-folder paths.

Preparation and Windows syntax checks do not count as that acceptance. Earlier
negative disposable-feed cases remain historical in [v0.5 validation](VALIDATION-v0.5.md).
No claim is made that publication, actual in-app upgrade, live mailbox/device
checks or all production failure/recovery scenarios have passed.
