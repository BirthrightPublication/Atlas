# Changelog

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
