# Private iPhone companion

Atlas includes a responsive companion at `/companion`, an installable web
manifest/service worker, and a separate Rust service. It uses the same Rust
application services and SQLite records as the desktop. The companion does not
expose SQLite, local filesystem paths, OAuth tokens, the email inbox, or AI
endpoints.

The selected first phone is iPhone. The implementation also uses standard PWA
APIs usable in other modern browsers; no physical phone validation is claimed.

## What works

- A one-time, 96-bit pairing code expires after ten minutes and cannot be reused.
  Per-device random bearer credentials are stored hashed on the desktop.
- Devices can be listed and revoked. Last synchronization is recorded. Revoking
  prevents future requests; it cannot remotely erase an offline phone copy.
- Saved stays, contact/address links, objectives, transport, and monthly budget
  can be read without the desktop. New expenses and objective completion are
  saved to a durable IndexedDB queue.
- Replaying the same mutation request ID returns the original result. Updates
  carry their observed record version. A changed desktop record produces an
  explicit conflict; the owner reviews both versions and chooses to discard the
  phone change or apply it against the latest version. No silent last-write-wins.
- Documents explicitly marked `companionAvailable: true` on the desktop can be
  downloaded by the paired phone and kept offline. Limit: 8 MiB each, 50 MiB
  total. Removing a phone copy does not delete the desktop original.
- Calendar export is available while the desktop is reachable. Delivery of
  imported calendar reminders is governed by the selected calendar app.
- Phone storage controls remove documents or all phone data. A persistent-storage
  request is offered; Safari/OS policy still determines whether storage survives.

Phone records and credentials reside in browser-managed storage. Atlas does not
add application-level encryption to IndexedDB. Use device passcode/encryption;
desktop database and attachments likewise depend on OS disk/user protections.
The service worker caches app code only. API responses and documents are never
put in the HTTP cache automatically.

## Start the service

Development only, with a separate synthetic-data directory:

```sh
cargo run -p atlas-host -- --dev --data-dir /tmp/atlas-dev
npm run dev
```

`--dev` is intentionally unauthenticated for local application development. It
binds `127.0.0.1` only and rejects unrelated Host/Origin headers. Never forward
this mode to a LAN, a tunnel, or a public interface. Vite forwards `/api` with
`changeOrigin: true`. Its configured development ports are allowed explicitly.

Production companion mode refuses to start without an HTTPS origin and a built
UI directory. The listener stays on loopback and expects a trusted private HTTPS
reverse proxy in front of it:

```sh
npm run build
cargo build --release -p atlas-host
target/release/atlas-host \
  --origin https://YOUR-DESKTOP.YOUR-TAILNET.ts.net \
  --static-dir dist
```

The packaged desktop can start its bundled companion service and UI resources
from Phone settings. The configured private origin must be reachable through trusted
HTTPS. Both processes use the `org.birthright.atlas` OS data directory unless an
explicit `--data-dir` is provided. Do not point a synthetic test host at personal
data. The default port is 47832; `--port` changes it on both server/control calls.
The desktop-managed host stops when Atlas exits; start it again from Phone
settings after reopening Atlas.

## A private HTTPS route for iPhone

One workable route is Tailscale Serve on an existing private tailnet. It requires
the owner's separate Tailscale account/device authorization and trusted HTTPS
configuration; Atlas does not create that account, install a VPN, or expose a
public endpoint automatically. Tailscale Funnel is **not** required.

1. On the desktop and iPhone, install the official Tailscale app and authorize
   the devices to the same private tailnet. Enable MagicDNS/HTTPS certificates
   as required by that service. Restrict tailnet access to the intended devices.
2. Determine the desktop's actual full `*.ts.net` hostname. Start Atlas companion
   with that exact `https://` origin, without a path.
3. On the desktop, configure the authenticated private reverse proxy:

   ```sh
   tailscale serve --bg http://127.0.0.1:47832
   tailscale serve status
   ```

4. On iPhone, with Tailscale connected, open
   `https://YOUR-DESKTOP.YOUR-TAILNET.ts.net/companion` in Safari. There must be
   no certificate warning. Never disable TLS checks or browser security.
5. In Safari, open Share → Add to Home Screen, turn on **Open as Web App** when
   offered, then tap Add. If Add to Home Screen is absent, use Edit Actions to
   add it. Launch Atlas from its new icon. Issue a pairing code on the desktop
   and enter it with a recognizable phone name.

Other existing trusted private HTTPS reverse proxies can be used. They must
route only the intended host to loopback port 47832 and preserve the configured
origin. Do not expose a plain `http://192.168…` address as a finished PWA solution.

Official references, successfully checked over verified HTTPS on **2026-10-22**:
[Tailscale Serve CLI](https://tailscale.com/docs/reference/tailscale-cli/serve)
documents `--bg`, HTTPS as the default serving mode with an automatically
provisioned certificate, and loopback HTTP proxy targets. [Apple: Turn a website
into an app](https://support.apple.com/guide/iphone/open-as-web-app-iphea86e5236/ios)
documents Share → Add to Home Screen → Open as Web App → Add. The former Apple
link now redirects to its bookmark guide, so this link points to the current web
app instructions. This documentation check does not validate an authorized
tailnet, a device certificate, or Safari installation on the owner's iPhone.

## Pair, inspect, revoke

With the host running, use Phone settings: **Pair an iPhone** issues a code and
shows its URL/expiry; the paired-device list provides **Revoke**. For the source
build started above, run these commands from the same repository directory:

```sh
target/release/atlas-host --pair
target/release/atlas-host --devices
target/release/atlas-host --revoke DEVICE_ID
```

For an installed app, prefer Phone settings or replace `target/release/atlas-host`
with the full path to its bundled host executable. On Windows that executable is
`atlas-host.exe`; installation does not require adding it to the shell's PATH.
Pass the same `--data-dir`/`--port` options if you overrode them when starting.
The pairing response contains `code`, `expiresAt`, and the companion URL. The
local control credential is kept in a private file and never sent to the phone.
The endpoint for this control rejects browser origins and requires the private
credential. Device responses omit token hashes. Token material must not be
copied into screenshots, public logs, Git, or issue reports.

## Behavior when devices are unavailable

The phone states whether it last reached the desktop. A browser's network
indicator alone is not treated as proof of host availability. Manual sync and
reconnection replay the queue. A stopped, sleeping, or disconnected desktop
cannot import mail, run scans, or answer new requests. Previously cached phone
data still works. Clearing browser/app storage removes unsynchronized changes;
the UI warns before its own clear-data action.

Atlas does not promise that a closed iPhone PWA delivers offline reminders. No
push service is implemented or silently introduced. Use calendar export for
calendar-managed notifications and desktop notifications while Atlas is running.

## Validation and remaining device steps

Automated host tests cover missing/invalid authorization, wrong origins, DNS
rebinding protection in development, one-time pairing, revocation, restricted
service access, private snapshot filtering, idempotent expense replay, and
record-version conflicts. Current container results: **7 host tests passed**,
including concurrent request replay and the successful SPA response needed by
service-worker installation. **2 Chromium browser tests passed** against the
real loopback host, SQLite services, IndexedDB, and service worker: pairing,
offline document retention/reload, queued expense synchronization exactly once,
and explicit objective-conflict resolution. Chromium used the standard secure
context exception for localhost; TLS verification was not disabled. These
results do not validate private HTTPS routing or a physical iPhone.

The following still require the owner's iPhone/private network:

1. Confirm the private hostname resolves and its HTTPS certificate is trusted
   on Safari over both Wi-Fi and mobile data with the private network active.
2. Install from Safari, pair, and synchronize a synthetic itinerary.
3. Disable connectivity, close/reopen the Home Screen app, and verify saved
   agenda and a selected synthetic document. Queue a small synthetic expense.
4. Restore connectivity and confirm exactly one expense on the desktop.
5. Edit the same objective on desktop/phone to verify the conflict review.
6. Revoke the device on the desktop and confirm new sync fails while an old
   offline copy remains until removed on the phone.

No physical iPhone, live tailnet, or trusted device certificate was available in
the build container. The account authorization, Safari installation, background
storage behavior, and private-network path are not marked verified by unit tests.
