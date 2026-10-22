# Development preview status

This document distinguishes source implementation from runtime and release
validation. Checks used the Linux cloud workspace and native CI runners.

| Area | Implemented behavior | Validation / limitation |
| --- | --- | --- |
| Persistent records | SQLite, stable IDs, versions, relationships, history, local files | Restart, conflict, migration, document backup/restore tests pass |
| Calendar | Stay bands, objectives, compact transport, month/week/day/journey, context creation defaults | Chromium acceptance passes at desktop and phone widths |
| Money | Exact night allocations, original currency, cash flow, deposits/refunds, allowance, period averages | Cross-month prepaid stay, currencies, refunds and allowance tests pass |
| Work | IANA timezone/DST, employer days, overlap, exceptions, breaks, recorded conflicts | Timezone/DST and interval tests pass; connectivity/venue hours remain owner-supplied evidence |
| Imports | Text, EML, ICS, text-bearing PDF, evidence, review and revision identity | Real SQLite acceptance and parser tests pass; scanned PDFs need external OCR |
| Reminders | Persistent due instants, snooze/dismiss, catch-up | Restart/delivery tests pass; delivery needs running host; no closed-iPhone push claim |
| Email | Read-only Proton Bridge/IMAP, Gmail and Outlook adapters, cursors, reconnect and OS-protected secrets | Protocol/fixture tests; no live mailbox or native keychain authorization yet. Proton Bridge needs a paid plan |
| Discovery | Editable website library, exact-date offer ranking, verified requirements, justified stretch budgets, linked journey costs, provider links | Saved-evidence comparison and immutable extension tests pass; no live travel inventory or authorized scheduled monitoring connected |
| Maps | Saved location directory and external provider directions | Offline map tiles / live route computation not implemented |
| Recurrence | Daily/weekly/monthly, interval, until, exceptions and per-occurrence completion | Expansion and local-time tests pass; advanced RRULE components explicitly rejected |
| Local AI | Optional supervised verified runtime, pinned artifact installer, one job, caps/cancel/idle release, schema/evidence checks | Official Qwen metadata/licenses and llama.cpp archive verified; model CDN blocked, so no model inference or CPU benchmark claimed |
| Companion | Authenticated pairing/revocation, limited services, IndexedDB snapshot/queue, version conflicts, opt-in documents | Host security and real Chromium offline tests pass; trusted private HTTPS and physical iPhone unverified |
| Desktop | Tauri native IPC/window, OS notifications, system-browser links, managed companion host | Linux AppImage/deb built; actual AppImage saved/reopened a stay and closed cleanly with sandbox intact |
| Releases | Windows NSIS, Linux AppImage/deb and macOS build pipelines; exact artifact metadata | Tests and packaging pass on all four native targets; Windows installation/restart passes; macOS installation unverified |
| Updates | Compiled verification key + HTTPS feed, backup, signature verification, install/restart | Cryptographic verification tests pass; owner signing setup and published feed absent |

## Validation in this workspace

The final service run passed 94 core unit tests, four SQLite acceptance
journeys, six service-boundary tests, seven companion-host tests, and five real
Chromium browser tests. Empty binary/doc-test targets are not counted. Frontend
type checking and production bundling passed. Native Linux launch used a
read-only view of the verified GTK/WebKit SDK, an isolated temporary user-data
directory and Xvfb; a stay saved through the native UI survived window restart,
and WebKit's sandbox remained enabled. Windows CI also installed its NSIS
artifact, checked the normal launcher, opened/closed two native windows and
verified SQLite integrity and restart persistence. These results do not
establish physical iPhone behavior, macOS installation or installation on the
owner's computer.

See [validation evidence and artifact digests](VALIDATION.md) for the tested
source commit, platform checks and remaining external dependencies.

## Accounting semantics

Checkout is exclusive. A 30-night USD 900 stay with 20 December nights and 10
January nights contributes USD 600 and USD 300 to those month-of-use views. A
November prepayment contributes USD 900 to November cash flow. Refundable
deposits reduce available cash until returned without becoming ordinary consumed
accommodation cost. Original and settled currencies are preserved; manual
exchange-rate scenarios do not mutate balances or paid history.

## Data and security boundaries

The database, documents and backups are plain local files protected by the OS
account and optional OS disk encryption. The application does not advertise
SQLite encryption. Mailbox secrets use the OS credential store. A phone can
retain its explicitly synchronized snapshot and selected documents; revocation
prevents new server access but cannot remotely erase an offline device's cache.
The companion settings include local cache removal. Email images are not loaded,
and saved listing URLs are not silently scraped.

## Remaining release prerequisites

Authorize the selected mailbox setup in the native app and supply account
registration settings there. A free Proton account cannot use Bridge. Validate
private HTTPS, pairing, airplane-mode reads/edits and reconnection on an actual
iPhone. Official model/runtime hashes are pinned; model bytes and a measured CPU
evaluation remain blocked by model-CDN access. macOS packages still need native
installation/startup checks. Signed updates need protected signing
keys, release artifacts and an activated feed. No secrets belong in chat or Git.
