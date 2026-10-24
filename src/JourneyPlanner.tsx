import { useState } from "react";
import {
  AtlasRecord,
  rpc,
  money,
  currencyDigits,
  minorFromInput,
  openExternal,
  download,
} from "./api";
const today = () => new Date().toISOString().slice(0, 10);
function newStop() {
  return {
    name: "",
    arrival: today(),
    departure: today(),
    timezone: "Europe/Rome",
    context: "on_foot",
    currency: "USD",
    stayId: "",
    transportId: "",
    notes: "",
    totalMinor: null,
    dailyBudgetMinor: null,
  };
}
export function JourneyPlanner({
  records,
  onRefresh,
  onSelect,
}: {
  records: AtlasRecord[];
  onRefresh: () => Promise<void>;
  onSelect: (r: AtlasRecord) => void;
}) {
  const [draft, setDraft] = useState<any>({
    kind: "trip",
    name: "My tentative journey",
    stops: [newStop()],
    status: "draft",
  });
  const [result, setResult] = useState<any>();
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [proposal, setProposal] = useState<any>();
  const [days, setDays] = useState(14);
  const [selectedDocs, setSelectedDocs] = useState<string[]>([]);
  const [route, setRoute] = useState<any>({
    date: today(),
    mode: "transit",
    fromId: "",
    toId: "",
    notes: "",
  });
  const journeys = records.filter(
    (r) => r.kind === "trip" && Array.isArray(r.stops),
  );
  const places = records.filter(
    (r) =>
      ["place", "stay", "objective"].includes(r.kind) &&
      (r.address || Number.isFinite(r.latitude)),
  );
  async function run(task: () => Promise<void>) {
    setBusy(true);
    setMessage("");
    try {
      await task();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  function stop(index: number, patch: any) {
    setDraft({
      ...draft,
      stops: draft.stops.map((s: any, i: number) =>
        i === index ? { ...s, ...patch } : s,
      ),
    });
    setResult(null);
  }
  function reorder(index: number, delta: number) {
    const stops = [...draft.stops];
    [stops[index], stops[index + delta]] = [stops[index + delta], stops[index]];
    setDraft({ ...draft, stops });
    setResult(null);
  }
  return (
    <details className="planning-summary" data-testid="journey-planner">
      <summary>Journey planning & offline trip pack</summary>
      <div className="panel-stack journey-workspace">
        <label className="field">
          Saved journey
          <select
            value={draft.id || ""}
            onChange={(e) => {
              const saved = journeys.find((r) => r.id === e.target.value);
              setDraft(
                saved
                  ? structuredClone(saved)
                  : {
                      kind: "trip",
                      name: "My tentative journey",
                      stops: [newStop()],
                    },
              );
              setResult(null);
            }}
          >
            <option value="">New tentative journey</option>
            {journeys.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          Journey name
          <input
            value={draft.name}
            onChange={(e) => setDraft({ ...draft, name: e.target.value })}
          />
        </label>
        <p>
          Draft stops can mix On Foot and On Road. Confirmed bookings remain in
          their saved dates.
        </p>
        {draft.stops.map((s: any, index: number) => (
          <section className="import-card" key={index}>
            <h3>Stop {index + 1}</h3>
            <div className="form-grid">
              {[
                ["name", "Destination", "text"],
                ["arrival", "Arrival", "date"],
                ["departure", "Departure", "date"],
                ["timezone", "IANA timezone", "text"],
                ["currency", "Currency", "text"],
                ["notes", "Stop notes", "text"],
              ].map(([key, label, type]) => (
                <label className="field" key={key}>
                  {label}
                  <input
                    type={type}
                    value={s[key] || ""}
                    onChange={(e) => stop(index, { [key]: e.target.value })}
                  />
                </label>
              ))}
              <label className="field">
                Travel context
                <select
                  value={s.context}
                  onChange={(e) => stop(index, { context: e.target.value })}
                >
                  <option value="on_foot">On Foot</option>
                  <option value="on_road">On Road</option>
                </select>
              </label>
              {[
                ["stayId", "Planned stay", "stay"],
                ["transportId", "Incoming transportation", "transport"],
              ].map(([key, label, kind]) => (
                <label className="field" key={key}>
                  {label}
                  <select
                    value={s[key] || ""}
                    onChange={(e) => stop(index, { [key]: e.target.value })}
                  >
                    <option value="">Not recorded</option>
                    {records
                      .filter((r) => r.kind === kind)
                      .map((r) => (
                        <option key={r.id} value={r.id}>
                          {r.name}
                        </option>
                      ))}
                  </select>
                </label>
              ))}
              {[
                ["totalMinor", "Accommodation total"],
                ["dailyBudgetMinor", "Daily spending estimate"],
              ].map(([key, label]) => (
                <label className="field" key={key}>
                  {label} · {s.currency}
                  <input
                    key={`${index}-${key}-${s.currency}-${s[key]}`}
                    inputMode="decimal"
                    defaultValue={
                      s[key] == null
                        ? ""
                        : String(s[key] / 10 ** currencyDigits(s.currency))
                    }
                    placeholder="Unknown"
                    onBlur={(e) => {
                      try {
                        stop(index, {
                          [key]: e.target.value
                            ? minorFromInput(e.target.value, s.currency)
                            : null,
                        });
                      } catch (err) {
                        setMessage(String(err));
                      }
                    }}
                  />
                </label>
              ))}
            </div>
            <div className="button-row">
              <button
                className="secondary"
                disabled={index === 0 || busy}
                onClick={() => reorder(index, -1)}
              >
                Move earlier
              </button>
              <button
                className="secondary"
                disabled={index === draft.stops.length - 1 || busy}
                onClick={() => reorder(index, 1)}
              >
                Move later
              </button>
              <button
                className="secondary"
                disabled={draft.stops.length === 1 || busy}
                onClick={() =>
                  setDraft({
                    ...draft,
                    stops: draft.stops.filter(
                      (_: any, i: number) => i !== index,
                    ),
                  })
                }
              >
                Remove stop
              </button>
              {s.stayId && (
                <button
                  className="secondary"
                  onClick={() => {
                    const r = records.find((r) => r.id === s.stayId);
                    if (r) onSelect(r);
                  }}
                >
                  Open planned stay
                </button>
              )}
            </div>
          </section>
        ))}
        <div className="button-row">
          <button
            className="secondary"
            disabled={busy || draft.stops.length >= 40}
            onClick={() =>
              setDraft({ ...draft, stops: [...draft.stops, newStop()] })
            }
          >
            Add destination
          </button>
          <button
            className="secondary"
            disabled={busy}
            onClick={() =>
              run(async () =>
                setResult(
                  await rpc("journeys.preview", { stops: draft.stops }),
                ),
              )
            }
          >
            Compare draft with saved plan
          </button>
          <button
            className="secondary"
            disabled={busy}
            onClick={() =>
              run(async () => {
                setDraft(
                  await rpc("journeys.save", {
                    record: draft,
                    expectedVersion: draft.version,
                  }),
                );
                await onRefresh();
                setMessage(
                  "Tentative journey saved. Bookings were not changed.",
                );
              })
            }
          >
            Save tentative journey
          </button>
        </div>
        {result && (
          <section className="info-note">
            <h3>Hypothetical journey · no commitments changed</h3>
            {result.issues.map((issue: any, i: number) => (
              <p key={i}>{issue.message || issue.type}</p>
            ))}
            {result.monthlyScenarioCosts.map((b: any, i: number) => (
              <p key={i}>
                {b.month} · {b.currency}: accommodation{" "}
                {money(b.accommodationMinor, b.currency)} · planned spending{" "}
                {money(b.expensesMinor, b.currency)}
              </p>
            ))}
            <p>
              Unknown costs, transport and availability remain unverified.
              Comparison uses actual month-of-use calculations.
            </p>
          </section>
        )}
        <details>
          <summary>Propose a day-by-day schedule</summary>
          <p>
            Protect saved work and proposed work requirements. Review each
            objective before saving it.
          </p>
          <label className="field">
            Days
            <input
              type="number"
              min={1}
              max={31}
              value={days}
              onChange={(e) => setDays(Number(e.target.value))}
            />
          </label>
          <button
            className="secondary"
            disabled={busy}
            onClick={() =>
              run(async () =>
                setProposal(
                  await rpc("journeys.propose", {
                    start: draft.stops[0].arrival,
                    timezone: draft.stops[0].timezone,
                    days,
                  }),
                ),
              )
            }
          >
            Generate schedule proposal
          </button>
          {records
            .filter((r) => r.kind === "proposal")
            .map((r) => (
              <button
                className="secondary"
                key={r.id}
                onClick={() => setProposal(r)}
              >
                Reopen {r.name}
              </button>
            ))}
          {proposal?.items?.length === 0 && (
            <p>No suitable free periods were found.</p>
          )}
          {proposal?.items?.map((item: any) => (
            <section className="panel-card" key={item.id}>
              <strong>
                {item.record.date} · {item.record.name}
              </strong>
              <p>
                Destination: {item.localStart} → {item.localEnd}
              </p>
              <p>
                Employer: {item.officeStart} → {item.officeEnd}
              </p>
              <p>{item.state}</p>
              {item.state === "pending" && (
                <>
                  <label className="field">
                    Objective name
                    <input
                      value={item.record.name}
                      onChange={(e) =>
                        setProposal({
                          ...proposal,
                          items: proposal.items.map((i: any) =>
                            i.id === item.id
                              ? {
                                  ...i,
                                  record: { ...i.record, name: e.target.value },
                                }
                              : i,
                          ),
                        })
                      }
                    />
                  </label>
                  <div className="button-row">
                    {[
                      ["journeys.accept", "Accept objective"],
                      ["journeys.reject", "Reject"],
                    ].map(([method, label]) => (
                      <button
                        className="secondary"
                        disabled={busy}
                        key={method}
                        onClick={() =>
                          run(async () => {
                            setProposal(
                              await rpc(method, {
                                id: proposal.id,
                                itemId: item.id,
                                expectedVersion: proposal.version,
                                name: item.record.name,
                              }),
                            );
                            await onRefresh();
                          })
                        }
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                </>
              )}
            </section>
          ))}
        </details>
        <details>
          <summary>Route draft between saved places</summary>
          <div className="form-grid">
            {[
              ["fromId", "Departure"],
              ["toId", "Destination"],
            ].map(([key, label]) => (
              <label className="field" key={key}>
                {label}
                <select
                  value={route[key]}
                  onChange={(e) =>
                    setRoute({ ...route, [key]: e.target.value })
                  }
                >
                  <option value="">Choose saved place</option>
                  {places.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </label>
            ))}
            <label className="field">
              Travel date
              <input
                type="date"
                value={route.date}
                onChange={(e) => setRoute({ ...route, date: e.target.value })}
              />
            </label>
            <label className="field">
              Mode
              <select
                value={route.mode}
                onChange={(e) => setRoute({ ...route, mode: e.target.value })}
              >
                {["driving", "walking", "transit", "flight"].map((mode) => (
                  <option key={mode}>{mode}</option>
                ))}
              </select>
            </label>
            <label className="field">
              Route notes / evidence
              <input
                value={route.notes}
                onChange={(e) => setRoute({ ...route, notes: e.target.value })}
              />
            </label>
          </div>
          <button
            className="secondary"
            disabled={busy}
            onClick={() =>
              run(async () => {
                await rpc("routes.draft", route);
                await onRefresh();
                setMessage("Unverified route draft saved");
              })
            }
          >
            Save route draft
          </button>
          {records
            .filter((r) => r.kind === "routeDraft")
            .map((r) => (
              <section className="panel-card" key={r.id}>
                <strong>{r.name}</strong>
                <p>
                  {r.date} · {r.mode} · Route unverified
                </p>
                <p>{r.notes}</p>
                <div className="button-row">
                  <button
                    className="secondary"
                    onClick={() => {
                      const p = records.find((p) => p.id === r.fromId);
                      if (p) onSelect(p);
                    }}
                  >
                    Open departure place
                  </button>
                  <button
                    className="secondary"
                    onClick={() => {
                      const p = records.find((p) => p.id === r.toId);
                      if (p) onSelect(p);
                    }}
                  >
                    Open destination place
                  </button>
                  <button
                    className="secondary"
                    onClick={() => openExternal(r.directionsUrl)}
                  >
                    Open provider directions
                  </button>
                </div>
              </section>
            ))}
        </details>
        <details>
          <summary>Export selected trip for offline use</summary>
          <p>
            Save the selected trip as a local JSON package with addresses,
            contacts, reminders, work requirements and budgets. Select document
            contents explicitly. For a paired phone, use its existing Documents
            selection to keep chosen files.
          </p>
          {records
            .filter((r) => r.kind === "document")
            .map((r) => (
              <label className="check-field" key={r.id}>
                <input
                  type="checkbox"
                  checked={selectedDocs.includes(r.id)}
                  onChange={(e) =>
                    setSelectedDocs(
                      e.target.checked
                        ? [...selectedDocs, r.id]
                        : selectedDocs.filter((id) => id !== r.id),
                    )
                  }
                />
                {r.filename || r.name}
              </label>
            ))}
          <button
            className="secondary"
            disabled={busy || !draft.id}
            onClick={() =>
              run(async () => {
                const pack = await rpc("journeys.offline", {
                  id: draft.id,
                  documentIds: selectedDocs,
                  currency: draft.stops[0].currency,
                });
                download(
                  "Atlas-selected-trip.json",
                  JSON.stringify(pack, null, 2),
                  "application/json",
                );
              })
            }
          >
            Download selected trip package
          </button>
        </details>
        {message && (
          <p role="status" className="info-note">
            {message}
          </p>
        )}
      </div>
    </details>
  );
}
