import { useEffect, useState } from "react";
import { AtlasRecord, Settings, rpc, money, openExternal } from "./api";
import { MoneyField } from "./RecordForm";
export function OfferComparison({
  ids,
  records,
  settings,
  onSelect,
}: {
  ids: string[];
  records: AtlasRecord[];
  settings: Settings;
  onSelect: (r: AtlasRecord) => void;
}) {
  const [data, setData] = useState<any>(null),
    [scenario, setScenario] = useState<any>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const [checkIn, setIn] = useState(""),
    [checkOut, setOut] = useState(""),
    [preferences, setPreferences] = useState({ ...settings.preferences });
  const [replace, setReplace] = useState("");
  async function compare() {
    setBusy(true);
    setError("");
    try {
      setData(
        await rpc("discovery.compare", {
          quoteIds: ids,
          preferences,
          ...(checkIn && checkOut ? { checkIn, checkOut } : {}),
          maxAgeDays: 14,
        }),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    setData(null);
    setScenario(null);
  }, [JSON.stringify(ids)]);
  return (
    <section className="offer-workbench">
      <h2>Compare the whole journey.</h2>
      <p>
        {ids.length} saved offers selected. Temporary preferences below apply
        only to this comparison.
      </p>
      <details className="advanced-settings">
        <summary>Temporary search preferences</summary>
        <div className="form-grid">
          <label className="field">
            <span>Intended check-in</span>
            <input
              type="date"
              value={checkIn}
              onChange={(e) => setIn(e.target.value)}
            />
          </label>
          <label className="field">
            <span>Intended checkout</span>
            <input
              type="date"
              value={checkOut}
              onChange={(e) => setOut(e.target.value)}
            />
          </label>
          <MoneyField
            label="Monthly target"
            value={preferences.monthlyTargetMinor}
            currency={preferences.currency || settings.currency}
            onChange={(v) =>
              setPreferences({ ...preferences, monthlyTargetMinor: v })
            }
          />
          <MoneyField
            label="Stretch cap"
            value={preferences.stretchCapMinor}
            currency={preferences.currency || settings.currency}
            onChange={(v) =>
              setPreferences({ ...preferences, stretchCapMinor: v })
            }
          />
        </div>
        <div className="inline-actions">
          {[
            ["refrigeratorRequired", "Refrigerator"],
            ["workspaceRequired", "Workspace"],
            ["internetRequired", "Internet"],
            ["kitchenRequired", "Kitchen"],
          ].map(([key, label]) => (
            <label className="check-field" key={key}>
              <input
                type="checkbox"
                checked={!!preferences[key]}
                onChange={(e) =>
                  setPreferences({ ...preferences, [key]: e.target.checked })
                }
              />
              {label} required
            </label>
          ))}
        </div>
      </details>
      <button
        className="primary"
        disabled={busy || ids.length < 2 || !!checkIn !== !!checkOut}
        onClick={compare}
      >
        {busy ? "Comparing…" : "Compare selected offers"}
      </button>
      {error && (
        <p className="error-banner" role="alert">
          {error}
        </p>
      )}
      {data && (
        <>
          <p className="field-help">{data.rankingBasis}</p>
          {data.groups.map((g: any) => (
            <section className="match-group" key={`${g.currency}-${g.checkIn}`}>
              <h3>
                {g.checkIn} → {g.checkOut} · {g.currency}
              </h3>
              <div className="comparison-cards">
                {g.offers.map((o: any) => (
                  <article className="import-card" key={o.id}>
                    <span className="eyebrow">
                      {o.rankWithinGroup}.{" "}
                      {o.recommendedForReview
                        ? "Meets saved criteria"
                        : "Needs verification"}
                    </span>
                    <button
                      className="offer-name"
                      onClick={() => onSelect(o.record)}
                    >
                      {o.name}
                    </button>
                    <h3>{money(o.knownPayableMinor, o.currency)}</h3>
                    <p>
                      {money(o.nightlyMinor, o.currency)} / night · {o.nights}{" "}
                      nights
                    </p>
                    <p>
                      {o.feesComplete
                        ? "Mandatory fees included"
                        : "Fees incomplete"}{" "}
                      · {o.freshness.replaceAll("_", " ")}
                    </p>
                    <p>Availability: {o.record.availability || "unknown"}</p>
                    <p>
                      {o.record.availabilityEvidence ||
                        "No availability evidence recorded."}
                    </p>
                    <p>
                      Cancellation: {o.record.cancellationPolicy || "unknown"}
                    </p>
                    <ul>
                      {o.reasons.map((r: string, i: number) => (
                        <li key={i}>{r}</li>
                      ))}
                    </ul>
                    <p className="field-help">
                      Source:{" "}
                      {o.source?.provider ||
                        o.record.provider ||
                        "owner snapshot"}{" "}
                      · {o.source?.checkedAt || "Check date unknown"}
                    </p>
                    <button
                      className="secondary"
                      onClick={async () => {
                        try {
                          setScenario(
                            await rpc("discovery.scenario", {
                              quoteId: o.id,
                              ...(replace ? { replaceBookingId: replace } : {}),
                            }),
                          );
                          setError("");
                        } catch (e) {
                          setError(String(e));
                        }
                      }}
                    >
                      Preview budget with this offer
                    </button>
                  </article>
                ))}
              </div>
            </section>
          ))}
          {data.transport?.length > 0 && (
            <section>
              <h3>Saved transport offers</h3>
              <p className="field-help">
                Compare routes and times explicitly. Prices in different
                currencies are never combined; incomplete totals cannot
                establish the cheapest complete journey.
              </p>
              <div className="comparison-cards">
                {data.transport.map((o: any) => (
                  <article className="import-card" key={o.record.id}>
                    <button
                      className="offer-name"
                      onClick={() => onSelect(o.record)}
                    >
                      {o.record.name}
                    </button>
                    <p>
                      {o.record.origin} → {o.record.destination}
                    </p>
                    <p>
                      {o.record.departure || "Departure unknown"} →{" "}
                      {o.record.arrival || "Arrival unknown"}
                    </p>
                    <strong>
                      {money(o.knownJourneyMinor, o.record.currency)} known
                      journey cost
                    </strong>
                    <p>
                      Unknown:{" "}
                      {o.unknownComponents.join(", ") ||
                        "No additional cost components marked unknown"}
                    </p>
                    <p>
                      {o.record.baggage || "Baggage allowance unknown"} ·{" "}
                      {o.record.connections || "Transfers unverified"}
                    </p>
                    <p>
                      {o.record.availability || "Availability unknown"} ·{" "}
                      {o.lastCheckedAt || "Check date unknown"}
                    </p>
                    <details>
                      <summary>
                        Duration, check-in and work compatibility
                      </summary>
                      <pre className="evidence">
                        {JSON.stringify(o.analysis, null, 2)}
                      </pre>
                    </details>
                  </article>
                ))}
              </div>
            </section>
          )}
          {data.skipped?.length > 0 && (
            <details>
              <summary>Offers outside this stay comparison</summary>
              {data.skipped.map((s: any) => (
                <p key={s.id}>
                  {s.name}: {s.reason}
                </p>
              ))}
            </details>
          )}
          <label className="field">
            <span>Hypothetical replacement (optional)</span>
            <select
              value={replace}
              onChange={(e) => setReplace(e.target.value)}
            >
              <option value="">
                Add the offer to the hypothetical itinerary
              </option>
              {records
                .filter((r) => r.kind === "stay" && r.status !== "cancelled")
                .map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.name}
                  </option>
                ))}
            </select>
          </label>
          {scenario && (
            <section className="scenario-result">
              <h3>Budget scenario · no records changed</h3>
              <p>
                Period forecast:{" "}
                {money(scenario.before.forecastMinor, scenario.after.currency)}{" "}
                → {money(scenario.after.forecastMinor, scenario.after.currency)}
              </p>
              <p>
                Daily estimate:{" "}
                {money(
                  scenario.after.dailyForecastMinor,
                  scenario.after.currency,
                )}{" "}
                · difference{" "}
                {money(scenario.forecastDeltaMinor, scenario.after.currency)}
              </p>
              {scenario.after.months.map((m: any) => (
                <p key={m.month}>
                  {m.month}: {money(m.forecastMinor, m.currency)} forecast ·
                  flexible funds {money(m.allowanceRemainingMinor, m.currency)}
                </p>
              ))}
              <p className="field-help">{scenario.basis}</p>
            </section>
          )}
          <p className="field-help">{data.coverage}</p>
        </>
      )}
    </section>
  );
}
export function PreparedSearch({ settings }: { settings: Settings }) {
  const [destination, setDestination] = useState(""),
    [date, setDate] = useState(""),
    [end, setEnd] = useState(""),
    [origin, setOrigin] = useState(""),
    [basis, setBasis] = useState(""),
    [error, setError] = useState("");
  async function preceding() {
    try {
      const r = await rpc("planning.origin", { date });
      setOrigin(r.location?.address || r.location?.name || "");
      setBasis(r.basis || r.source || "Preceding saved segment");
    } catch (e) {
      setError(String(e));
    }
  }
  async function open(provider: string) {
    try {
      let url: URL;
      if (provider === "Booking.com") {
        url = new URL("https://www.booking.com/searchresults.html");
        url.searchParams.set("ss", destination);
        if (date) url.searchParams.set("checkin", date);
        if (end) url.searchParams.set("checkout", end);
      } else if (provider === "Airbnb") {
        url = new URL(
          `https://www.airbnb.com/s/${encodeURIComponent(destination)}/homes`,
        );
        if (date) url.searchParams.set("checkin", date);
        if (end) url.searchParams.set("checkout", end);
      } else {
        url = new URL(
          `https://www.rome2rio.com/s/${encodeURIComponent(origin)}/${encodeURIComponent(destination)}`,
        );
      }
      await openExternal(url.href);
    } catch (e) {
      setError(String(e));
    }
  }
  return (
    <details className="prepared-search">
      <summary>Prepare a stay or route search</summary>
      <p className="field-help">
        Open provider searches with your destination and dates. Check prices,
        fees and availability at the source, then save an offer in Atlas.
      </p>
      <div className="form-grid">
        <label className="field">
          <span>Destination</span>
          <input
            value={destination}
            onChange={(e) => setDestination(e.target.value)}
          />
        </label>
        <label className="field">
          <span>Start / departure date</span>
          <input
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </label>
        <label className="field">
          <span>Checkout</span>
          <input
            type="date"
            value={end}
            onChange={(e) => setEnd(e.target.value)}
          />
        </label>
        <label className="field">
          <span>Route origin</span>
          <input value={origin} onChange={(e) => setOrigin(e.target.value)} />
        </label>
      </div>
      <button className="text-button" disabled={!date} onClick={preceding}>
        Use preceding planned destination
      </button>
      <small>{typeof basis === "string" ? basis : JSON.stringify(basis)}</small>
      <div className="inline-actions">
        {["Booking.com", "Airbnb", "Rome2Rio"].map((p) => (
          <button
            className="secondary"
            disabled={!destination || (p === "Rome2Rio" && !origin)}
            key={p}
            onClick={() => open(p)}
          >
            Search {p}
          </button>
        ))}
      </div>
      {error && (
        <p role="alert" className="error-banner">
          {error}
        </p>
      )}
    </details>
  );
}
export function WatchlistActions({
  record,
  onRefresh,
}: {
  record: AtlasRecord;
  onRefresh: () => Promise<void>;
}) {
  const [price, setPrice] = useState<number | undefined>(
      record.lastSnapshot?.priceMinor,
    ),
    [availability, setAvailability] = useState(
      record.lastSnapshot?.availability || "unknown",
    ),
    [evidence, setEvidence] = useState(""),
    [due, setDue] = useState(""),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  async function run(method: string, payload: any) {
    setBusy(true);
    try {
      await rpc(method, {
        id: record.id,
        expectedVersion: record.version,
        ...payload,
      });
      await onRefresh();
      setError("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <details className="watchlist-actions">
      <summary>Source snapshot & recheck</summary>
      <p className="field-help">
        Manual snapshots only. No live price or availability monitoring is
        connected.
      </p>
      <p>
        {record.lastCheckedAt || record.checkedAt || "Not checked"} ·{" "}
        {record.lastSnapshot?.availability ||
          record.availability ||
          "Availability unknown"}
      </p>
      {record.url && (
        <button
          className="text-button"
          onClick={() =>
            openExternal(record.url).catch((e) => setError(String(e)))
          }
        >
          Open source
        </button>
      )}
      <MoneyField
        label="Last verified price"
        value={price}
        currency={record.currency || "USD"}
        onChange={setPrice}
      />
      <label className="field">
        <span>Availability</span>
        <select
          value={availability}
          onChange={(e) => setAvailability(e.target.value)}
        >
          {["unknown", "available", "unavailable", "stale"].map((a) => (
            <option key={a}>{a}</option>
          ))}
        </select>
      </label>
      <label className="field">
        <span>Source evidence / notes</span>
        <textarea
          value={evidence}
          onChange={(e) => setEvidence(e.target.value)}
        />
      </label>
      <button
        className="secondary"
        disabled={busy}
        onClick={() =>
          run("watchlist.snapshot", {
            snapshot: {
              priceMinor: price,
              currency: record.currency || "USD",
              availability,
              evidence,
              sourceUrl: record.url,
            },
          })
        }
      >
        Save manual snapshot
      </button>
      <label className="field">
        <span>Recheck reminder (ISO with offset)</span>
        <input
          value={due}
          onChange={(e) => setDue(e.target.value)}
          placeholder="2026-12-01T09:00:00+01:00"
        />
      </label>
      <button
        className="secondary"
        disabled={busy || !due}
        onClick={() => run("watchlist.remind", { dueAt: due })}
      >
        Remind me to recheck
      </button>
      {error && (
        <p className="error-banner" role="alert">
          {error}
        </p>
      )}
    </details>
  );
}
