# Development preview status

This document distinguishes source implementation from runtime and release
validation. Checks used the Linux cloud workspace and native CI runners.

Version 0.5.0 preserves the approved neutral theme, bottom dock, full-page
Settings and shared nonmodal utility panel. Assistant chats and actionable
notification state persist in schema 5. Numbers use local Times
New Roman, falling back to Liberation Serif/Times when it is unavailable.
See [v0.5 validation](VALIDATION-v0.5.md) for current evidence; the
[v0.2](VALIDATION-v0.2.md) and [v0.1](VALIDATION.md) reports remain historical.
The [v0.6 activation report](VALIDATION-v0.6.md) tracks the merged default-branch
workflow and the remaining malformed owner public-key setting separately.

| Area                        | Implemented behavior                                                                                                                       | Validation / limitation                                                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| Persistent records          | SQLite, stable IDs, versions, relationships, history, local files                                                                          | Restart, conflict, migration, document backup/restore tests pass                                                                       |
| Calendar                    | Stay bands, objectives, compact transport, month/week/day/journey, context creation defaults                                               | Chromium acceptance passes at desktop and phone widths                                                                                 |
| Money                       | Exact night allocations, original currency, cash flow, deposits/refunds, allowance, period averages                                        | Cross-month prepaid stay, currencies, refunds and allowance tests pass                                                                 |
| Work                        | IANA timezone/DST, employer days, overlap, exceptions, breaks, recorded conflicts                                                          | Timezone/DST and interval tests pass; connectivity/venue hours remain owner-supplied evidence                                          |
| Imports                     | Text, EML, ICS, text-bearing PDF, evidence, review and revision identity                                                                   | Real SQLite acceptance and parser tests pass; scanned PDFs need external OCR                                                           |
| Reminders                   | Persistent due instants, snooze/dismiss, catch-up                                                                                          | Restart/delivery tests pass; delivery needs running host; no closed-iPhone push claim                                                  |
| Email                       | Read-only Proton Bridge/IMAP, Gmail and Outlook adapters, cursors, reconnect and OS-protected secrets                                      | Protocol/fixture tests; no live mailbox or native keychain authorization yet. Proton Bridge needs a paid plan                          |
| Discovery                   | Editable website library, exact-date offer ranking, verified requirements, justified stretch budgets, linked journey costs, provider links | Saved-evidence comparison and immutable extension tests pass; no live travel inventory or authorized scheduled monitoring connected    |
| Maps                        | Saved location directory and external provider directions                                                                                  | Offline map tiles / live route computation not implemented                                                                             |
| Recurrence                  | Daily/weekly/monthly, interval, until, exceptions and per-occurrence completion                                                            | Expansion and local-time tests pass; advanced RRULE components explicitly rejected                                                     |
| Local AI                    | Optional supervised verified runtime, pinned artifact installer, one job, caps/cancel/idle release, schema/evidence checks                 | v0.5 actual 0.6B native download, inference and lifecycle checks pass; small-model extraction needs review; 1.7B unbenchmarked         |
| Companion                   | Authenticated pairing/revocation, limited services, IndexedDB snapshot/queue, version conflicts, opt-in documents                          | Host security and real Chromium offline tests pass; trusted private HTTPS and physical iPhone unverified                               |
| Desktop                     | Tauri native IPC/window, OS notifications, system-browser links, managed companion host                                                    | v0.5 passed three Linux packaged launches/closes, Windows NSIS installation and populated v0.4 upgrades on both platforms              |
| Releases                    | Windows NSIS, Linux AppImage/deb and macOS build pipelines; exact artifact metadata                                                        | All four native CI jobs passed; exact installer hashes are in the v0.5 report; macOS installation unverified                           |
| Updates                     | Compiled key + HTTPS feed, real progress, backup, signature/version/target verification, install/restart                                   | Seven v0.5 Windows controlled update cases passed, including signed installation/restart; production signing/feed requires owner setup |
| Assistant and notifications | Local chat sessions, contextual deterministic answers, safe navigation, durable read/dismiss state                                         | Service/restart and browser checks pass; actual optional 0.6B inference passed on Linux and Windows                                    |
| Travel planning             | Night gaps, tentative drafts, date-line duration, workspace evidence, alternate hours/commute, immutable scenarios                         | Real core service checks; no live route or venue information                                                                           |

## Version 0.3 additions and validation

The existing services remain in place. New Rust services persist chats and
notifications, evaluate accommodation gaps and workspace evidence, preview
travel conflicts and estimate transport scenarios without saving commitments.
Settings handles dirty navigation explicitly. Utility panels retain session and
composer state across module changes; chat/notification histories are paginated.
Release history comes only from published GitHub releases, with a local cache.
The installer remains authorized exclusively by the compiled key and HTTPS
Tauri update feed. Actual transfer bytes and verification phases drive progress.

The historical v0.3 test and platform results are tracked in
[the v0.3 report](VALIDATION-v0.3.md). A defined workflow is not proof of an
installed update. Disposable update-test keys do not establish production
signing. No live prices, availability, routes or workspace evidence are invented.

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
iPhone. Official model/runtime hashes are pinned; the 0.6B model has executed
with measured inference and lifecycle checks. The 1.7B model and owner-machine
performance remain separately unverified. macOS packages still need native
installation/startup checks. Signed updates need protected signing
keys, release artifacts and an activated feed. No secrets belong in chat or Git.

## Version 0.4 additions

Up to 12 local mailbox account slots retain separate protected credentials and
incremental cursors. The original account keeps its v0.3 credential-store key.
IMAP header filtering and Graph metadata filtering precede MIME retrieval;
Gmail uses provider search plus metadata checks. Exact sender/domain matching,
exclusions, subject keyword groups and date bounds are editable. Successful
batch acknowledgment follows durable source/draft persistence; filter changes
restart scanning with account-scoped identity deduplication. Attempts back off
from the last attempt, including failures. Native network/text parsing workers do
not hold the main SQLite mutex. Mailbox authorization remains unverified against
an owner's live account; fixture tests are not a substitute for that check.

The focused Import Center has Pending Review, Changes, Processed, Dismissed and
Connections views. Source evidence, before/after fields and planning previews
precede explicit acceptance. Omitted fields preserve existing facts. Accepted
cancellations retain payments and audit history. Refund notices have announced,
expected and received states; partial receipts link to the original payment and
only explicit actual receipts create settled negative payments. Cross-currency
refund receipts require the existing payment editor and manual reconciliation.

Readiness derives confirmation, workspace, document, gap, work, deadline,
review-draft and refund issues from current records. It reports unknown critical
requirements instead of declaring readiness. Suggestions and optional automatic
booking rules use timezone-aware instants. Pending reminder times follow booking
revisions; snoozed/dismissed choices are retained and cancellations suppress
irrelevant deliveries. No phone push while hosts are offline is claimed.

Discovery compares saved evidence for matching dates/currencies, known mandatory
fees, facilities, freshness and recorded transport costs. Scenarios call the real
accounting services without saving bookings or payments. Prepared searches open
Airbnb, Booking.com and Rome2Rio; the broader provider directory remains. Manual
watchlist snapshots and recheck reminders do not represent automated price feeds.
Optional AI remained independent; actual model inference was unverified in v0.4.

## Version 0.5 evidence boundaries

Native Rust model installation and explicit downloads replace the developer
Python requirement for installed Atlas. Catalog, approval, extraction and forged
inventory checks pass; actual inference is reported separately. Local AI only
explains authorized planner results. The Planner and Local AI modes retain chats.
Tentative multi-stop journeys, route drafts, individually reviewed schedules and
selected offline trip packages use real persistence/calculation services. The
companion Trip view reads synchronized records and chosen documents offline.

Production updating is not yet activated: owner-controlled keys, approved default
branch integration, a signed bootstrap and approved stable publication are
required. See [owner setup](OWNER-UPDATE-SETUP.md) and [explicit completion
categories](VALIDATION-v0.5.md). Historical v0.4 native validation remains valid
for that version and is not substituted for v0.5 or production acceptance.
