import { useEffect, useRef, useState } from "react";
import { Plus, Send, Trash2 } from "lucide-react";
import { AtlasRecord, money, rpc } from "./api";
import { AtlasTarget, Module } from "./navigation";

function suggestions(module: Module, records: AtlasRecord[], context: any) {
  const hasStays = records.some((r) => r.kind === "stay");
  const byModule: Record<Module, string[]> = {
    Calendar: [
      "Show my accommodation gaps.",
      "Find room in my schedule for groceries before work.",
      ...(hasStays
        ? ["Show my upcoming checkout deadlines."]
        : ["What is on my calendar?"]),
    ],
    Discover: [
      "Compare my saved apartments.",
      "Which saved accommodations have a refrigerator and suitable workspace?",
      "Which saved destinations match my preferences?",
    ],
    Map: [
      "Show my saved places and objectives.",
      "What transportation is saved on my calendar?",
    ],
    Budget: [
      "What is my estimated daily cost?",
      "How much flexible allowance remains?",
      ...(hasStays ? ["What happens if I extend this stay by 7 days?"] : []),
    ],
    Settings: [
      "Explain my work schedule settings.",
      "How do I configure automatic booking imports?",
      "How does Atlas store my data?",
    ],
  };
  return byModule[module];
}
function recency(value: string) {
  const days = Math.max(
    0,
    Math.floor((Date.now() - Date.parse(value)) / 86400000),
  );
  return days === 0 ? "Today" : days === 1 ? "Yesterday" : `${days} days ago`;
}
export function AssistantPanel({
  module,
  context,
  records,
  onNavigate,
  onRefresh,
}: {
  module: Module;
  context: any;
  records: AtlasRecord[];
  onNavigate: (target: AtlasTarget) => boolean;
  onRefresh: () => Promise<void>;
}) {
  const [sessions, setSessions] = useState<any[]>([]);
  const [session, setSession] = useState<any>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [ai, setAi] = useState<any>(null);
  const [allHistory, setAllHistory] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const inFlight = useRef(false);
  const pending = useRef<{ text: string; id: string } | null>(null);
  const messages = useRef<HTMLDivElement>(null);
  const refreshSessions = async () => {
    const list = await rpc<any[]>("chat.list");
    setSessions(list);
    return list;
  };
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const list = await rpc<any[]>("chat.list");
        if (!alive) return;
        setSessions(list);
        if (list[0]) {
          const chat = await rpc("chat.get", { id: list[0].id });
          if (alive) setSession(chat);
        }
        const state = await rpc("ai.status");
        if (alive) setAi(state);
      } catch (e) {
        if (alive) setError(String(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, []);
  useEffect(() => {
    messages.current?.scrollTo(0, messages.current.scrollHeight);
  }, [session?.id, session?.messages?.length]);
  async function ask(text: string) {
    if (!text.trim() || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError("");
    try {
      let chat = session;
      if (!chat) {
        chat = await rpc("chat.create");
        setSession(chat);
      }
      if (pending.current?.text !== text)
        pending.current = { text, id: crypto.randomUUID() };
      const next = await rpc("chat.send", {
        id: chat.id,
        text,
        requestId: pending.current.id,
        context: { ...context, module },
      });
      setSession(next);
      setInput("");
      pending.current = null;
      await refreshSessions();
      await onRefresh();
    } catch (e) {
      setError(String(e));
      setInput(text);
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  async function action(fn: () => Promise<void>) {
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    }
  }
  return (
    <div className="chat-layout">
      <div className="panel-toolbar">
        <small className="muted">
          Local deterministic planner ·{" "}
          {ai?.available
            ? "optional extraction runtime available"
            : "AI inference unavailable"}
        </small>
        <button
          className="text-button"
          disabled={busy}
          onClick={() =>
            action(async () => {
              setSession(await rpc("chat.create"));
              setInput("");
              setDeleting(false);
              pending.current = null;
              await refreshSessions();
            })
          }
        >
          <Plus size={14} />
          New conversation
        </button>
      </div>
      <div className="chat-suggestions" aria-label={`${module} suggestions`}>
        {suggestions(module, records, context).map((q) => (
          <button disabled={busy} key={q} onClick={() => ask(q)}>
            {q}
          </button>
        ))}
      </div>
      {error && (
        <p role="alert" className="error-banner">
          {error}
        </p>
      )}
      {session && (
        <div className="panel-toolbar">
          <input
            aria-label="Conversation title"
            value={session.title}
            maxLength={120}
            onChange={(e) => setSession({ ...session, title: e.target.value })}
            onBlur={() =>
              action(async () => {
                await rpc("chat.rename", {
                  id: session.id,
                  title: session.title,
                });
                await refreshSessions();
              })
            }
          />
          <button
            className="icon-button"
            title="Delete conversation"
            aria-label="Delete conversation"
            disabled={busy}
            onClick={() => setDeleting(true)}
          >
            <Trash2 size={15} />
          </button>
        </div>
      )}
      {deleting && (
        <div className="info-note">
          Delete this local conversation?
          <div className="inline-actions">
            <button
              className="secondary"
              onClick={() =>
                action(async () => {
                  await rpc("chat.delete", { id: session.id });
                  setSession(null);
                  setDeleting(false);
                  await refreshSessions();
                })
              }
            >
              Delete chat
            </button>
            <button className="text-button" onClick={() => setDeleting(false)}>
              Keep chat
            </button>
          </div>
        </div>
      )}
      <div
        className="chat-messages"
        ref={messages}
        aria-label="Conversation messages"
        aria-live="polite"
      >
        {session?.messages?.length >= 100 && (
          <button
            className="text-button"
            onClick={() =>
              action(async () => {
                const older = await rpc("chat.get", {
                  id: session.id,
                  beforeSequence: session.messages[0].sequence,
                });
                setSession({
                  ...session,
                  messages: [...older.messages, ...session.messages],
                });
              })
            }
          >
            Load earlier messages
          </button>
        )}
        {session?.messages?.map((m: any) => (
          <article key={m.id} className={m.role}>
            <small>{m.role === "user" ? "YOU" : "ATLAS"}</small>
            <p>{m.text}</p>
            {m.result?.records?.map((r: AtlasRecord) => (
              <button
                key={r.id}
                className="text-button"
                onClick={() =>
                  onNavigate({
                    module: "Calendar",
                    recordId: r.id,
                    date: r.checkIn || r.date,
                  })
                }
              >
                {r.name}
              </button>
            ))}
            {m.result?.targets?.map((t: any, i: number) => (
              <button
                className="text-button"
                key={i}
                onClick={() => onNavigate(t.target)}
              >
                {t.label}
              </button>
            ))}
            {m.result?.work && (
              <div className="info-note">
                <span>
                  Office: {m.result.work.officeTimezone} · Destination:{" "}
                  {m.result.work.destinationTimezone}
                </span>
                {m.result.work.officeWindows?.map((w: any, i: number) => (
                  <p key={i}>
                    Employer: {w.officeStart} → {w.officeEnd}
                    <br />
                    Local: {w.localStart} → {w.localEnd}
                  </p>
                ))}
                {m.result.work.conflicts?.map((c: any, i: number) => (
                  <p key={i}>{c.message}</p>
                ))}
              </div>
            )}
            {m.result?.workspaces?.map((w: any) => (
              <details key={w.record.id}>
                <summary>
                  {w.record.name}: {w.suitability.status}
                </summary>
                <p>Unknown: {w.suitability.unknown.join(", ") || "None"}</p>
                <p>Conflicts: {w.suitability.conflicts.join(", ") || "None"}</p>
              </details>
            ))}
            {m.result?.undo && (
              <button
                className="secondary"
                disabled={busy}
                onClick={() =>
                  action(async () => {
                    setSession(
                      await rpc("chat.undo", {
                        id: session.id,
                        sequence: m.sequence,
                      }),
                    );
                    await onRefresh();
                  })
                }
              >
                Undo reminder
              </button>
            )}
          </article>
        ))}
        {!session?.messages?.length && (
          <p className="muted">
            Ask about saved plans or choose a suggestion. Answers use local
            services; prices and availability are never checked implicitly.
          </p>
        )}
        {busy && <p role="status">Reading your saved plans…</p>}
      </div>
      <form
        className="chat-composer"
        onSubmit={(e) => {
          e.preventDefault();
          void ask(input);
        }}
      >
        <textarea
          aria-label="Ask Atlas"
          value={input}
          maxLength={8000}
          placeholder="Ask about your plans…"
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (
              e.key === "Enter" &&
              !e.shiftKey &&
              !e.nativeEvent.isComposing
            ) {
              e.preventDefault();
              void ask(input);
            }
          }}
        />
        <button
          className="icon-button"
          aria-label="Send question"
          disabled={busy || !input.trim()}
        >
          <Send size={17} />
        </button>
      </form>
      <details className="recent-chats">
        <summary>Recent Chats ({sessions.length})</summary>
        <div>
          {(allHistory ? sessions : sessions.slice(0, 5)).map((s) => (
            <button
              disabled={busy}
              key={s.id}
              onClick={() =>
                action(async () => {
                  setSession(await rpc("chat.get", { id: s.id }));
                  setDeleting(false);
                })
              }
            >
              <span>{s.title}</span>
              <small>{recency(s.updatedAt)}</small>
            </button>
          ))}
        </div>
        <button
          className="text-button"
          onClick={() => setAllHistory(!allHistory)}
        >
          {allHistory ? "Show recent only" : "View full history"}
        </button>
        {allHistory && sessions.length >= 50 && (
          <button
            className="text-button"
            onClick={() =>
              action(async () =>
                setSessions([
                  ...sessions,
                  ...(await rpc<any[]>("chat.list", {
                    offset: sessions.length,
                  })),
                ]),
              )
            }
          >
            Load older chats
          </button>
        )}
      </details>
    </div>
  );
}
