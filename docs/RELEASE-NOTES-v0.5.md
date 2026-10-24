# Birthright: Atlas 0.5.0 candidate

This candidate prepares the permanent signed-update transition, adds native
Local AI model management, reviewed journey planning and selected offline trip
information. The charcoal interface, dock, utility panels and Times New Roman
numerals are retained.

Production updating is **pending owner activation**. The completed application
and release workflow must be integrated into the default branch after approval;
owner-controlled keys must be configured; a signed bootstrap and subsequent
stable release must be built, reviewed and published. Follow
[OWNER-UPDATE-SETUP.md](OWNER-UPDATE-SETUP.md). An ordinary unsigned CI installer
is a development candidate, not the production-key-bearing bootstrap.

- The Updates panel clears stale availability after a failed check, distinguishes
  unavailable/offline/unconfigured states, shows measured transfer progress and
  offers the official release page when published stable metadata is available.
- Release preparation checks signing configuration before four native builds.
  Exact version-bound signatures and platform mappings remain mandatory. Nothing
  merges or publishes automatically.
- Settings → Local AI offers pinned official Qwen 0.6B/1.7B Q8_0 downloads,
  explicit size approval, progress, cancellation, checksum verification,
  enable/disable, removal, testing and bounded CPU/context/output/idle settings.
  Installed Atlas uses Rust for installation; no Python toolchain is required.
- Atlas Planner and Local AI modes are distinct. Local AI explains authorized
  saved planner results and cannot execute tools, authorize payments or save
  generated facts. Deterministic planning continues without a model.
- Map contains tentative multi-stop journeys, saved-place route drafts, known-cost
  month-of-use comparisons and individually reviewed scheduling proposals. Work
  and confirmed bookings are protected; acceptance rechecks current conflicts.
- Export an explicit trip package with chosen documents. The companion's Trip
  view reads the synchronized saved journey offline and keeps document selection
  explicit. Routes, travel durations and live availability remain unknown until
  established by evidence.

Schema remains 5. The application identity and data directory remain unchanged.
See [validation and limitations](VALIDATION-v0.5.md). Successful inference,
physical iPhone behavior and production publication are separate checks.
