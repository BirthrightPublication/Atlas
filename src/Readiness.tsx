import { useEffect, useState } from "react";
import {
  ArrowLeft,
  ArrowUpRight,
  CheckCircle,
  HelpCircle,
  AlertCircle,
  Clock,
} from "lucide-react";
import { AtlasRecord, rpc } from "./api";
import { AtlasTarget } from "./navigation";
export function Readiness({
  date,
  records,
  onClose,
  onNavigate,
  onCreate,
  onRefresh,
}: {
  date: string;
  records: AtlasRecord[];
  onClose: () => void;
  onNavigate: (t: AtlasTarget) => boolean;
  onCreate: (kind: string) => void;
  onRefresh: () => Promise<void>;
}) {
  const [data, setData] = useState<any>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  useEffect(() => {
    let alive = true;
    rpc("readiness.summary", { start: date })
      .then((v) => {
        if (alive) setData(v);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, [date, records]);
  async function adopt(s: any) {
    setBusy(true);
    try {
      await rpc("reminders.adopt", { suggestionKey: s.suggestionKey });
      await onRefresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="readiness-page">
      <button className="text-button" onClick={onClose}>
        <ArrowLeft size={15} />
        Back to Calendar
      </button>
      <div className="page-intro">
        <span className="eyebrow">YOUR NEXT CHAPTER</span>
        <h1>Trip Readiness</h1>
        <p>
          {data?.ready
            ? "Saved requirements are complete for this period."
            : "Review the details that need attention before you go."}
        </p>
      </div>
      {error && (
        <p role="alert" className="error-banner">
          {error}
        </p>
      )}
      {data && (
        <>
          <div className="readiness-next">
            {[
              ["Next confirmed stay", data.nextStay],
              ["Next transportation", data.nextTransport],
            ].map(([label, r]: any) => (
              <article key={label}>
                <span className="eyebrow">{label}</span>
                <h3>{r?.name || "Not recorded"}</h3>
                <small>{r?.checkIn || r?.departure || "Unknown"}</small>
                {r && (
                  <button
                    className="text-button"
                    onClick={() =>
                      onNavigate({ module: "Calendar", recordId: r.id })
                    }
                  >
                    View booking
                    <ArrowUpRight size={14} />
                  </button>
                )}
              </article>
            ))}
          </div>
          <div className="inline-actions">
            <button className="secondary" onClick={() => onCreate("stay")}>
              Add missing stay
            </button>
            <button
              className="secondary"
              onClick={() => onNavigate({ action: "imports" })}
            >
              Review imports
            </button>
            <button
              className="secondary"
              onClick={() => onNavigate({ module: "Budget" })}
            >
              Open Budget
            </button>
            <button className="secondary" onClick={() => onCreate("reminder")}>
              Add reminder
            </button>
          </div>
          <div className="readiness-groups">
            {[
              ["attention", "Needs attention", AlertCircle],
              ["upcoming", "Upcoming", Clock],
              ["complete", "Complete", CheckCircle],
              ["unknown", "Unknown / unverified", HelpCircle],
            ].map(([key, label, Icon]: any) => (
              <section key={key}>
                <h2>
                  <Icon size={18} />
                  {label}
                  <span className="count-badge">
                    {data.items.filter((i: any) => i.group === key).length}
                  </span>
                </h2>
                {data.items
                  .filter((i: any) => i.group === key)
                  .map((item: any, i: number) => (
                    <button
                      className="readiness-item"
                      key={i}
                      onClick={() => onNavigate(item.target)}
                    >
                      <strong>{item.title}</strong>
                      <small>{item.detail}</small>
                      <ArrowUpRight size={14} />
                    </button>
                  ))}
                {!data.items.some((i: any) => i.group === key) && (
                  <p className="muted">No items in this group.</p>
                )}
              </section>
            ))}
          </div>
          <section className="reminder-suggestions">
            <h2>Suggested reminders</h2>
            <p className="field-help">
              Timezone-aware reminders use saved booking times. Add each
              suggestion explicitly, or enable individual rules in Settings →
              Notifications. Delivery requires a running host.
            </p>
            {data.suggestions.map((s: any) => (
              <div className="suggestion-row" key={s.suggestionKey}>
                <span>
                  <strong>{s.name}</strong>
                  <small>
                    {new Date(s.dueAt).toLocaleString()} · event {s.eventAt}
                  </small>
                </span>
                <button
                  className="secondary"
                  disabled={busy || s.adopted}
                  onClick={() => adopt(s)}
                >
                  {s.adopted ? `Saved · ${s.existingState}` : "Add reminder"}
                </button>
              </div>
            ))}
          </section>
          <p className="field-help">{data.basis}</p>
        </>
      )}
    </section>
  );
}
