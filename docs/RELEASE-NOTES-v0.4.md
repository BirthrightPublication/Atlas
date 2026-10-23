# Birthright: Atlas 0.4.0

This development candidate adds connected booking automation while preserving the
neutral theme, bottom dock, full Settings, utility panels and Times New Roman
numerals from v0.3.

- Connect multiple read-only mailboxes with guided provider setup, selective
  retrieval, sync statistics, pause/reconnect and OS-protected credentials.
- Review new bookings, revisions and cancellations in a focused Import Center,
  choose individual changed fields, and retain evidence and audit history.
- Track promised refunds separately from explicitly verified partial/full receipts.
- Compare saved accommodation and transport offers, preview budget choices,
  prepare searches from preceding plans, and record manual watchlist snapshots.
- Review Trip Readiness and adopt timezone-aware booking reminders; optional rules
  follow accepted changes without overriding snooze/dismiss choices.
- Ask Atlas Assistant about booking reviews, cancellations, payments, saved offers
  and readiness using deterministic local services.

Schema 4 upgrades to 5 with a pre-migration backup. Existing account credentials,
companion pairing, documents, chats, notifications and money history retain their
established locations. See [validation](VALIDATION-v0.4.md) for executed tests,
native packages and upgrade evidence.

Live provider authorization, physical iPhone validation and optional model
inference remain owner-side checks. Production signing and the stable HTTPS
update feed require owner configuration. Preview builds never bypass signature
verification. No stable release is published by this development build.
