# Birthright: Atlas

A local-first travel planner for stays, transport, objectives, work schedules,
and money. The native desktop application uses Tauri 2, React/TypeScript, Rust
services, and SQLite. An optional paired phone companion shares the same service
boundary and keeps an explicit offline edit queue.

**Version 0.5.0: development candidate.** Connect read-only booking mailboxes,
review reservations and revisions in the Import Center, compare saved offers,
and check Trip Readiness. The approved neutral dark interface, bottom dock,
persistent chats, full-page Settings and Times New Roman numerals remain.
Prepare permanent updates with the [owner activation guide](docs/OWNER-UPDATE-SETUP.md).
Production updating requires owner signing, integration and publication; an unsigned CI download does not activate it.
See [v0.5 validation](docs/VALIDATION-v0.5.md), [release notes](docs/RELEASE-NOTES-v0.5.md)
and [feature status](docs/FEATURES.md) for actual evidence and remaining owner-side
mailbox, device and production-signing checks. Historical validation reports remain available.

## Start development

Install Node 24.19.0, Rust 1.90.0, and the native prerequisites in
[BUILD.md](docs/BUILD.md). From this checkout:

```sh
npm ci
npm run build
cargo test --workspace --locked
npm run desktop
```

For browser development, run these in separate terminals:

```sh
cargo run --locked -p atlas-host -- --dev --data-dir /tmp/atlas-development
npm run dev
```

The browser host binds loopback only; `--dev` grants full local application
access and must never be exposed to a network. The desktop uses native IPC and
does not need this development server. In the prepared cloud workspace, first
run `source scripts/cloud-env.sh` in each Rust terminal.

## What to try

1. Open **Import booking**, choose the clearly labeled synthetic confirmation,
   review the extracted dates, currency, facilities and source evidence, then
   accept it.
2. Inspect the stay band, month/week/day agenda, journey timeline and booking
   detail drawer. Changing **On Foot / On Road** changes new-entry defaults.
3. Record a payment separately from the stay; inspect month-of-use costs and
   payment-date cash flow. Set income, reserve and flexible allowance yourself.
4. Add an objective, protect work time, and create a persisted reminder. Restart
   the app and verify that the records remain.
5. Import a revised confirmation. Its reservation identity links the review to
   the existing booking, with optimistic version checks and change history.

The application starts empty. Synthetic samples are inserted only when selected.
Personal preferences, databases, messages, documents and models belong in local
user-data directories, not this public repository. SQLite and backups are not
encrypted by the application; use operating-system disk encryption. Mailbox
credentials use the OS secret store and fail closed if it is unavailable.

## Guides

- [Journey planning and offline preparation](docs/JOURNEYS.md)
- [Activate the production updater](docs/OWNER-UPDATE-SETUP.md)
- [Desktop builds and toolchain](docs/BUILD.md)
- [Email setup and provider limitations](docs/EMAIL.md)
- [iPhone companion, pairing and private HTTPS](docs/COMPANION.md)
- [Optional local inference and verified models](docs/AI.md)
- [Recurring objectives and work blocks](docs/RECURRENCE.md)
- [Version 0.5 release notes](docs/RELEASE-NOTES-v0.5.md)
- [Booking imports, revisions and refunds](docs/IMPORTS.md)
- [Saved offers, comparisons and watchlists](docs/DISCOVERY.md)
- [Release packaging and signed updates](docs/RELEASE.md)
- [Application service contract](docs/SERVICE-CONTRACT.md)
- [Validation and feature status](docs/FEATURES.md)
- [Tested builds and artifact evidence](docs/VALIDATION-v0.5.md)

No application license has been selected. Public repository visibility does not
grant a software license. Third-party components retain their own licenses and
notices; see the dependency inventory generated for release builds.
