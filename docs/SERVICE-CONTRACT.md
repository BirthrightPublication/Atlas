# Atlas application service contract

The desktop and companion invoke the same Rust service. Native IPC command:
`atlas_call({ method, payload })`. HTTP development/companion transport:
`POST /api/rpc` with `{ "method": "…", "payload": {} }`.
Both return the result JSON directly; HTTP errors return `{ "error": "…" }`.
Desktop commands reject with a readable error. No secrets in responses.

Rust entry point: `atlas_core::Atlas::open(data_dir: impl AsRef<Path>) -> Result<Atlas, String>`;
`atlas.call(method: &str, payload: serde_json::Value) -> Result<Value, String>`.
An Atlas instance is held behind a mutex by the host. Core SQLite data lives in
the OS user data directory. Tests explicitly use temporary directories.

## Records

Records use camelCase fields and UUID IDs. Every record has `id`, `kind`,
`createdAt`, `updatedAt`, `version`. Record kinds: `trip`, `stay`, `transport`,
`objective`, `workBlock`, `payment`, `expense`, `reminder`, `place`, `document`,
`quote`, `watchlist`, `budget`, `connection`, `source`, `draft`.

`records.list {kind?}` returns an array. `records.save {record, expectedVersion?}`
returns the saved record; version mismatch is a conflict. `records.delete {id,
expectedVersion?}` deletes a record and returns `{deleted:true}`. `records.history
{id}` returns audit entries. `snapshot {}` returns `{records, settings}`.

Stay: `name, type, tripId?, placeId?, address, latitude?, longitude?, timezone,
checkIn (YYYY-MM-DD), checkOut (exclusive date), checkInTime, checkOutTime,
context (on_foot|on_road), status (tentative|confirmed|cancelled), currency,
totalMinor (integer), provider, reservationId, url, hostUrl, contact, notes,
facilities {refrigerator: true|false|null, workspace: true|false|null,
kitchen: true|false|null, internet: true|false|null}, cancellationAt?,
priceComponents?, paymentSchedule?, documentIds?`.
Transport: `name, mode, origin, destination, departure, arrival` (ISO instants),
`departureTimezone, arrivalTimezone, overnight (bool), currency, totalMinor,
status, context, reservationId?, provider?, tripId?, stayId?, notes?`.
Objective: `name, category, stayId?, placeId?, date?, startTime?, endTime?,
timezone, status (planned|done), notes?, url?, currency?, totalMinor?, recurrence?`.
Payment: `bookingId, date, currency, amountMinor` (positive payment, negative
refund), `type (payment|deposit|refund|deposit_refund), status (planned|settled)`.
Expense: `name, date, currency, amountMinor, category`.
Reminder: `name, bookingId?, dueAt` (RFC3339), `state
(pending|delivered|dismissed|snoozed), catchUp (deliver|skip), deliveredAt?`.
Budget: `month (YYYY-MM), currency, incomeMinor, openingBalanceMinor,
reserveMinor, allowanceMinor, recurringExpenses [{name,amountMinor}]`.

## Methods

* `settings.get {}` -> object; `settings.save {settings}` -> merged settings.
  Settings include `currency, context, officeTimezone, currentTimezone,
  currentLocation, workStart, workEnd, workHours, overlapHours, workdays,
  preferences` (editable personal presets, no assumed personal income).
* `budget.summary {month, currency?}` -> `{month,currency,daysInMonth,
  accommodationMinor,transportMinor,objectivesMinor,expensesMinor,
  recurringMinor,forecastMinor,cashFlowMinor,settledCashFlowMinor,incomeMinor,
  openingBalanceMinor,reserveMinor,availableMinor,allowanceMinor,
  allowanceRemainingMinor,remainingDays,dailyForecastMinor,
  dailyAllowanceMinor,occupiedNights,nightlyAccommodationMinor,
  excludedCurrencies,paymentStatuses,allocations}`. Monetary totals are integer
  minor units; averages may be fractional minor units with visible denominators.
* `work.window {date, timezone?}` -> office/destination windows and conflicts.
* `imports.parse {text, format? (text|eml|ics), filename?}` -> saved draft array.
* `imports.file {filename, contentBase64}` -> saved drafts, or saved document for
  non-import attachments. Bound input size, never execute input.
* `imports.accept {id, record?, expectedVersion?}` -> saved booking with source
  links; repeated/revised source updates existing booking rather than duplicating.
* `imports.dismiss {id}` -> dismissed draft.
* `calendar.export {}` -> `{filename,content}` (ICS).
* `reminders.due {now?}` -> newly delivered reminder array; records persist.
* `reminders.snooze {id, minutes}` / `reminders.dismiss {id}` -> saved reminder.
* `backup.export {}` -> `{path, filename}`; `backup.restore {filename}` uses only
  a basename in the owned backups directory and backs up before restoring.
* `assistant.ask {text}` -> `{message, records?, suggestions?}`. Queries call
  exact services; no unsupported live claims. Writes require an explicit action.
* `email.status {}` / `email.configure {provider,clientId,tenant?,filter?}` /
  `email.connect {}` -> `{authorizationUrl?}` / `email.sync {}` /
  `email.pause {paused}` / `email.disconnect {}`. OAuth and tokens native only.
* `ai.status {}` -> explicit availability; optional inference configuration.

Methods can grow while preserving this contract. Unknown facilities stay null.
URLs open in the system browser only after HTTP(S) validation. Saved data is
offline; online connections and companion availability are shown explicitly.

Additional shared services:

* `calendar.occurrences {start,end}` expands a bounded exclusive-end interval;
  occurrences retain original IDs and add `occurrenceId`/`occurrenceDate`. Edit
  the original record, or use per-date completion/exception lists.
* `fx.convert {amountMinor,fromCurrency,toCurrency,rateNumerator,rateDenominator,
  rateDate,source?}` returns an exact rational planning conversion without
  changing history. Settled cross-currency payments can store their immutable
  `bookingCurrency`, `bookingAmountMinor`, `rateDate`, and `rateSource` basis.
* `discovery.rank {checkIn?,checkOut?,currency?,quoteIds?,preferences?}` ranks
  saved exact-date offers within comparable groups and reports unknown evidence.
* `planning.what_if {stayId,additionalNights? or newCheckOut}` estimates an
  extension, flags conflicts, and never changes bookings or payments.
* `planning.origin {date,excludeStayId?,tripId?}` prefers recorded planned origin
  over the separate current-location setting, with an explicit evidence basis.
* `imports.ai {text}` performs optional local extraction and persists a review
  draft; deterministic parsing remains the normal first step.
* `email.auto_sync {}` synchronizes only when the retained native connector is
  connected, unpaused and due. The native worker calls it every minute.
* `assistant.ask` supports queries and explicit `Remind me: NAME at RFC3339`
  requests. Created reminders return a version-checked undo action. Suggested
  reminders and what-if requests never imply an automatic booking change.
