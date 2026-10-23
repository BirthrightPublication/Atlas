# Interface and application flow

The React interface uses the Rust application service for persistence and domain
calculations. Native windows use Tauri IPC. Development uses Vite's loopback
proxy to the authenticated host in development mode. There is no mock database
fallback when the service is unavailable.

Version 0.3 retains `src/theme.css` for the common neutral palette and `src/shell.css`
for the native desktop layout. The header, scrollable workspace and bottom dock
occupy separate grid rows, so content cannot disappear beneath navigation.
Calendar, Discover, Map and Budget keep their full content width. Switching
modules starts at the top of the destination workspace. Safe-area insets protect
the dock on compact displays.

The gear at the dock's right opens a full Settings page. Its ten categories use
arrow-key navigation. Leaving dirty settings offers Save and continue, Discard
and continue, or Keep editing. Only changed preference keys are persisted.

Notifications, Updates and Atlas Assistant share a nonmodal right panel. At wide
sizes the main content contracts; compact windows use an overlay above the dock.
Same-button toggling, direct switching, Escape and explicit close are supported.
Panels retain local state across module navigation, scroll independently and
load their code only on first use. Other entry/import dialogs retain focus traps.

Assistant conversations and notification state are stored in SQLite. Responses
use context references and deterministic services; the optional model's actual
availability is reported separately. All result navigation uses local typed
module/date/record targets with missing-record handling. No URL can become an
internal command. Updates installs only through Tauri's trusted signed updater;
release history is informational. Unsaved Settings or open record forms disable
installation until the user saves or closes them.

Digits use a local-only Unicode font face selecting Times New Roman, then
Liberation Serif/Times. Letter typography and theme tokens remain unchanged.
No proprietary font is bundled or downloaded.

## Verification

`npm run build` checks TypeScript and bundles the interface. `npm run test:e2e`
exercises the actual local host, import review, budgets, revisions, persisted manual
records, and desktop/phone layouts. Native OS/device tests and live mailbox tests
must be reported separately from browser/service checks.
