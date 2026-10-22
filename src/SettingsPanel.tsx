import { FormEvent, useEffect, useRef, useState } from "react";
import {
  X,
  Save,
  Mail,
  ExternalLink,
  RefreshCw,
  Pause,
  Shield,
  Database,
  Download,
  Smartphone,
  Clock,
  Settings2,
  Check,
  AlertCircle,
  KeyRound,
} from "lucide-react";
import { Settings, isDesktop, money, openExternal, rpc } from "./api";
import { MoneyField } from "./RecordForm";
import { AiSettings, PhoneSettings } from "./Integrations";

export function SettingsPanel({
  settings,
  onClose,
  onRefresh,
}: {
  settings: Settings;
  onClose: () => void;
  onRefresh: () => Promise<void>;
}) {
  const [tab, setTab] = useState("Preferences");
  const [form, setForm] = useState<Settings>({
    ...settings,
    preferences: { ...settings.preferences },
  });
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [restoreFile, setRestoreFile] = useState("");
  const set = (key: string, value: any) =>
    setForm((f) => ({ ...f, [key]: value }));
  const pref = (key: string, value: any) =>
    set("preferences", { ...form.preferences, [key]: value });
  async function run(fn: () => Promise<any>, message: string) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const result = await fn();
      await onRefresh();
      setNotice(
        typeof result?.path === "string"
          ? `${message} ${result.path}`
          : message,
      );
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }
  async function save(e: FormEvent) {
    e.preventDefault();
    await run(
      () => rpc("settings.save", { settings: form }),
      "Preferences saved.",
    );
  }
  return (
    <div
      className="drawer-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <aside
        className="drawer wide-drawer"
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
      >
        <div className="drawer-top">
          <span className="eyebrow">MAKE ATLAS YOUR OWN</span>
          <button
            className="icon-button"
            aria-label="Close settings"
            onClick={onClose}
          >
            <X size={20} />
          </button>
        </div>
        <div className="drawer-heading">
          <h2>A little more you.</h2>
          <p>Personal defaults, your connections, and your local data.</p>
        </div>
        <div className="settings-tabs">
          {[
            "Preferences",
            "Work & clocks",
            "Email import",
            "Data & recovery",
            "Phone",
          ].map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={tab === t ? "active" : ""}
            >
              {t}
            </button>
          ))}
        </div>
        <div className="detail-content">
          {error && (
            <div role="alert" className="error-banner">
              {error}
            </div>
          )}
          {notice && (
            <div role="status" className="success-banner">
              <Check size={15} />
              {notice}
            </div>
          )}
          {tab === "Email import" ? (
            <EmailSettings />
          ) : tab === "Data & recovery" ? (
            <>
              <div className="settings-feature">
                <Database size={24} />
                <div>
                  <h3>Your data lives with you.</h3>
                  <p>
                    Records, documents, and backups are stored in your OS
                    user-data directory. Plain SQLite is not encrypted. Protect
                    your device with OS encryption and a strong account
                    password.
                  </p>
                </div>
              </div>
              <button
                className="secondary full"
                disabled={busy}
                onClick={() => run(() => rpc("backup.export"), "Backup saved:")}
              >
                <Download size={16} />
                Create a local backup
              </button>
              <h3 className="form-section">Restore a local backup</h3>
              <p className="field-help">
                Choose the basename of an Atlas backup from its own backups
                folder. Atlas creates a safety backup before restoring. Close
                other Atlas clients first.
              </p>
              <label className="field">
                <span>Backup filename</span>
                <input
                  value={restoreFile}
                  onChange={(e) => setRestoreFile(e.target.value)}
                  placeholder="atlas-backup-…"
                />
              </label>
              <button
                className="secondary"
                disabled={busy || !restoreFile}
                onClick={() =>
                  run(
                    () => rpc("backup.restore", { filename: restoreFile }),
                    "Backup restored.",
                  )
                }
              >
                Restore backup
              </button>
              <h3 className="form-section">Local AI</h3>
              <AiSettings settings={settings} onRefresh={onRefresh} />
            </>
          ) : tab === "Phone" ? (
            <PhoneSettings />
          ) : (
            <form onSubmit={save}>
              {tab === "Preferences" && (
                <>
                  <div className="form-grid">
                    <label className="field">
                      <span>Display currency</span>
                      <input
                        value={form.currency || "USD"}
                        maxLength={3}
                        onChange={(e) =>
                          set("currency", e.target.value.toUpperCase())
                        }
                      />
                    </label>
                    <label className="field">
                      <span>New entry context</span>
                      <select
                        value={form.context || "on_foot"}
                        onChange={(e) => set("context", e.target.value)}
                      >
                        <option value="on_foot">On Foot</option>
                        <option value="on_road">On Road</option>
                      </select>
                    </label>
                  </div>
                  <label className="field">
                    <span>Confirmed current location</span>
                    <input
                      value={form.currentLocation || ""}
                      onChange={(e) => set("currentLocation", e.target.value)}
                      placeholder="City or area you are actually in"
                    />
                  </label>
                  <label className="field">
                    <span>Current local timezone</span>
                    <input
                      value={
                        form.currentTimezone ||
                        Intl.DateTimeFormat().resolvedOptions().timeZone
                      }
                      onChange={(e) => set("currentTimezone", e.target.value)}
                      placeholder="Europe/Lisbon"
                    />
                  </label>
                  <p className="field-help">
                    Your confirmed location is separate from future stay
                    locations. Atlas does not infer it from your IP address.
                  </p>
                  <h3 className="form-section">Stay preferences</h3>
                  <label className="field">
                    <span>Preset name</span>
                    <input
                      value={form.preferences?.name || ""}
                      onChange={(e) => pref("name", e.target.value)}
                      placeholder="My travel preferences"
                    />
                  </label>
                  <label className="field">
                    <span>Preferred accommodation</span>
                    <select
                      value={form.preferences?.preferredType || "any"}
                      onChange={(e) => pref("preferredType", e.target.value)}
                    >
                      <option value="any">Any accommodation</option>
                      <option value="entire">
                        Entire apartments and homes
                      </option>
                      <option value="room">Private rooms</option>
                      <option value="vehicle">Vehicle / nature stays</option>
                    </select>
                  </label>
                  <div className="form-grid">
                    <MoneyField
                      label="Monthly accommodation target"
                      value={form.preferences?.monthlyTargetMinor || 0}
                      currency={form.currency || "USD"}
                      onChange={(v) => pref("monthlyTargetMinor", v)}
                    />
                    <MoneyField
                      label="Stretch cap (requires a reason)"
                      value={form.preferences?.stretchCapMinor || 0}
                      currency={form.currency || "USD"}
                      onChange={(v) => pref("stretchCapMinor", v)}
                    />
                  </div>
                  <p className="field-help">
                    Targets help compare date-specific offers. Atlas never
                    assumes a month is 30 nights or silently increases a cap.
                  </p>
                  {[
                    "refrigeratorRequired",
                    "workspacePriority",
                    "kitchenRequired",
                    "scenicPreferred",
                  ].map((key, i) => (
                    <label className="check-field" key={key}>
                      <input
                        type="checkbox"
                        checked={!!form.preferences?.[key]}
                        onChange={(e) => pref(key, e.target.checked)}
                      />
                      {
                        [
                          "Refrigerator required",
                          "Usable laptop workspace is a priority",
                          "Kitchen required",
                          "Scenic views preferred",
                        ][i]
                      }
                    </label>
                  ))}
                  <label className="field">
                    <span>Surroundings for this preset</span>
                    <select
                      value={form.preferences?.surroundings || "flexible"}
                      onChange={(e) => pref("surroundings", e.target.value)}
                    >
                      {["flexible", "quiet", "social", "urban", "nature"].map(
                        (s) => (
                          <option key={s}>{s}</option>
                        ),
                      )}
                    </select>
                  </label>
                  <label className="field">
                    <span>Interests</span>
                    <textarea
                      value={
                        Array.isArray(form.preferences?.interests)
                          ? form.preferences.interests.join(", ")
                          : form.preferences?.interests || ""
                      }
                      onChange={(e) =>
                        pref(
                          "interests",
                          e.target.value
                            .split(",")
                            .map((s) => s.trim())
                            .filter(Boolean),
                        )
                      }
                      placeholder="Nature, architecture, markets…"
                      rows={3}
                    />
                  </label>
                  <button
                    className="secondary"
                    type="button"
                    onClick={() => {
                      const name = form.preferences?.name || "Unnamed preset";
                      set("presets", [
                        ...(form.presets || []).filter(
                          (p: any) => p.name !== name,
                        ),
                        { ...form.preferences, name },
                      ]);
                      setNotice(
                        "Preset added. Save preferences to persist it.",
                      );
                    }}
                  >
                    Save as reusable preset
                  </button>
                  {(form.presets || []).length > 0 && (
                    <label className="field">
                      <span>Load saved preset</span>
                      <select
                        defaultValue=""
                        onChange={(e) => {
                          if (e.target.value)
                            set("preferences", {
                              ...form.presets[Number(e.target.value)],
                            });
                        }}
                      >
                        <option value="">Choose a preset…</option>
                        {form.presets.map((p: any, i: number) => (
                          <option value={i} key={i}>
                            {p.name}
                          </option>
                        ))}
                      </select>
                    </label>
                  )}
                </>
              )}
              {tab === "Work & clocks" && (
                <>
                  <label className="field">
                    <span>Employer timezone mode</span>
                    <select
                      value={form.officeTimezoneMode || "iana"}
                      onChange={(e) =>
                        set("officeTimezoneMode", e.target.value)
                      }
                    >
                      <option value="iana">
                        IANA timezone (daylight-saving aware)
                      </option>
                      <option value="fixed">Explicit fixed UTC offset</option>
                    </select>
                  </label>
                  {form.officeTimezoneMode === "fixed" && (
                    <label className="field">
                      <span>
                        Fixed UTC offset in minutes (for example −480)
                      </span>
                      <input
                        type="number"
                        min="-840"
                        max="840"
                        value={form.officeFixedOffsetMinutes ?? 0}
                        onChange={(e) =>
                          set(
                            "officeFixedOffsetMinutes",
                            Number(e.target.value),
                          )
                        }
                      />
                    </label>
                  )}

                  <label className="field">
                    <span>Employer timezone (IANA)</span>
                    <input
                      value={form.officeTimezone || "America/Los_Angeles"}
                      onChange={(e) => set("officeTimezone", e.target.value)}
                      required
                    />
                  </label>
                  <div className="form-grid">
                    <label className="field">
                      <span>Office opens</span>
                      <input
                        type="time"
                        value={form.workStart || "08:00"}
                        onChange={(e) => set("workStart", e.target.value)}
                        required
                      />
                    </label>
                    <label className="field">
                      <span>Office closes</span>
                      <input
                        type="time"
                        value={form.workEnd || "17:00"}
                        onChange={(e) => set("workEnd", e.target.value)}
                        required
                      />
                    </label>
                  </div>
                  <div className="form-grid">
                    <label className="field">
                      <span>Hours per employer workday</span>
                      <input
                        type="number"
                        min="0"
                        max="24"
                        value={form.workHours ?? 8}
                        onChange={(e) =>
                          set("workHours", Number(e.target.value))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>Minimum office overlap (hours)</span>
                      <input
                        type="number"
                        min="0"
                        max="24"
                        value={form.overlapHours ?? 4}
                        onChange={(e) =>
                          set("overlapHours", Number(e.target.value))
                        }
                      />
                    </label>
                  </div>
                  <label className="field">
                    <span>Employer workdays</span>
                  </label>
                  <div className="weekday-controls">
                    {["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map(
                      (d, i) => {
                        const days = form.workdays || [1, 2, 3, 4, 5];
                        const day = i + 1;
                        return (
                          <button
                            type="button"
                            key={d}
                            className={days.includes(day) ? "active" : ""}
                            onClick={() =>
                              set(
                                "workdays",
                                days.includes(day)
                                  ? days.filter((n: number) => n !== day)
                                  : [...days, day],
                              )
                            }
                          >
                            {d}
                          </button>
                        );
                      },
                    )}
                  </div>
                  <label className="field">
                    <span>
                      Employer holidays / exceptions (one date per line)
                    </span>
                    <textarea
                      rows={4}
                      value={(form.holidays || []).join("\n")}
                      onChange={(e) =>
                        set(
                          "holidays",
                          e.target.value
                            .split("\n")
                            .map((s) => s.trim())
                            .filter(Boolean),
                        )
                      }
                      placeholder="2026-12-25"
                    />
                  </label>
                  <label className="field">
                    <span>Daily break (minutes)</span>
                    <input
                      type="number"
                      min="0"
                      max="240"
                      value={form.breakMinutes || 0}
                      onChange={(e) =>
                        set("breakMinutes", Number(e.target.value))
                      }
                    />
                  </label>
                  <h3 className="form-section">
                    Personal destination-local limits
                  </h3>
                  <label className="check-field">
                    <input
                      type="checkbox"
                      checked={!!form.localDayRestrictions}
                      onChange={(e) =>
                        set(
                          "localDayRestrictions",
                          e.target.checked
                            ? { start: "07:00", end: "22:00" }
                            : null,
                        )
                      }
                    />
                    Limit suggested work to a local-day window
                  </label>
                  {form.localDayRestrictions && (
                    <div className="form-grid">
                      <label className="field">
                        <span>Earliest local start</span>
                        <input
                          type="time"
                          value={form.localDayRestrictions.start || "07:00"}
                          onChange={(e) =>
                            set("localDayRestrictions", {
                              ...form.localDayRestrictions,
                              start: e.target.value,
                            })
                          }
                        />
                      </label>
                      <label className="field">
                        <span>Latest local end</span>
                        <input
                          type="time"
                          value={form.localDayRestrictions.end || "22:00"}
                          onChange={(e) =>
                            set("localDayRestrictions", {
                              ...form.localDayRestrictions,
                              end: e.target.value,
                            })
                          }
                        />
                      </label>
                    </div>
                  )}
                  <p className="info-note">
                    Office dates govern workday requirements. Destination dates
                    can differ. IANA timezone rules account for daylight-saving
                    changes.
                  </p>
                </>
              )}
              <div className="form-actions">
                <button type="submit" className="primary" disabled={busy}>
                  <Save size={15} />
                  {busy ? "Saving…" : "Save preferences"}
                </button>
              </div>
            </form>
          )}
        </div>
      </aside>
    </div>
  );
}

function EmailSettings() {
  const [status, setStatus] = useState<any>(null);
  const [form, setForm] = useState<any>({
    provider: "proton-bridge",
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
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const initialized = useRef(false);
  const refresh = async () => {
    try {
      const s = await rpc("email.status");
      setStatus(s);
      if (s.configured && !initialized.current) {
        initialized.current = true;
        setForm((f: any) => ({
          ...f,
          ...Object.fromEntries(
            [
              "provider",
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
          afterDate: s.filter?.afterDate || "",
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
  }, []);
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
      <div className="email-status">
        <span className={`status-dot ${status?.connected ? "online" : ""}`} />
        <div>
          <strong>
            {status?.connected
              ? "Mailbox connected"
              : status?.authorizationPending
                ? "Waiting for authorization"
                : "No mailbox connected"}
          </strong>
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
            running local Bridge. Free Proton accounts cannot use this automatic
            import path. Use the Bridge-generated local credentials and export
            its certificate.
          </div>
        </div>
      )}
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
                Restricted Gmail read-only scope requires correct consent setup.
                External apps left in Testing may have seven-day refresh-token
                expiry.
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
      <p className="field-help">
        Read-only import. Filters restrict retrieval, not the OAuth grant. No
        sending or deletion permissions are requested. Passwords and tokens are
        stored in the OS secret store; they are never saved to the project.
      </p>
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
                  afterDate: form.afterDate || null,
                },
              };
              for (const key of ["password", "clientSecret", "certificatePem"])
                if (!payload[key]) delete payload[key];
              return rpc("email.configure", payload).then((result) => {
                setForm((f: any) => ({ ...f, password: "", clientSecret: "" }));
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
              () => rpc("email.connect"),
              imap
                ? "Connection checked."
                : "Complete authorization in your browser; this panel will update.",
            )
          }
        >
          <KeyRound size={15} />
          {busy ? "Working…" : "Connect mailbox"}
        </button>
      </div>
      {status?.configured && (
        <div className="inline-actions">
          <button
            className="secondary"
            disabled={busy || status.syncing}
            onClick={() =>
              run(
                () => rpc("email.sync"),
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
                () => rpc("email.pause", { paused: !status.paused }),
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
              run(() => rpc("email.disconnect"), "Mailbox disconnected.")
            }
          >
            Disconnect
          </button>
        </div>
      )}
      <p className="field-help">
        Automatic retrieval runs while the desktop host is running and online. A
        live mailbox is verified only after successful authorization and
        synchronization.
      </p>
    </>
  );
}

function AiStatus() {
  const [status, setStatus] = useState<any>(null);
  useEffect(() => {
    rpc("ai.status")
      .then(setStatus)
      .catch((e) => setStatus({ message: String(e) }));
  }, []);
  return (
    <div className="info-note">
      <div>
        <strong>
          {status?.available
            ? "Local inference available"
            : "Optional local inference"}
        </strong>
        <p>
          {status?.message ||
            status?.reason ||
            "The calendar, exact calculations, manual entry, and ordinary import parsers work with AI off. Model installation and CPU benchmarking are separate from application updates."}
        </p>
        {status?.model && <small>Model: {status.model}</small>}
      </div>
    </div>
  );
}
