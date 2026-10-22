import { useEffect, useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { isDesktop, rpc } from "./api";
import { APP_VERSION } from "./version";

export function UpdatesPanel({
  hasUnsaved,
  onStatus,
}: {
  hasUnsaved: boolean;
  onStatus: (status: any) => void;
}) {
  const [status, setStatus] = useState<any>(null);
  const [history, setHistory] = useState<any>(null);
  const [phase, setPhase] = useState("");
  const [progress, setProgress] = useState<any>(null);
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const installing = !!phase && !["Failed", "Checking"].includes(phase);
  async function check() {
    setChecking(true);
    setError("");
    const releaseRequest = rpc("releases.history", { refresh: true })
      .then(setHistory)
      .catch((e) => setError(String(e)));
    try {
      const result = isDesktop()
        ? await (
            await import("@tauri-apps/api/core")
          ).invoke<any>("update_status")
        : {
            configured: false,
            available: false,
            currentVersion: APP_VERSION,
            reason:
              "Install the desktop app to use signed updates. Published release history is informational.",
          };
      setStatus(result);
      onStatus(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
    await releaseRequest;
  }
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    if (isDesktop())
      void import("@tauri-apps/api/event").then(async ({ listen }) => {
        const off = await listen<any>("atlas:update-progress", (event) => {
          const p = event.payload;
          setPhase(p.phase);
          setProgress(p);
          if (p.error) setError(p.error);
        });
        if (alive) stop = off;
        else off();
      });
    void check();
    return () => {
      alive = false;
      stop?.();
    };
  }, []);
  async function install() {
    if (hasUnsaved || installing || checking) return;
    setError("");
    setPhase("Preparing backup");
    setProgress(null);
    try {
      await (await import("@tauri-apps/api/core")).invoke("update_install");
    } catch (e) {
      setError(String(e));
      setPhase("Failed");
    }
  }
  return (
    <div className="panel-stack" aria-label="Update status">
      <section>
        <span className="eyebrow">INSTALLED</span>
        <h3>Atlas {status?.currentVersion || APP_VERSION}</h3>
      </section>
      {error && (
        <p role="alert" className="error-banner">
          {error}
        </p>
      )}
      {status?.available ? (
        <section className="info-note">
          <span className="eyebrow">LATEST COMPATIBLE SIGNED UPDATE</span>
          <h3>Version {status.version}</h3>
          {status.date && (
            <small>{new Date(status.date).toLocaleDateString()}</small>
          )}
          <p className="preserve-lines">
            {status.notes || "A newer compatible release is available."}
          </p>
          <p>
            Atlas backs up saved data, verifies the update, installs it and
            restarts.
          </p>
          {hasUnsaved && (
            <p role="status">Save or discard open edits before updating.</p>
          )}
          <button
            className="primary"
            disabled={installing || checking || hasUnsaved}
            onClick={install}
          >
            <Download size={15} />
            {installing ? phase : "Update Now"}
          </button>
        </section>
      ) : (
        <p className="info-note">
          {checking
            ? "Checking for updates…"
            : status?.configured
              ? "Atlas is up to date on this build’s signed update feed."
              : status?.reason || "Checking update configuration…"}
        </p>
      )}
      {phase && (
        <div role="status">
          <p>
            {phase === "Failed"
              ? "Update failed. Saved data and its backup remain available."
              : phase}
          </p>
          {progress?.downloaded != null && (
            <p className="numeric">
              {progress.downloaded.toLocaleString()} bytes
              {progress.total
                ? ` of ${progress.total.toLocaleString()}`
                : " downloaded"}
            </p>
          )}
          {progress?.total > 0 && phase === "Downloading" && (
            <progress
              aria-label="Download progress"
              value={progress.downloaded}
              max={progress.total}
            />
          )}
        </div>
      )}
      <button
        className="secondary"
        disabled={checking || installing}
        onClick={check}
      >
        <RefreshCw size={14} />
        {checking ? "Checking…" : "Check for updates"}
      </button>
      <section>
        <h3>Published release history</h3>
        {history?.warning && (
          <p className="muted">
            {history.lastChecked ? "Showing cached metadata. " : ""}
            {history.warning}
          </p>
        )}
        {history?.lastChecked && (
          <small>
            Last checked: {new Date(history.lastChecked).toLocaleString()}
          </small>
        )}
        {history?.releases?.length === 0 && (
          <p className="muted">
            {history.warning
              ? "Published releases could not be retrieved. No cached history is available."
              : "No published releases yet. CI installer artifacts are development previews."}
          </p>
        )}
        {history?.releases?.map((r: any) => (
          <details className="panel-card" key={r.version}>
            <summary>
              <strong>{r.version}</strong> · {r.channel}
              {r.current ? " · Installed" : ""}
              <br />
              <small>
                {new Date(r.date).toLocaleDateString()} · {r.summary}
              </small>
            </summary>
            <p className="release-notes">
              {r.notes || "No release notes provided."}
            </p>
          </details>
        ))}
      </section>
    </div>
  );
}
