# Development preview status

This document distinguishes source implementation from runtime and release
validation. The initial build is being validated in a Linux cloud workspace.

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
| Local AI | Optional supervised verified runtime, one job, caps/cancel/idle release, schema/evidence checks | Validation tests pass; no model downloaded, inferred or CPU-benchmarked yet |
| Companion | Authenticated pairing/revocation, limited services, IndexedDB snapshot/queue, version conflicts, opt-in documents | Host security and real Chromium offline tests pass; trusted private HTTPS and physical iPhone unverified |
| Desktop | Tauri native IPC/window, OS notifications, system-browser links, managed companion host | Optimized Linux executable built and native window rendered with sandbox intact; packaging checks in progress |
| Releases | Windows NSIS, Linux AppImage/deb and macOS pipeline definitions; exact artifact metadata | Native runner results and Windows/macOS installation not yet verified |
| Updates | Compiled verification key + HTTPS feed, backup, signature verification, install/restart | Cryptographic verification tests pass; owner signing setup and published feed absent |

## Validation in this workspace

The current completed run passed 92 core unit tests, four SQLite acceptance
journeys, six service-boundary tests, seven companion-host tests, and five real
Chromium browser tests. Empty binary/doc-test targets are not counted. Frontend
type checking and production bundling passed. Native Linux launch used a
read-only view of the verified GTK/WebKit SDK, an isolated temporary user-data
directory and Xvfb; WebKit's sandbox remained enabled. These results do not
establish physical iPhone behavior or Windows/macOS installation.

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

The owner must select a supported mailbox setup, authorize it in the native app,
and supply any account registration settings there. A free Proton account cannot
use Bridge. Validate private HTTPS, pairing, airplane-mode reads/edits and
reconnection on an actual iPhone. Model files/runtime need authoritative hashes
and a measured CPU evaluation before choosing a default. Windows/macOS packages
need native installation/startup checks. Signed updates need protected signing
keys, release artifacts and an activated feed. No secrets belong in chat or Git.
