import { useState } from "react";
import { X } from "lucide-react";
import { rpc } from "./api";
import { AtlasTarget } from "./navigation";

export function NotificationsPanel({
  data,
  onRefresh,
  onNavigate,
  onCreate,
}: {
  data: any;
  onRefresh: () => Promise<void>;
  onNavigate: (target: AtlasTarget) => boolean;
  onCreate: () => void;
}) {
  const [error, setError] = useState("");
  const [older, setOlder] = useState<any[]>([]);
  async function run(fn: () => Promise<void>) {
    try {
      setError("");
      await fn();
      await onRefresh();
    } catch (e) {
      setError(String(e));
    }
  }
  return (
    <div className="panel-stack">
      <div className="panel-toolbar">
        <button
          className="text-button"
          onClick={() =>
            run(async () => {
              await rpc("notifications.read_all");
              setOlder(
                older.map((r) => ({ ...r, readAt: new Date().toISOString() })),
              );
            })
          }
        >
          Mark All as Read
        </button>
        <button className="text-button" onClick={onCreate}>
          New reminder
        </button>
      </div>
      {error && (
        <p className="error-banner" role="alert">
          {error}
        </p>
      )}
      {data?.items?.length === 0 && !older.length && (
        <p className="muted">
          No notifications. Saved reminders and upcoming travel events appear
          here when relevant.
        </p>
      )}
      {[...(data?.items || []), ...older].map((n: any) => (
        <article
          className={`panel-card ${n.readAt ? "" : "unread"}`}
          key={n.id}
        >
          <button
            onClick={() =>
              run(async () => {
                if (onNavigate(n.target))
                  await rpc("notifications.read", { id: n.id });
              })
            }
          >
            <strong>{n.title}</strong>
            <p>{n.description}</p>
            <small>
              {new Date(n.createdAt).toLocaleString()} · {n.priority}
              {!n.readAt && " · Unread"}
            </small>
          </button>
          <button
            className="text-button"
            aria-label={`Dismiss ${n.title}`}
            onClick={() =>
              run(async () => {
                await rpc("notifications.dismiss", { id: n.id });
                setOlder(older.filter((r) => r.id !== n.id));
              })
            }
          >
            <X size={13} />
            Dismiss
          </button>
        </article>
      ))}
      {data?.items?.length >= 100 && (
        <button
          className="text-button"
          onClick={() =>
            run(async () => {
              const next = await rpc("notifications.list", {
                offset: data.items.length + older.length,
              });
              setOlder([...older, ...next.items]);
            })
          }
        >
          Load older notifications
        </button>
      )}
    </div>
  );
}
