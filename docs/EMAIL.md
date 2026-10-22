# Automatic booking email import

Atlas has native read-only connectors for Proton Mail Bridge / other IMAP,
Gmail, and Outlook / Microsoft 365. A mailbox must be explicitly connected in
Settings. Imported confirmations become review drafts: they do not silently
purchase, cancel, or modify confirmed bookings. Gmail/Graph credentials and IMAP
passwords are kept only in the operating system's credential store. If that store
is unavailable or locked, connection fails; there is no plaintext fallback.

The desktop polls while it is running and online. Pause stops subsequent polls.
Sync now is available for catch-up. A successful sync records its last-sync time;
network and authorization failures remain visible. The host must be running for
mail retrieval. The phone does not independently hold mailbox credentials.

## Proton Mail

**Official Proton Mail Bridge requires a paid Proton Mail plan.** A free Proton
account cannot use this connector. Atlas does not change the account provider or
add a paid aggregator to work around that limitation. If the owner uses free
Proton, automatic import remains blocked until the owner selects a supported
account configuration. Manual EML import remains available but is not described
as a substitute for the required automatic integration.

1. Install the official Bridge from <https://proton.me/mail/bridge> on the same
   computer as Atlas. Sign in through Bridge, which handles Proton authentication.
2. In Bridge, copy its IMAP host, port, username, and **Bridge-generated password**.
   This is not the Proton account password. Keep Bridge running.
3. In Atlas Settings select Proton Bridge. Choose STARTTLS or TLS to match Bridge;
   the usual STARTTLS port is 1143, but the values shown by Bridge are authoritative.
   Only `localhost` and `127.0.0.1` are accepted for this connector.
4. Export Bridge's TLS certificate and paste its PEM certificate into the trust
   certificate field if it is not already trusted by the operating system. Use a
   hostname included in that certificate. Atlas keeps certificate and hostname
   verification enabled, with TLS 1.2 or newer. An untrusted or mismatched
   certificate fails visibly; there is no “ignore certificate errors” option.
5. Save the settings and connect. Atlas authenticates over TLS and opens the
   configured folder with `EXAMINE`. An initial sync creates review drafts. Adjust
   sender/date filters to keep retrieval focused on bookings.

A Bridge update or account reset can rotate local connection details and the
certificate. Update the explicitly trusted certificate after checking it in the
local Bridge app. Never take a replacement trust certificate from an email.

The official Bridge repository was read during implementation:
<https://github.com/ProtonMail/proton-bridge>. Its README documents that Bridge
must be running and uses the native credential manager on Windows/macOS and a
Secret Service provider on Linux. Proton's web support page was not reachable
through this build environment's network policy, so current plan terms still
need a live recheck before release.

## Other IMAP providers

Use the provider's documented IMAP hostname, folder, port, and supported password
or app-password authentication. Atlas supports implicit TLS (usually 993) and
mandatory STARTTLS. Plaintext connections are unsupported. Password authentication
is used only inside a verified TLS tunnel. The provider must permit this
supported authentication method; an account requiring OAuth-only IMAP should use
its Gmail/Outlook connector instead, or needs a future provider-specific adapter.

Only read-only commands are issued: `LOGIN`, `EXAMINE`, `UID SEARCH`,
`UID FETCH ... BODY.PEEK[]`, and `LOGOUT`. `BODY.PEEK[]` does not set the message's
Seen flag. Cursor identity contains the folder, UIDVALIDITY, and UID. If the server
changes UIDVALIDITY, Atlas rescans and the import pipeline deduplicates existing
booking evidence. It does not delete or send mail. The account password itself
may grant broader access than those commands; filters do not change that grant.

## Gmail / Google Workspace

1. In Google Cloud Console create an OAuth project, enable Gmail API, and register
   an OAuth client of type **Desktop app**. Configure the consent screen for your
   intended users and use Google's documented personal-use/production rules.
2. Enter the client ID and client secret from **your own** desktop registration in
   Atlas Settings. The per-owner secret is stored in the OS credential store; it
   is not a shared commercial credential embedded in the executable.
3. Connect opens Google's authorization page in the system browser. Atlas uses
   PKCE S256, a random state, and a temporary loopback callback. The only Gmail
   scope requested is `https://www.googleapis.com/auth/gmail.readonly`.
4. Review the consent screen. This restricted scope can read the mailbox, even
   when Atlas retrieval is filtered to selected dates, senders, or labels.

External OAuth apps left in Testing may receive refresh tokens that expire after
seven days for these scopes. That configuration is not a dependable unattended
integration. Use an appropriate production/personal setup under Google's current
rules or expect reconnect notices. Commercial distribution may require restricted
scope verification and other Google requirements; fixture tests do not establish
approval or a working live account.

Initial Gmail sync uses a configurable booking query, defaulting to reservation,
booking, confirmation, itinerary, cancellation, or refund. It records a history
baseline before paging. Later syncs use history and recheck the configured query
before retrieving message bodies. An expired history cursor (HTTP 404) causes a
fresh scan with normal deduplication. Filters can be configured through the
service as `filter.query`, `filter.senders`, `filter.afterDate`, and
`filter.labelOrFolder`; only the supported controls exposed in Settings are
available in the normal UI.

References requiring a current live recheck:
- <https://developers.google.com/identity/protocols/oauth2/native-app>
- <https://developers.google.com/workspace/gmail/api/auth/scopes>
- <https://developers.google.com/gmail/api/guides/sync>
- <https://developers.google.com/identity/protocols/oauth2>

## Outlook / Microsoft 365

1. Register a public/native application in Microsoft Entra ID for the account
   types you intend to support. Set up the supported loopback redirect for the
   desktop platform. Use `common` for eligible personal and organization accounts
   or supply your tenant identifier.
2. Add delegated Microsoft Graph `Mail.Read` and the normal public-client
   `openid` / `offline_access` scopes. Do not configure mail sending/deletion
   permissions. `Mail.ReadBasic` cannot supply the bodies needed for import.
3. Enter the client ID and connect in the system browser. PKCE S256 and state are
   used; public native applications do not need an embedded client secret.
4. Tenant policies or administrator consent may block the grant. Atlas shows that
   failure without pretending the account is connected.

Graph sync follows folder-specific `@odata.nextLink` and `@odata.deltaLink`
continuations. Continuation URLs are accepted only on the provider's HTTPS host,
without URL credentials or alternate ports. Deleted message tombstones are not
interpreted as booking cancellations. Actual cancellation/refund messages are
parsed as evidence and need review. HTML is converted to inert text; remote
images and tracking pixels are never fetched or displayed.

References requiring a current live recheck:
- <https://learn.microsoft.com/en-us/graph/permissions-reference>
- <https://learn.microsoft.com/en-us/graph/delta-query-messages>
- <https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow>

## Cursor, retry, and privacy contract

`EmailService::sync()` returns a bounded batch and a proposed cursor. The caller
must durably store or explicitly account for every source before calling
`acknowledge(&batch)`. An import failure must not acknowledge the batch. Repeating
an unacknowledged batch is intentional and must be deduplicated by provider plus
message identity and then by booking identity in the import service. Configuration
changes invalidate a pending batch. Up to 30 IMAP or list results are processed
per poll; additional pages are reported with `hasMore`.

HTTPS requests have timeouts and bounded response sizes. Rate-limit/server
responses retry twice with bounded backoff; later polls provide further catch-up.
Provider error bodies and credentials are never included in user-facing errors or
logs. OAuth callbacks expire after five minutes. Retrieved MIME content is
untrusted parser input; it receives no shell, file, or network authority. Message
size is limited to 12 MiB and response size to 20 MiB. Large attachments should be
imported deliberately using the document workflow.

Core data and mailbox configuration are local user data, not encrypted SQLite.
OS secret protection covers credentials, not all imported content. Use the OS
account protections and disk encryption for private database/document backups.

## Validation boundaries

Unit tests cover PKCE/read-only scopes, OAuth state rejection, no credentials in
metadata/status, Proton loopback and TLS requirements, commit-after-import cursor
semantics, literal-aware IMAP parsing and limits, exact sender matching, malicious
continuation rejection, command injection rejection, and refresh-token retention.
These tests use synthetic fixtures and an in-memory test secret store.

No personal mailbox, Proton Bridge instance, Google OAuth registration, or
Microsoft tenant was available in the build environment. Live authorization,
Bridge TLS trust, ongoing sync, and platform keychain access must therefore be
verified on the owner's desktop. Google/Microsoft official web documentation
returned a proxy CONNECT 403 during implementation; implementation against known
API contracts is not a claim those current pages were verified.
