# Changelog

## 0.4.0 — booking automation and travel readiness

- Add account-scoped read-only mailbox connections, guided setup, header filters,
  configurable polling, attempt/success statistics and durable batch acknowledgments.
- Keep mailbox network and text extraction work outside the application data lock.
- Replace the import drawer with a focused Import Center, field-level revision
  review, retained source evidence, atomic approval and cancellation history.
- Track refund announcements separately from verified full or partial receipts.
- Add contextual Trip Readiness, opt-in booking reminder rules and recalculation
  that retains snooze/dismiss decisions.
- Compare saved stay and transport offers, preview exact budget scenarios, prepare
  provider searches from preceding plans, and save manual watchlist snapshots.
- Extend deterministic Assistant responses and notifications with these services.
- Migrate schema 4 to 5 with a retained backup; preserve credential-store keys and
  companion identity. Extend Windows manual and controlled signed upgrade checks.

## 0.3.0 — integrated travel planning preview

- Make Settings a full workspace page with explicit save/discard navigation.
- Share one nonmodal right panel across Notifications, Updates and Assistant;
  preserve panel state across modules and load secondary content on demand.
- Persist local conversations, contextual responses, retry identity and notification
  read/dismiss state in schema 4, with pre-migration and restore backups.
- Add actionable accommodation gaps, multimonth review, tentative duplication,
  booking documents, local-time transport entry and elapsed-time calculations.
- Assess workspace evidence, alternate venue hours/commute, protected-work
  conflicts, checkout gaps, arrival deadlines and overlapping transport.
- Clarify daily/nightly/available-funds budgeting and add immutable accommodation
  extension and transport-price scenarios using the existing exact services.
- Use Times New Roman for numbers, with local serif fallbacks where unavailable.
- Show actual published release history with an offline cache; extend the trusted
  Tauri updater with measured progress, backup, duplicate protection and restart.
- Add controlled HTTPS/signed Windows upgrade checks using disposable credentials.
  Production signing and the published feed still require owner configuration.

## 0.2.0 — interface and usability preview

- Replace the warm palette throughout the desktop and companion with shared
  neutral charcoal tokens. Remove decorative gradients, blur and large shadows.
- Replace the permanent sidebar with a full-width bottom dock. Calendar,
  Discover, Map and Budget share the dock; the Settings gear sits at its right.
- Use one compact branded header with context defaults, notifications, updates
  and the Atlas Assistant. Keep imports accessible in Calendar and Budget.
- Give Calendar the full workspace width. Move clocks, work requirements,
  allowance and accommodation information into expandable horizontal summaries.
- Organize Settings into ten categories in a temporary modal. Add focus trapping,
  Escape dismissal and focus restoration to desktop dialogs; save only changed
  preference keys to preserve unrelated background updates.
- Keep the database schema, application identity, records and domain services
  compatible with 0.1.0. Add an installed Windows manual-upgrade check using
  populated synthetic 0.1 data, documents and exact accounting.
- Add browser regression coverage and screenshots at 2560×1440, 1920×1080,
  1366×768, 1024×768 and 390px widths. Preserve the companion offline queue.

## 0.1.0 — development preview

- Add native desktop shell, calendar hierarchy, booking detail and edit flows,
  discovery library, saved offers, map-provider links and budget views.
- Add SQLite records, optimistic concurrency, audit history, versioned migration,
  pre-migration backups, document integrity checks, export and restore.
- Add exact cross-month accommodation allocation, separate cash flow/deposits,
  flexible allowance and employer-timezone work scheduling.
- Add evidence-based text/EML/ICS/text-PDF imports and reviewed revision matching.
- Add persisted reminders, provider-neutral read-only email connections,
  optional local inference controls and companion pairing/offline queue.
- Add native packaging workflows, artifact checksums and signed-update tooling.

This version has outstanding live-provider, physical-device and release-platform
validation. Consult `docs/FEATURES.md` before using it for personal records.
