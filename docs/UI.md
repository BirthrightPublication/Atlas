# Interface and application flow

The React interface uses the Rust application service for persistence and domain
calculations. Native windows use Tauri IPC. Development uses Vite's loopback
proxy to the authenticated host in development mode. There is no mock database
fallback when the service is unavailable.

Calendar supports month, agenda, day, and journey views. Phone widths switch to
an agenda rather than shrinking month cells. Stays occupy continuous night bands;
objectives and protected work appear below them; transport markers remain compact.
On Foot and On Road only change creation defaults. Recurring records are expanded
by the Rust service and editing an occurrence edits the original series.

The import inbox accepts text, URLs, EML, text-bearing PDF, and ICS. An explicit
synthetic sample demonstrates the cross-month acceptance case. Drafts preserve
unknowns and evidence. The user reviews fields before accepting. Optional verified
local inference runs only from an explicit extraction action after ordinary parsing.

Budget totals come from the exact application service. Month-of-use, cash-flow,
and available-cash views are separate. Cash projection displays remaining flexible
allowance and reserve deductions; warnings identify missing payment schedules and
currency bases. A date-range calculator exposes its denominator.

The website library contains prepared links with an honest capability label. It is
editable, and individual saved places and offers are separate records. Saved offers
can be compared using date-specific totals, fees, freshness, and verified facilities.
The Map view is a local location directory; external mapping providers supply live
maps and directions. No background scraping or live route data is implied.

Settings include named personal presets, configurable work rules, office/current
timezones, optional fixed offsets, protected local-day windows, provider connection
setup, backups, local model configuration, and native companion pairing controls.
Personal preferences are user data, not public seed fixtures. Gmail/Outlook and
Proton Bridge/IMAP are explicitly disconnected until successfully authorized.

The desktop app starts update checks and refreshes local data when native import
and reminder events arrive. Updating asks users to save open edits. Signed update
configuration is required before the native updater can install an update.

## Verification

`npm run build` checks TypeScript and bundles the interface. `npm run test:e2e`
exercises the actual local host, import review, budgets, revisions, persisted manual
records, and desktop/phone layouts. Native OS/device tests and live mailbox tests
must be reported separately from browser/service checks.
