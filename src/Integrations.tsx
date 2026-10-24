import { FormEvent, useEffect, useState } from "react";
import {
  Smartphone,
  Shield,
  Plus,
  X,
  RefreshCw,
  Power,
  Copy,
  Cpu,
  Save,
  Check,
} from "lucide-react";
import { Settings, isDesktop, rpc } from "./api";

export function PhoneSettings() {
  const [origin, setOrigin] = useState("");
  const [status, setStatus] = useState<any>(null);
  const [devices, setDevices] = useState<any[]>([]);
  const [pair, setPair] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function refresh() {
    if (!isDesktop()) return;
    try {
      setStatus(await rpc("companion.status"));
      const result = await rpc("companion.devices");
      setDevices(result.devices || []);
    } catch {
      /* start action exposes actionable errors */
    }
  }
  useEffect(() => {
    void refresh();
  }, []);
  async function action(fn: () => Promise<any>) {
    setBusy(true);
    setError("");
    try {
      const result = await fn();
      await refresh();
      return result;
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <div className="settings-feature">
        <Smartphone size={28} />
        <div>
          <h3>Take the essentials with you.</h3>
          <p>
            The iPhone companion offers an agenda, selected offline documents,
            small expense entry, and an offline change queue. New
            synchronization requires a running, reachable desktop host.
          </p>
        </div>
      </div>
      <p className="info-note">
        Install on iPhone using Safari → Share → Add to Home Screen. A trusted
        private HTTPS address is required. Set up a private HTTPS reverse proxy
        to the local companion service before pairing; the app does not open a
        public server.
      </p>
      {error && (
        <div role="alert" className="error-banner">
          {error}
        </div>
      )}
      {isDesktop() ? (
        <>
          <div className="email-status">
            <span className={`status-dot ${status?.running ? "online" : ""}`} />
            <div>
              <strong>
                {status?.running
                  ? "Companion host running"
                  : "Companion host stopped"}
              </strong>
              <small>{status?.origin || "Private access is optional."}</small>
            </div>
          </div>
          <label className="field">
            <span>Trusted private HTTPS origin</span>
            <input
              type="url"
              value={origin}
              onChange={(e) => setOrigin(e.target.value)}
              placeholder="https://atlas.your-private-domain.example"
            />
          </label>
          <p className="field-help">
            Your iPhone must trust the certificate and reach this address. Point
            the private proxy to Atlas’s loopback service; do not expose it
            without the pairing gateway.
          </p>
          <div className="inline-actions">
            <button
              className="primary"
              disabled={busy || (!origin && !status?.running)}
              onClick={() =>
                action(() =>
                  rpc(
                    status?.running ? "companion.stop" : "companion.start",
                    status?.running ? {} : { origin },
                  ),
                )
              }
            >
              <Power size={15} />
              {status?.running ? "Stop host" : "Start companion host"}
            </button>
            <button
              className="secondary"
              disabled={busy || !status?.running}
              onClick={async () => {
                const result = await action(() => rpc("companion.pair"));
                if (result) setPair(result);
              }}
            >
              <Plus size={15} />
              Pair an iPhone
            </button>
          </div>
          {pair && (
            <div className="pair-code">
              <span>ONE-TIME PAIRING CODE</span>
              <strong>{pair.code}</strong>
              <p>Open {pair.url || origin + "/companion"} on your iPhone.</p>
              <small>
                Expires{" "}
                {pair.expiresAt
                  ? new Date(pair.expiresAt).toLocaleTimeString()
                  : "in a few minutes"}
                . Keep this code private.
              </small>
            </div>
          )}
          <h3 className="form-section">Paired devices</h3>
          {devices.length === 0 ? (
            <p className="field-help">
              No devices paired. Pairing grants access to the authorized
              companion services.
            </p>
          ) : (
            devices.map((device: any) => (
              <div className="device-row" key={device.id}>
                <Smartphone size={17} />
                <div>
                  <strong>{device.name}</strong>
                  <small>
                    {device.revoked
                      ? "Revoked"
                      : device.lastSync
                        ? "Last sync " +
                          new Date(device.lastSync).toLocaleString()
                        : "Not synchronized yet"}
                  </small>
                </div>
                {!device.revoked && (
                  <button
                    className="text-button danger"
                    disabled={busy}
                    onClick={() =>
                      action(() =>
                        rpc("companion.revoke", { deviceId: device.id }),
                      )
                    }
                  >
                    Revoke
                  </button>
                )}
              </div>
            ))
          )}
        </>
      ) : (
        <>
          <a className="primary full" href="/companion">
            <Smartphone size={16} />
            Open companion
          </a>
          <p className="field-help">
            In development, create a one-time pairing code with the companion
            host’s <code>--pair</code> command. See the companion setup guide
            for trusted HTTPS and device management.
          </p>
        </>
      )}
      <p className="field-help">
        Previously synchronized data remains available offline. Pending edits
        reconcile on reconnect with explicit conflict review. The companion has
        controls to remove cached documents and local data.
      </p>
      <p className="field-help">
        A closed PWA cannot promise offline reminder delivery. Export your
        calendar for your phone’s calendar app.
      </p>
    </>
  );
}

export function AiSettings({
  settings,
  onRefresh,
}: {
  settings: Settings;
  onRefresh: () => Promise<void>;
}) {
  const [form, setForm] = useState<any>({ ...settings.ai });
  const [status, setStatus] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const refresh = () =>
    rpc("ai.status")
      .then(setStatus)
      .catch((e) => setError(String(e)));
  useEffect(() => {
    void refresh();
  }, []);
  async function save(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      await rpc("settings.save", { settings: { ai: form } });
      await onRefresh();
      await refresh();
      setNotice(
        "Local inference settings saved. Files are verified before launch.",
      );
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }
  const field = (key: string, label: string, placeholder = "") => (
    <label className="field" key={key}>
      <span>{label}</span>
      <input
        value={form[key] || ""}
        onChange={(e) => setForm((f: any) => ({ ...f, [key]: e.target.value }))}
        placeholder={placeholder}
      />
    </label>
  );
  return (
    <>
      <div className="info-note">
        <Cpu size={20} />
        <div>
          <strong>
            {status?.running
              ? "Local runtime running"
              : status?.configured
                ? "Local runtime configured"
                : "Local inference is optional"}
          </strong>
          <p>
            {status?.message ||
              "Calendar, exact calculations, reminders, and ordinary parsers work with AI off. No model is bundled."}
          </p>
        </div>
      </div>
      <details className="advanced-settings">
        <summary>Configure verified local model files</summary>
        <p className="field-help">
          Obtain a compatible llama.cpp runtime and GGUF from authoritative
          sources, verify their licensing and SHA-256 hashes, and keep them
          outside the repository and program installation. No model is selected
          or downloaded automatically.
        </p>
        <form onSubmit={save}>
          {error && <div className="error-banner">{error}</div>}
          {notice && <div className="success-banner">{notice}</div>}
          <label className="check-field">
            <input
              type="checkbox"
              checked={!!form.enabled}
              onChange={(e) =>
                setForm((f: any) => ({ ...f, enabled: e.target.checked }))
              }
            />
            Enable demand-loaded local inference
          </label>
          {field("modelName", "Model name")}
          {field("runtimePath", "Absolute llama-server executable path")}
          {field("runtimeSha256", "Verified runtime SHA-256")}
          {field("modelPath", "Absolute GGUF model path")}
          {field("modelSha256", "Verified model SHA-256")}
          {field("modelSource", "Official model source URL", "https://…")}
          {field("modelLicense", "Model license / notice reference")}
          <p className="field-help">
            One job at a time, two CPU threads, bounded context/output,
            cancellation, and idle resource release. Model file size does not
            measure runtime RAM. CPU benchmarks are separate from configuration.
          </p>
          <button className="primary" disabled={busy}>
            <Save size={15} />
            Save local AI settings
          </button>
        </form>
      </details>
      {status?.busy && (
        <button
          className="secondary"
          onClick={async () => {
            try {
              await rpc("ai.cancel");
              await refresh();
            } catch (e) {
              setError(String(e));
            }
          }}
        >
          Cancel current inference
        </button>
      )}
    </>
  );
}
