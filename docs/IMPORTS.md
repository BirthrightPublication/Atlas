# Booking Import Center

Open Import booking from Calendar, Booking Import Center from Discover, a booking
notification, or Settings → Email & Integrations → Sync & review. It replaces the
main workspace temporarily; the bottom dock and utility panels remain available.

Pending Review holds new reservations, Changes holds matched revisions,
Processed retains accepted drafts and refund notices, Dismissed retains rejected
imports, and Connections opens account settings. Paste/upload remains available
for text, MIME EML, ICS and text-bearing PDFs. Scanned images are attachments,
not OCR results. Saving a URL does not retrieve private reservation information.

Review fields shows known incoming values and saved values, evidence and missing
information. For revisions, uncheck fields to retain their saved values. Edit
fields opens the existing booking editor with saved facts merged into the draft.
Accept uses the booking version seen in the review; a concurrent edit requires
refreshing and reviewing again. Missing fields never erase prior facts. Dates,
timezones and prices still must pass ordinary record validation. Parsed mode and
other derived fields are labeled inferred/review instead of being called sourced.

Review planning effects before approval: timing and work compatibility, linked
records and history remain available. Accepted bookings use the same Calendar,
Journey and budget records as manual entry. Approvals, sources and import identity
are transactional. Replayed messages do not produce duplicate drafts within an
account, and multiple booking blocks in a message remain separate drafts.

Cancellation requires sourced text and explicit approval. Mailbox deletion is
never a cancellation signal. Accepted cancellation retains the record, audit
history and original payments, while removing the booking from forecasts.

A refund email creates a refund notice linked to the booking. Provider claims of
receipt still require the owner to verify actual money received. In Processed,
select the original settled payment, enter the actual amount/date, confirm
receipt, and record it. Partial receipts retain the outstanding notice and
request identity prevents duplicate retry settlement. Original cash history is
preserved. Cross-currency receipts need the existing payment editor's explicit
historic currency basis; the notice workflow does not invent an exchange rate.

The parser recognizes explicit labeled booking fields and bounded MIME/ICS/PDF
content; it cannot reliably decode every provider's prose. Unknown information
needs manual correction. Optional local AI requires a separately verified model
and runtime and cannot execute imported instructions or approve a booking.
