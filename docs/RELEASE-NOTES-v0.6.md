# Birthright: Atlas 0.6.0

This release prepares the transition from the unsigned 0.5.0 preview to the
owner's permanent production update identity. Install the trusted bootstrap
once over the existing installation; subsequent compatible releases signed by
the same identity use **Updates → Update Now**.

Updater failures distinguish missing published manifests, incompatible
platforms, failed/interrupted transfers, rejected signatures and installer
failures. A failed check never means Atlas is up to date. TLS, version-bound
signatures and pre-install backups remain required.

Stays, transportation, budgets/payment history, conversations, notifications,
preferences, documents, mailbox configuration, companion state and local models
remain under the established application identity and data directory. Schema 5
is unchanged. Do not uninstall or delete data for the bootstrap transition.

Release preparation verifies the permanent key/password pair before native
builds and attaches the public identity, exact checksums, signature sidecars,
platform manifest and synthetic Windows installation evidence to a draft.
Stable publication requires owner approval. Updater signatures do not establish
Windows Authenticode or Apple notarization.

The [activation report](VALIDATION-v0.6.md) records executed evidence and keeps
real published-feed/Update Now acceptance separate from build results.
