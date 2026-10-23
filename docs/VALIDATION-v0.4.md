# Version 0.4 validation

Candidate validation is in progress on `codex/atlas-v0.4-booking-automation`, based
on exact v0.3 head `da1d9210d492821b5dd8e3c2b19e55433f6b3c64`. This report will be
updated with executed platform results and artifact hashes after CI completes.

Local service checks cover account/message deduplication, failed-batch rollback,
header filters, cursor persistence, selective booking approvals, source history,
cancellation, expected and partial received refunds, reminder recalculation,
readiness unknowns, saved-offer scenarios, watchlist snapshots, migration and
restore. Browser checks use the real local HTTP/SQLite services and synthetic
records. No live mailbox authorization or model inference is implied.

The native pipeline retains the established Windows NSIS installation and
controlled HTTPS/signed update smoke tests, now targeting a populated v0.3 manual
upgrade to v0.4 and a disposable v0.4.1-update-test.1 restart. A defined workflow
is not evidence of success; completed run details belong below.

Production update keys/feed, live mailbox authorization, physical iPhone behavior,
and macOS installation/startup remain separately unverified owner-side checks.
