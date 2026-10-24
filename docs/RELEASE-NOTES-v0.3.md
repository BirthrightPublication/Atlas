# Birthright: Atlas 0.3.0 — development preview

Atlas 0.3 keeps the neutral dark theme and bottom navigation while making travel
planning, conversations and application controls easier to use together.

- Settings now occupies a full application page, with explicit save/discard
  handling when leaving unsaved edits.
- Notifications, Updates and Atlas Assistant share a toggleable right panel.
  The main workspace remains usable while a panel is open.
- Assistant conversations, recent chats and notification read/dismiss state are
  saved locally and survive restarts. Suggestions use the active screen and
  saved itinerary, budget and preference evidence.
- Stay and transport entry support tentative drafts, duplication and linked
  documents. Journey reviews show accommodation gaps, overnight coverage,
  transfer timing and remote-work suitability, including unknown evidence.
- Budget shows daily and nightly estimates, flexible money and reserves.
  Stay-extension and transport-price scenarios leave saved financial history
  unchanged.
- Numerals use Times New Roman where installed, with local serif fallbacks.
- Updates shows real published release metadata and cached results when
  available. The native updater uses a compiled verification key, HTTPS,
  signatures, backups and measured download progress.

The preview installer supports a manual upgrade from v0.2. It retains the Atlas
application identity and user-data directory; schema migration creates a backup
before adding conversation and notification tables. Close Atlas before running
the installer. See [validation and installer evidence](VALIDATION-v0.3.md).

Production update signing and the stable feed still require
[owner configuration](RELEASE.md#signing-setup). A build containing the production
verification key must be installed manually once before subsequent signed
releases can use **Update Now**. Disposable update tests do not enable that feed.

Local AI remains optional. Live Proton authorization, physical iPhone validation
and macOS installation are separate external checks. This candidate does not
claim live travel inventory, model inference, OS publisher signing or Apple
notarization.
