import { useEffect, useState } from "react";
import { rpc, Settings } from "./api";
export function ModelManager({
  settings,
  onRefresh,
}: {
  settings: Settings;
  onRefresh: () => Promise<void>;
}) {
  const [catalog, setCatalog] = useState<any>();
  const [model, setModel] = useState("0.6b");
  const [status, setStatus] = useState<any>();
  const [operation, setOperation] = useState<any>();
  const [approve, setApprove] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [limits, setLimits] = useState({ ...settings.ai });
  async function refresh() {
    const [s, o] = await Promise.all([
      rpc("ai.status"),
      rpc("ai.install_status"),
    ]);
    setStatus(s);
    setOperation(o);
  }
  useEffect(() => {
    rpc("ai.models")
      .then(setCatalog)
      .catch((e) => setMessage(String(e)));
    void refresh();
    const timer = setInterval(() => void refresh().catch(() => {}), 1000);
    return () => clearInterval(timer);
  }, []);
  async function run(method: string, extra: any = {}) {
    setBusy(true);
    setMessage("");
    try {
      const result = await rpc(method, { model, ...extra });
      setMessage(
        method === "ai.test"
          ? `${result.message} · Measured response: ${result.elapsedMs} ms on this computer.`
          : {
              "ai.install": "Verified model installed. Enable it when ready.",
              "ai.verify": "Model and runtime files verified.",
              "ai.enable": "Local AI enabled.",
              "ai.remove": "Selected model removed.",
              "settings.save": "Local AI settings saved.",
            }[method] || "Completed.",
      );
      await onRefresh();
      await refresh();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  const selected = catalog?.models?.[model];
  const pending = busy || operation?.busy;
  return (
    <section
      className="panel-stack model-manager"
      aria-label="Local AI model installation"
    >
      <h3>Local AI</h3>
      <p>
        Atlas Planner works with AI off. Models run on your computer and are
        kept outside the application installation.
      </p>
      <label className="field">
        Supported model
        <select
          value={model}
          disabled={pending}
          onChange={(e) => {
            setModel(e.target.value);
            setApprove(false);
          }}
        >
          {Object.entries(catalog?.models || {}).map(([key, m]: any) => (
            <option key={key} value={key}>
              {m.name}
            </option>
          ))}
        </select>
      </label>
      {selected && (
        <div className="info-note">
          <p>
            {selected.bytes.toLocaleString()} model bytes +{" "}
            {catalog?.runtime?.bytes?.toLocaleString() || "unsupported"} runtime
            bytes
          </p>
          <p>{catalog?.requirements?.[model]}</p>
          <p>
            Model: Apache-2.0 · Runtime: MIT · Official pinned revisions and
            SHA-256 verification
          </p>
          <small>{selected.revision}</small>
        </div>
      )}
      <label className="check-field">
        <input
          type="checkbox"
          checked={approve}
          onChange={(e) => setApprove(e.target.checked)}
        />
        I approve downloading the displayed model and runtime.
      </label>
      <div className="button-row">
        <button
          className="secondary"
          disabled={pending || !approve || !catalog?.runtime}
          onClick={() => run("ai.install", { approveDownload: true })}
        >
          Download model
        </button>
        <button
          className="secondary"
          disabled={pending}
          onClick={() => run("ai.verify")}
        >
          Verify model
        </button>
        <button
          className="secondary"
          disabled={pending}
          onClick={() => run("ai.enable")}
        >
          Enable verified model
        </button>
        <button
          className="secondary"
          disabled={pending}
          onClick={() => run("ai.test")}
        >
          Test model
        </button>
      </div>
      <p role="status">
        {status?.running
          ? "Running"
          : status?.enabled
            ? "Enabled, idle"
            : "Disabled"}{" "}
        · {status?.busy ? "Inference in progress" : "No inference job"}
      </p>
      {operation?.progress && (
        <div role="status">
          <p>
            {operation.progress.phase} · {operation.progress.message}
          </p>
          {operation.progress.total > 0 && (
            <>
              <progress
                aria-label="Model download progress"
                max={operation.progress.total}
                value={operation.progress.downloaded}
              />
              <p className="numeric">
                {operation.progress.downloaded.toLocaleString()} /{" "}
                {operation.progress.total.toLocaleString()} bytes
              </p>
            </>
          )}
        </div>
      )}
      <div className="button-row">
        <button
          className="secondary"
          onClick={() => {
            void rpc("ai.cancel");
            void rpc("ai.install_cancel");
          }}
        >
          Cancel / stop runtime
        </button>
        <button
          className="secondary"
          disabled={pending}
          onClick={() =>
            run("settings.save", { settings: { ai: { enabled: false } } })
          }
        >
          Disable Local AI
        </button>
      </div>
      <details>
        <summary>Remove the selected model</summary>
        <p>
          Saved travel records are retained. Shared runtime files are retained
          for other installed models.
        </p>
        <button
          className="secondary"
          disabled={pending}
          onClick={() => run("ai.remove", { confirm: true })}
        >
          Confirm removal
        </button>
      </details>
      <details>
        <summary>Resource limits</summary>
        {[
          ["threads", "CPU threads", 1, 4, 2],
          ["contextTokens", "Context tokens", 1024, 8192, 4096],
          ["maxOutputTokens", "Output token cap", 64, 1024, 700],
          ["idleUnloadSeconds", "Idle shutdown seconds", 30, 600, 120],
        ].map(([key, label, min, max, defaultValue]) => (
          <label className="field" key={key}>
            {label}
            <input
              type="number"
              min={min as number}
              max={max as number}
              value={limits[key as string] ?? defaultValue}
              onChange={(e) =>
                setLimits({ ...limits, [key]: Number(e.target.value) })
              }
            />
          </label>
        ))}
        <button
          className="secondary"
          disabled={pending}
          onClick={() =>
            run("settings.save", {
              settings: {
                ai: {
                  threads: limits.threads ?? 2,
                  contextTokens: limits.contextTokens ?? 4096,
                  maxOutputTokens: limits.maxOutputTokens ?? 700,
                  idleUnloadSeconds: limits.idleUnloadSeconds ?? 120,
                },
              },
            })
          }
        >
          Save resource limits
        </button>
      </details>
      {message && (
        <p role="status" className="info-note preserve-lines">
          {message}
        </p>
      )}
    </section>
  );
}
