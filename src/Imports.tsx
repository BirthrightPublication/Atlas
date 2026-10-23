import { useEffect, useState } from "react";
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
import { MoneyField } from "./RecordForm";
import { AtlasRecord, rpc, money } from "./api";

export const SYNTHETIC_CONFIRMATION = `Booking confirmation\nProvider: Example Stays\nReservation ID: SYNTHETIC-ATLAS-001\nProperty: Riverside apartment\nAddress: Example Street, Lisbon, Portugal\nCheck-in: 2026-12-12\nCheck-out: 2027-01-11\nTimezone: Europe/Lisbon\nTotal: USD 900.00\nRefrigerator: yes\nWorkspace: yes\nStatus: confirmed\nThis is synthetic sample data for testing.`;

export function ImportPanel({
  records,
  onClose,
  onRefresh,
  onReview,
  onConnections,
  onSelect,
}: {
  records: AtlasRecord[];
  onClose: () => void;
  onRefresh: () => Promise<void>;
  onReview: (draft: AtlasRecord) => void;
  onConnections: () => void;
  onSelect: (record: AtlasRecord) => void;
}) {
  const [tab, setTab] = useState("Pending Review");
  const [limit, setLimit] = useState(30);
  const [adding, setAdding] = useState(false);
  const [text, setText] = useState("");
  const [parsedText, setParsedText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const drafts = records.filter(
    (r) =>
      r.kind === "draft" &&
      (tab === "Processed"
        ? r.status === "accepted"
        : tab === "Dismissed"
          ? r.status === "dismissed"
          : r.status === "pending" &&
            (tab === "Changes" ? !!r.existingRecordId : !r.existingRecordId)),
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
    <section className="import-center" aria-label="Booking Import Center">
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
        <h1>Booking Import Center</h1>
        <p>
          Paste a confirmation, save a link, or bring in a document. You review
          what becomes part of your journey.
        </p>
      </div>
      <nav className="connection-steps" aria-label="Import sections">
        {[
          "Pending Review",
          "Changes",
          "Processed",
          "Dismissed",
          "Connections",
        ].map((t) => (
          <button
            key={t}
            className={tab === t ? "active" : ""}
            onClick={() => {
              setTab(t);
              setLimit(30);
            }}
          >
            {t}
          </button>
        ))}
      </nav>
      {tab === "Connections" ? (
        <div className="import-card">
          <h3>Connected mailboxes</h3>
          <p>
            Connect, pause and filter accounts in Email & Integrations.
            Authorized mail arrives here as review drafts.
          </p>
          <button className="primary" onClick={onConnections}>
            Manage connections
          </button>
        </div>
      ) : (
        <div className="detail-content">
          <button className="secondary" onClick={() => setAdding(!adding)}>
            {adding ? "Hide manual import" : "Paste or upload a booking"}
          </button>
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
          {adding && (
            <>
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
                  Run ordinary parsing first. A configured, verified local
                  runtime can propose remaining fields, which still require
                  evidence and review. No hosted AI is used.
                </p>
                <button
                  className="secondary"
                  disabled={busy || !parsedText}
                  onClick={() =>
                    run(() => rpc("imports.ai", { text: parsedText }))
                  }
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
            </>
          )}
          <div className="section-divider" />
          <div className="section-heading">
            <h3>{tab}</h3>
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
            drafts
              .slice(0, limit)
              .map((d) => (
                <ReviewCard
                  key={`${d.id}-${d.version}`}
                  draft={d}
                  records={records}
                  onReview={onReview}
                  onRefresh={onRefresh}
                  onSelect={onSelect}
                />
              ))
          )}
          {drafts.length > limit && (
            <button className="secondary" onClick={() => setLimit(limit + 30)}>
              Load more
            </button>
          )}
          {tab === "Processed" &&
            records
              .filter((r) => r.kind === "refund")
              .map((r) => (
                <RefundCard
                  key={`${r.id}-${r.version}`}
                  record={r}
                  records={records}
                  onRefresh={onRefresh}
                />
              ))}
        </div>
      )}
    </section>
  );
}

function ReviewCard({
  draft,
  records,
  onReview,
  onRefresh,
  onSelect,
}: {
  draft: AtlasRecord;
  records: AtlasRecord[];
  onReview: (d: AtlasRecord) => void;
  onRefresh: () => Promise<void>;
  onSelect: (r: AtlasRecord) => void;
}) {
  const [preview, setPreview] = useState<any>(null),
    [fields, setFields] = useState<string[]>([]),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const r = draft.record || draft.candidate || {};
  const source = records.find((s) => s.id === draft.sourceId);
  async function review() {
    setBusy(true);
    try {
      const v = await rpc("imports.preview", { id: draft.id });
      setPreview(v);
      setFields(v.changes.map((c: any) => c.field));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function act(method: string, payload: any) {
    setBusy(true);
    setError("");
    try {
      await rpc(method, { id: draft.id, ...payload });
      await onRefresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <article className="import-card">
      <div className="record-label">
        <FileText size={15} />
        <span>
          {r.kind || "Booking"} · {draft.action || "review"}
        </span>
        <span className="spacer" />
        <small>{draft.status}</small>
      </div>
      <h3>
        {r.name ||
          records.find((b) => b.id === draft.existingRecordId)?.name ||
          "Booking details need review"}
      </h3>
      <p>
        {r.provider || "Provider unknown"} ·{" "}
        {r.reservationId || "Reference unknown"}
      </p>
      <p>
        {r.checkIn || r.departure || "Date unknown"} →{" "}
        {r.checkOut || r.arrival || "Unknown"} ·{" "}
        {r.address || r.destination || "Destination unknown"}
      </p>
      <p>
        {money(r.totalMinor, r.currency)} ·{" "}
        {r.status || "Confirmation state unknown"}
      </p>
      <small>
        Source: {source?.metadata?.provider || "Manual import"} ·{" "}
        {new Date(
          source?.metadata?.receivedAt || source?.createdAt || draft.createdAt,
        ).toLocaleString()}
      </small>
      {draft.refund && (
        <div className="info-note">
          Refund {draft.refund.state}:{" "}
          {money(draft.refund.amountMinor, draft.refund.currency)}. Approving
          records the notice; recording an actual receipt is a separate decision
          in Processed.
        </div>
      )}
      {error && (
        <p className="error-banner" role="alert">
          {error}
        </p>
      )}
      <details>
        <summary>Missing fields and source evidence</summary>
        <ul className="issue-list">
          {draft.issues?.map((i: any, n: number) => (
            <li key={n}>{typeof i === "string" ? i : JSON.stringify(i)}</li>
          ))}
        </ul>
        <pre className="evidence">
          {JSON.stringify(draft.evidence || draft.sourceText || {}, null, 2)}
        </pre>
      </details>
      {preview && (
        <section className="revision-review">
          <h4>
            {preview.before
              ? "Choose the changes to accept"
              : "Review extracted fields"}
          </h4>
          <p className="field-help">
            {preview.before
              ? "Unchecked fields keep the saved value. Missing information never erases existing facts."
              : "Check the extracted details against your confirmation. Use Edit fields to correct missing or ambiguous information."}
          </p>
          <div className="revision-fields">
            {preview.changes.map((c: any) => (
              <label className="revision-field" key={c.field}>
                {preview.before ? (
                  <input
                    type="checkbox"
                    checked={fields.includes(c.field)}
                    onChange={(e) =>
                      setFields((f) =>
                        e.target.checked
                          ? [...f, c.field]
                          : f.filter((k) => k !== c.field),
                      )
                    }
                  />
                ) : (
                  <Check size={14} />
                )}
                <span>
                  <strong>{fieldLabel(c.field)}</strong>
                  <small>{c.state || "Review required"}</small>
                  <span className="change-values">
                    <del>
                      {c.field.endsWith("Minor") && typeof c.before === "number"
                        ? money(c.before, preview.before?.currency)
                        : showValue(c.before)}
                    </del>
                    <ArrowRight size={14} />
                    <b>
                      {c.field.endsWith("Minor") && typeof c.after === "number"
                        ? money(c.after, preview.candidate?.currency)
                        : showValue(c.after)}
                    </b>
                  </span>
                  <small>
                    {typeof c.evidence === "string"
                      ? c.evidence
                      : "Review the source before accepting this field."}
                  </small>
                </span>
              </label>
            ))}
          </div>
          <details>
            <summary>Planning effects and audit history</summary>
            <p>{preview.notice}</p>
            {preview.effects?.needsReview && (
              <p className="error-banner">{preview.effects.needsReview}</p>
            )}
            {preview.effects?.nights != null && (
              <p>
                {preview.effects.nights} occupied nights · checkout is exclusive
              </p>
            )}
            <ul>
              {[
                ...(preview.effects?.warnings || []),
                ...(preview.effects?.work?.conflicts || []),
              ].map((w: any, i: number) => (
                <li key={i}>
                  {w.message || "Review the saved work and travel times."}
                </li>
              ))}
              {preview.gaps?.ranges?.map((g: any, i: number) => (
                <li key={`gap-${i}`}>
                  {g.start} → {g.end}: {g.nights} nights · {g.status}
                </li>
              ))}
            </ul>
            {preview.history?.length > 0 && (
              <>
                <h4>Saved revisions</h4>
                <ul>
                  {preview.history.map((h: any, i: number) => (
                    <li key={i}>
                      {new Date(h.at).toLocaleString()} · {h.action} ·{" "}
                      {h.record.name} · {h.record.checkIn || h.record.departure}{" "}
                      → {h.record.checkOut || h.record.arrival} ·{" "}
                      {money(h.record.totalMinor, h.record.currency)}
                    </li>
                  ))}
                </ul>
              </>
            )}
          </details>
          <div className="inline-actions">
            <button
              className="primary"
              disabled={busy}
              onClick={() =>
                act("imports.accept", {
                  fields: preview.before ? fields : undefined,
                  expectedVersion: preview.before?.version,
                })
              }
            >
              Accept {preview.before ? "selected changes" : "booking"}
            </button>
            <button
              className="secondary"
              onClick={() => onReview({ ...draft, record: preview.candidate })}
            >
              Edit fields
            </button>
            {preview.before && (
              <button
                className="text-button"
                disabled={busy}
                onClick={() => act("imports.dismiss", {})}
              >
                Keep existing booking
              </button>
            )}
          </div>
        </section>
      )}
      <div className="inline-actions">
        {draft.status === "pending" && (
          <>
            <button
              className="text-button muted"
              disabled={busy}
              onClick={() => act("imports.dismiss", {})}
            >
              Dismiss
            </button>
            {!preview && (
              <button className="secondary" disabled={busy} onClick={review}>
                {busy ? "Opening…" : "Review fields"}
                <ArrowRight size={14} />
              </button>
            )}
          </>
        )}
        {draft.acceptedRecordId && (
          <button
            className="secondary"
            onClick={() => {
              const booking = records.find(
                (b) => b.id === draft.acceptedRecordId,
              );
              if (booking) onSelect(booking);
            }}
          >
            View booking
          </button>
        )}
      </div>
    </article>
  );
}
const fieldLabel = (key: string) =>
  ({
    totalMinor: "Total payable",
    checkIn: "Check-in",
    checkOut: "Checkout",
    reservationId: "Reservation reference",
    sourceIds: "Source evidence",
    documentIds: "Documents",
    facilities: "Facilities",
    cancellationAt: "Cancellation deadline",
    paymentDueAt: "Payment due",
  })[key] ||
  key.replace(/([A-Z])/g, " $1").replace(/^./, (c) => c.toUpperCase());
const showValue = (value: any) =>
  value == null
    ? "Unknown"
    : typeof value === "object"
      ? Object.entries(value)
          .map(
            ([k, v]) =>
              `${fieldLabel(k)}: ${v === true ? "Yes" : v === false ? "No" : v == null ? "Unknown" : String(v)}`,
          )
          .join(" · ")
      : String(value);
function RefundCard({
  record,
  records,
  onRefresh,
}: {
  record: AtlasRecord;
  records: AtlasRecord[];
  onRefresh: () => Promise<void>;
}) {
  const payments = records.filter(
    (p) =>
      p.kind === "payment" &&
      p.bookingId === record.bookingId &&
      p.status === "settled" &&
      ["payment", "deposit"].includes(p.type),
  );
  const [paymentId, setPaymentId] = useState(payments[0]?.id || ""),
    [amount, setAmount] = useState<number | undefined>(record.amountMinor),
    [date, setDate] = useState(new Date().toISOString().slice(0, 10)),
    [confirmed, setConfirmed] = useState(false),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const currency =
    payments.find((p) => p.id === paymentId)?.currency ||
    record.currency ||
    "USD";
  return (
    <article className="import-card">
      <h3>{record.name}</h3>
      <p>
        {record.state} · expected {money(record.amountMinor, record.currency)} ·
        received{" "}
        {money(
          record.receivedMinor || 0,
          record.receivedCurrency || record.currency,
        )}
      </p>
      {record.state !== "received" && (
        <>
          <p className="field-help">
            Record money only after it reaches your account. Original payments
            remain in history.
          </p>
          <div className="form-grid">
            <label className="field">
              <span>Original payment</span>
              <select
                value={paymentId}
                onChange={(e) => setPaymentId(e.target.value)}
              >
                <option value="">Choose a payment</option>
                {payments.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.date} · {money(p.amountMinor, p.currency)}
                  </option>
                ))}
              </select>
            </label>
            <MoneyField
              label="Amount received"
              value={amount}
              currency={currency}
              onChange={setAmount}
            />
            <label className="field">
              <span>Receipt date</span>
              <input
                type="date"
                value={date}
                onChange={(e) => setDate(e.target.value)}
              />
            </label>
          </div>
          <label className="check-row">
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(e) => setConfirmed(e.target.checked)}
            />
            I verified the money was received
          </label>
          <button
            className="primary"
            disabled={busy || !confirmed || !paymentId}
            onClick={async () => {
              setBusy(true);
              try {
                await rpc("refunds.receive", {
                  id: record.id,
                  expectedVersion: record.version,
                  paymentId,
                  amountMinor: amount,
                  currency,
                  date,
                  confirmReceived: confirmed,
                  requestId: crypto.randomUUID(),
                });
                await onRefresh();
              } catch (e) {
                setError(String(e));
              } finally {
                setBusy(false);
              }
            }}
          >
            Record refund receipt
          </button>
        </>
      )}
      {error && (
        <p role="alert" className="error-banner">
          {error}
        </p>
      )}
    </article>
  );
}
