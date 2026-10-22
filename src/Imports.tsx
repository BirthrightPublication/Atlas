import { useState } from "react";
import {
  X,
  Upload,
  ArrowRight,
  FileText,
  AlertCircle,
  Link,
  Inbox,
  Check,
} from "lucide-react";
import { useDialog } from "./useDialog";
import { AtlasRecord, rpc } from "./api";

export const SYNTHETIC_CONFIRMATION = `Booking confirmation\nProvider: Example Stays\nReservation ID: SYNTHETIC-ATLAS-001\nProperty: Riverside apartment\nAddress: Example Street, Lisbon, Portugal\nCheck-in: 2026-12-12\nCheck-out: 2027-01-11\nTimezone: Europe/Lisbon\nTotal: USD 900.00\nRefrigerator: yes\nWorkspace: yes\nStatus: confirmed\nThis is synthetic sample data for testing.`;

export function ImportPanel({
  records,
  onClose,
  onRefresh,
  onReview,
}: {
  records: AtlasRecord[];
  onClose: () => void;
  onRefresh: () => Promise<void>;
  onReview: (draft: AtlasRecord) => void;
}) {
  const dialog = useDialog(onClose);
  const [text, setText] = useState("");
  const [parsedText, setParsedText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const drafts = records.filter(
    (r) => r.kind === "draft" && !["accepted", "dismissed"].includes(r.status),
  );
  async function run(fn: () => Promise<any>) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const result = await fn();
      await onRefresh();
      const list = Array.isArray(result) ? result : result.drafts;
      setParsedText(text || parsedText);
      setNotice(
        list
          ? `${list.length} draft${list.length === 1 ? "" : "s"} ready for review.`
          : "Document stored locally.",
      );
      setText("");
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }
  async function upload(file: File) {
    if (file.size > 15 * 1024 * 1024) {
      setError("Choose a document smaller than 15 MB.");
      return;
    }
    await run(async () => {
      const bytes = new Uint8Array(await file.arrayBuffer());
      let binary = "";
      for (let i = 0; i < bytes.length; i += 8192)
        binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
      return rpc("imports.file", {
        filename: file.name,
        contentBase64: btoa(binary),
      });
    });
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
        ref={dialog}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label="Import bookings"
      >
        <div className="drawer-top">
          <span className="eyebrow">BRING YOUR PLANS TOGETHER</span>
          <button
            className="icon-button"
            aria-label="Close imports"
            onClick={onClose}
          >
            <X size={20} />
          </button>
        </div>
        <div className="drawer-heading">
          <h2>A little less copying.</h2>
          <p>
            Paste a confirmation, save a link, or bring in a document. You
            review what becomes part of your journey.
          </p>
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
          <label className="field">
            <span>Booking text or saved URL</span>
            <textarea
              rows={8}
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="Paste your apartment, hotel, or transport confirmation…"
            />
          </label>
          <div className="inline-actions">
            <label className="secondary upload-label">
              <Upload size={15} />
              Choose file
              <input
                type="file"
                accept=".eml,.ics,.pdf,.txt,.jpg,.jpeg,.png"
                onChange={(e) => {
                  if (e.target.files?.[0]) void upload(e.target.files[0]);
                  e.target.value = "";
                }}
                hidden
                disabled={busy}
              />
            </label>
            <button
              className="primary"
              onClick={() =>
                run(() => rpc("imports.parse", { text, format: "text" }))
              }
              disabled={busy || !text.trim()}
            >
              {busy ? "Reading…" : "Create review drafts"}
              <ArrowRight size={15} />
            </button>
          </div>
          <p className="field-help">
            Text, EML, ICS and text-bearing PDFs. Images are stored as
            attachments; image OCR is not available. Links are saved without
            fetching private booking data.
          </p>
          <button
            className="text-button muted"
            onClick={() => setText(SYNTHETIC_CONFIRMATION)}
          >
            Try a synthetic sample confirmation
          </button>
          <details className="advanced-settings">
            <summary>Resolve ambiguous text with optional local AI</summary>
            <p className="field-help">
              Run ordinary parsing first. A configured, verified local runtime
              can propose remaining fields, which still require evidence and
              review. No hosted AI is used.
            </p>
            <button
              className="secondary"
              disabled={busy || !parsedText}
              onClick={() => run(() => rpc("imports.ai", { text: parsedText }))}
            >
              Extract unresolved text locally
            </button>
            {busy && (
              <button
                className="text-button"
                onClick={() =>
                  rpc("ai.cancel").catch((e) => setError(String(e)))
                }
              >
                Cancel inference
              </button>
            )}
          </details>
          <div className="section-divider" />
          <div className="section-heading">
            <h3>Review inbox</h3>
            <span className="count-badge">{drafts.length}</span>
          </div>
          {drafts.length === 0 ? (
            <div className="empty small">
              <Inbox size={30} />
              <h3>All caught up.</h3>
              <p>
                Imported bookings appear here before they reach your calendar.
              </p>
            </div>
          ) : (
            drafts.map((d) => {
              const r = d.record || d.candidate || {};
              return (
                <article className="import-card" key={d.id}>
                  <div className="record-label">
                    <FileText size={15} />
                    <span>{r.kind || "Booking"} draft</span>
                    <span className="spacer" />
                    <small>
                      {d.updateOf
                        ? "Update to existing booking"
                        : "Needs review"}
                    </small>
                  </div>
                  <h3>{r.name || d.name || "Untitled booking"}</h3>
                  <p>
                    {r.checkIn || r.departure || "Date unknown"}{" "}
                    {r.checkOut ? "→ " + r.checkOut : ""}
                  </p>
                  {d.issues?.length > 0 && (
                    <ul className="issue-list">
                      {d.issues.map((issue: any, i: number) => (
                        <li key={i}>
                          <AlertCircle size={13} />
                          {typeof issue === "string"
                            ? issue
                            : issue.message ||
                              issue.field ||
                              JSON.stringify(issue)}
                        </li>
                      ))}
                    </ul>
                  )}
                  <details>
                    <summary>Source evidence</summary>
                    <pre className="evidence">
                      {typeof d.evidence === "string"
                        ? d.evidence
                        : JSON.stringify(
                            d.evidence || d.sourceText || {},
                            null,
                            2,
                          )}
                    </pre>
                  </details>
                  <div className="inline-actions">
                    <button
                      className="text-button muted"
                      onClick={() =>
                        run(() => rpc("imports.dismiss", { id: d.id }))
                      }
                    >
                      Dismiss
                    </button>
                    <button className="secondary" onClick={() => onReview(d)}>
                      Review fields
                      <ArrowRight size={14} />
                    </button>
                  </div>
                </article>
              );
            })
          )}
        </div>
      </aside>
    </div>
  );
}
