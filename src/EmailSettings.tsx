import { useEffect, useRef, useState } from "react";
import { Shield, RefreshCw, KeyRound, Pause } from "lucide-react";
import { rpc, openExternal } from "./api";
export function EmailSettings({ onImports }: { onImports: () => void }) {
  const [accounts, setAccounts] = useState<any[]>([]);
  const [accountId, setAccountId] = useState("default");
  const [step, setStep] = useState(0);
  const currentAccount = useRef(accountId);
  currentAccount.current = accountId;
  const accountRpc = (method: string, payload: any = {}) =>
    rpc(method, { ...payload, accountId });
  const [status, setStatus] = useState<any>(null);
  const [form, setForm] = useState<any>({
    provider: "proton-bridge",
    label: "",
    clientId: "",
    clientSecret: "",
    tenant: "common",
    host: "127.0.0.1",
    port: 1143,
    username: "",
    password: "",
    tlsMode: "starttls",
    certificatePem: "",
    folder: "INBOX",
    senders: "",
    afterDate: "",
    beforeDate: "",
    domains: "",
    excludedSenders: "",
    subjectKeywords: "",
    reservationKeywords: "",
    query: "",
    labelOrFolder: "",
    syncMinutes: 5,
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const initialized = useRef(false);
  const refresh = async () => {
    try {
      const s = await accountRpc("email.status");
      const list = await rpc("email.accounts");
      if (currentAccount.current !== accountId) return;
      setStatus(s);
      setAccounts(list);
      if (s.configured && !initialized.current) {
        initialized.current = true;
        setForm((f: any) => ({
          ...f,
          ...Object.fromEntries(
            [
              "provider",
              "label",
              "clientId",
              "tenant",
              "host",
              "port",
              "username",
              "tlsMode",
              "folder",
            ]
              .filter((key) => s[key] != null)
              .map((key) => [key, s[key]]),
          ),
          senders: (s.filter?.senders || []).join(", "),
          ...Object.fromEntries(
            [
              "domains",
              "excludedSenders",
              "subjectKeywords",
              "reservationKeywords",
            ].map((k) => [k, (s.filter?.[k] || []).join(", ")]),
          ),
          ...Object.fromEntries(
            ["afterDate", "beforeDate", "query", "labelOrFolder"].map((k) => [
              k,
              s.filter?.[k] || "",
            ]),
          ),
          syncMinutes: s.syncMinutes || 5,
        }));
      }
    } catch (e) {
      setError(String(e));
    }
  };
  useEffect(() => {
    void refresh();
    const timer = setInterval(refresh, 5000);
    return () => clearInterval(timer);
  }, [accountId]);
  const set = (key: string, value: any) =>
    setForm((f: any) => ({ ...f, [key]: value }));
  const imap = ["proton-bridge", "imap"].includes(form.provider);
  async function run(fn: () => Promise<any>, message: string) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const result = await fn();
      if (result?.authorizationUrl) await openExternal(result.authorizationUrl);
      await refresh();
      setNotice(message);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <div className="form-grid">
        <label className="field">
          <span>Mailbox account</span>
          <select
            value={accountId}
            disabled={busy}
            onChange={(e) => {
              initialized.current = false;
              setAccountId(e.target.value);
              setStep(0);
              setForm((f: any) => ({
                ...f,
                password: "",
                clientSecret: "",
                certificatePem: "",
              }));
            }}
          >
            {accounts.map((a) => (
              <option key={a.accountId} value={a.accountId}>
                {a.label ||
                  a.username ||
                  `${a.provider || "New connection"} · ${a.accountId.slice(0, 6)}`}{" "}
                · {a.connectionState}
              </option>
            ))}
          </select>
        </label>
        <button
          className="secondary"
          disabled={busy}
          onClick={() =>
            run(async () => {
              const a = await rpc("email.add");
              initialized.current = false;
              setAccountId(a.accountId);
              setStep(0);
              setForm((f: any) => ({
                ...f,
                label: "",
                username: "",
                password: "",
                clientSecret: "",
                clientId: "",
                certificatePem: "",
              }));
            }, "Connection slot created.")
          }
        >
          Add account
        </button>
      </div>
      <nav className="connection-steps" aria-label="Connection setup">
        {[
          "Provider & permissions",
          "Authorize & test",
          "Filters & start date",
          "Sync & review",
        ].map((name, i) => (
          <button
            key={name}
            className={step === i ? "active" : ""}
            onClick={() => setStep(i)}
          >
            {i + 1}. {name}
          </button>
        ))}
      </nav>
      <div className="email-status">
        <span className={`status-dot ${status?.connected ? "online" : ""}`} />
        <div>
          <strong>{status?.connectionState || "Not connected"}</strong>
          <small>
            {status?.lastSync
              ? "Last sync " + new Date(status.lastSync).toLocaleString()
              : "Bookings will arrive as review drafts."}
          </small>
        </div>
        {status?.syncing && <RefreshCw size={16} className="spin" />}
      </div>
      {status?.lastError && (
        <div className="error-banner">{status.lastError}</div>
      )}
      {error && (
        <div role="alert" className="error-banner">
          {error}
        </div>
      )}
      {notice && (
        <div role="status" className="success-banner">
          {notice}
        </div>
      )}
      {step === 0 && (
        <>
          <label className="field">
            <span>Account label</span>
            <input
              value={form.label}
              maxLength={80}
              onChange={(e) => set("label", e.target.value)}
              placeholder="Travel bookings"
            />
          </label>
          <label className="field">
            <span>Booking email provider</span>
            <select
              value={form.provider}
              onChange={(e) => {
                const provider = e.target.value;
                setForm((f: any) => ({
                  ...f,
                  provider,
                  host: provider === "proton-bridge" ? "127.0.0.1" : "",
                  port: provider === "proton-bridge" ? 1143 : 993,
                  tlsMode: provider === "proton-bridge" ? "starttls" : "tls",
                }));
              }}
            >
              <option value="proton-bridge">Proton Mail Bridge</option>
              <option value="imap">Other provider · IMAP</option>
              <option value="gmail">Gmail / Google Workspace</option>
              <option value="outlook">Outlook / Microsoft 365</option>
            </select>
          </label>
          {form.provider === "proton-bridge" && (
            <div className="info-note">
              <Shield size={18} />
              <div>
                Proton’s official Bridge requires a paid Proton Mail plan and a
                running local Bridge. Free Proton accounts cannot use this
                automatic import path. Use the Bridge-generated local
                credentials and export its certificate.
              </div>
            </div>
          )}
          <div className="info-note">
            <Shield size={18} />
            <div>
              <strong>Read-only booking import</strong>
              <p>
                Gmail grants restricted gmail.readonly access. Outlook grants
                delegated Mail.Read access. IMAP uses read-only commands with
                credentials that may grant wider account access. Retrieval
                filters do not narrow these permissions.
              </p>
              <p>
                Atlas retrieves selected messages and retains booking evidence
                locally. Remote images are never loaded. For practical
                separation, use a dedicated booking mailbox or folder.
              </p>
            </div>
          </div>
          <button className="primary" onClick={() => setStep(1)}>
            Continue to authorization
          </button>
        </>
      )}
      {step === 1 && (
        <>
          {!imap ? (
            <>
              <label className="field">
                <span>Registered desktop OAuth client ID</span>
                <input
                  value={form.clientId}
                  onChange={(e) => set("clientId", e.target.value)}
                  autoComplete="off"
                />
              </label>
              {form.provider === "gmail" && (
                <>
                  <label className="field">
                    <span>Desktop OAuth client secret</span>
                    <input
                      type="password"
                      value={form.clientSecret}
                      onChange={(e) => set("clientSecret", e.target.value)}
                      autoComplete="new-password"
                    />
                  </label>
                  <p className="field-help">
                    Google desktop OAuth uses your own registered application.
                    Restricted Gmail read-only scope requires correct consent
                    setup. External apps left in Testing may have seven-day
                    refresh-token expiry.
                  </p>
                </>
              )}
              {form.provider === "outlook" && (
                <label className="field">
                  <span>Tenant</span>
                  <input
                    value={form.tenant}
                    onChange={(e) => set("tenant", e.target.value)}
                    placeholder="common"
                  />
                </label>
              )}
            </>
          ) : (
            <>
              <div className="form-grid">
                <label className="field">
                  <span>IMAP host</span>
                  <input
                    value={form.host}
                    onChange={(e) => set("host", e.target.value)}
                  />
                </label>
                <label className="field">
                  <span>Port</span>
                  <input
                    type="number"
                    value={form.port}
                    onChange={(e) => set("port", Number(e.target.value))}
                  />
                </label>
              </div>
              <label className="field">
                <span>Transport security</span>
                <select
                  value={form.tlsMode}
                  onChange={(e) => set("tlsMode", e.target.value)}
                >
                  <option value="tls">TLS</option>
                  <option value="starttls">STARTTLS (required upgrade)</option>
                </select>
              </label>
              <label className="field">
                <span>Mailbox username</span>
                <input
                  value={form.username}
                  onChange={(e) => set("username", e.target.value)}
                  autoComplete="username"
                />
              </label>
              <label className="field">
                <span>
                  {form.provider === "proton-bridge"
                    ? "Bridge-generated password"
                    : "IMAP app password"}
                </span>
                <input
                  type="password"
                  value={form.password}
                  onChange={(e) => set("password", e.target.value)}
                  autoComplete="new-password"
                />
              </label>
              {form.provider === "proton-bridge" && (
                <label className="field">
                  <span>
                    Bridge certificate (PEM)
                    {status?.hasCustomTrustCertificate
                      ? " · saved certificate retained if blank"
                      : ""}
                  </span>
                  <textarea
                    rows={4}
                    value={form.certificatePem}
                    onChange={(e) => set("certificatePem", e.target.value)}
                    placeholder="-----BEGIN CERTIFICATE-----"
                  />
                </label>
              )}
              <label className="field">
                <span>Folder</span>
                <input
                  value={form.folder}
                  onChange={(e) => set("folder", e.target.value)}
                />
              </label>
            </>
          )}
        </>
      )}
      {step === 2 && (
        <>
          <h3 className="form-section">Retrieval filters</h3>
          <label className="field">
            <span>Allowed senders (comma-separated; optional)</span>
            <input
              value={form.senders}
              onChange={(e) => set("senders", e.target.value)}
              placeholder="bookings@example.com"
            />
          </label>
          <label className="field">
            <span>Retrieve after (optional)</span>
            <input
              type="date"
              value={form.afterDate}
              onChange={(e) => set("afterDate", e.target.value)}
            />
          </label>
          <div className="form-grid">
            {[
              ["domains", "Approved sender domains"],
              ["excludedSenders", "Excluded sender addresses"],
              ["subjectKeywords", "Subject keywords (any)"],
              ["reservationKeywords", "Reservation keywords in subject (any)"],
            ].map(([key, label]) => (
              <label className="field" key={key}>
                <span>{label} · comma-separated</span>
                <input
                  value={form[key]}
                  onChange={(e) => set(key, e.target.value)}
                />
              </label>
            ))}
            <label className="field">
              <span>Retrieve before (exclusive)</span>
              <input
                type="date"
                value={form.beforeDate}
                onChange={(e) => set("beforeDate", e.target.value)}
              />
            </label>
            <label className="field">
              <span>Sync interval · minutes</span>
              <input
                type="number"
                min={5}
                max={1440}
                value={form.syncMinutes}
                onChange={(e) => set("syncMinutes", Number(e.target.value))}
              />
            </label>
            {!imap && (
              <label className="field">
                <span>
                  {form.provider === "gmail"
                    ? "Gmail label"
                    : "Outlook folder ID or name"}
                </span>
                <input
                  value={form.labelOrFolder}
                  onChange={(e) => set("labelOrFolder", e.target.value)}
                />
              </label>
            )}
            {form.provider === "gmail" && (
              <label className="field">
                <span>Gmail search query</span>
                <input
                  value={form.query}
                  onChange={(e) => set("query", e.target.value)}
                  placeholder="reservation OR booking OR refund"
                />
              </label>
            )}
          </div>
          <p className="field-help">
            Addresses and domains form one allowlist. Exclusions take priority.
            Each enabled keyword group must match the subject. A blank group
            imposes no restriction. Changing filters safely starts a fresh scan;
            existing drafts are deduplicated.
          </p>
        </>
      )}
      <p className="field-help">
        Read-only import. Filters restrict retrieval, not the OAuth grant. No
        sending or deletion permissions are requested. Passwords and tokens are
        stored in the OS secret store; they are never saved to the project.
      </p>
      {(step === 1 || step === 2) && (
        <div className="inline-actions">
          <button
            className="secondary"
            disabled={busy}
            onClick={() =>
              run(() => {
                const payload: any = {
                  ...form,
                  filter: {
                    senders: form.senders
                      .split(",")
                      .map((s: string) => s.trim())
                      .filter(Boolean),
                    ...Object.fromEntries(
                      [
                        "domains",
                        "excludedSenders",
                        "subjectKeywords",
                        "reservationKeywords",
                      ].map((k) => [
                        k,
                        form[k]
                          .split(",")
                          .map((s: string) => s.trim())
                          .filter(Boolean),
                      ]),
                    ),
                    ...Object.fromEntries(
                      ["afterDate", "beforeDate", "query", "labelOrFolder"].map(
                        (k) => [k, form[k] || null],
                      ),
                    ),
                  },
                };
                for (const key of [
                  "password",
                  "clientSecret",
                  "certificatePem",
                ])
                  if (!payload[key]) delete payload[key];
                return accountRpc("email.configure", payload).then((result) => {
                  setForm((f: any) => ({
                    ...f,
                    password: "",
                    clientSecret: "",
                  }));
                  return result;
                });
              }, "Connection settings saved securely.")
            }
          >
            Save connection
          </button>
          <button
            className="primary"
            disabled={busy}
            onClick={() =>
              run(
                () => accountRpc("email.connect"),
                imap
                  ? "Connection checked."
                  : "Complete authorization in your browser; this panel will update.",
              )
            }
          >
            <KeyRound size={15} />
            {busy ? "Working…" : "Connect mailbox"}
          </button>
          <button
            className="text-button"
            onClick={() => setStep(step === 1 ? 2 : 3)}
          >
            Continue
          </button>
        </div>
      )}
      {step === 3 && (
        <>
          {status?.configured && (
            <div className="inline-actions">
              <button
                className="secondary"
                disabled={busy || status.syncing}
                onClick={() =>
                  run(
                    () => accountRpc("email.sync"),
                    "Mailbox sync requested. Review imported drafts in the import inbox.",
                  )
                }
              >
                <RefreshCw size={14} />
                Sync now
              </button>
              <button
                className="secondary"
                disabled={busy}
                onClick={() =>
                  run(
                    () => accountRpc("email.pause", { paused: !status.paused }),
                    status.paused ? "Import resumed." : "Import paused.",
                  )
                }
              >
                <Pause size={14} />
                {status.paused ? "Resume" : "Pause"}
              </button>
              <button
                className="text-button danger"
                disabled={busy}
                onClick={() =>
                  run(
                    () => accountRpc("email.disconnect"),
                    "Disconnected. Imported local records and evidence are retained.",
                  )
                }
              >
                Disconnect
              </button>
            </div>
          )}
          <dl className="sync-statistics">
            <div>
              <dt>Last attempt</dt>
              <dd>
                {status?.lastAttempt
                  ? new Date(status.lastAttempt).toLocaleString()
                  : "Not attempted"}
              </dd>
            </div>
            <div>
              <dt>Last successful sync</dt>
              <dd>
                {status?.lastSync
                  ? new Date(status.lastSync).toLocaleString()
                  : "Not synchronized"}
              </dd>
            </div>
            {[
              ["examined", "Messages examined"],
              ["candidates", "Booking candidates"],
              ["drafts", "New review drafts"],
            ].map(([key, label]) => (
              <div key={key}>
                <dt>{label}</dt>
                <dd>{status?.statistics?.[key] ?? "—"}</dd>
              </div>
            ))}
          </dl>
          <button className="primary" onClick={onImports}>
            Open Import Center
          </button>
          <p className="field-help">
            Disconnect removes this account’s protected credentials. Previously
            imported local bookings, drafts and evidence remain available.
          </p>
        </>
      )}
      <p className="field-help">
        Automatic retrieval runs while the desktop host is running and online. A
        live mailbox is verified only after successful authorization and
        synchronization.
      </p>
    </>
  );
}
