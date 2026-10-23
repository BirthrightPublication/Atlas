import { useEffect, useState } from "react";
import { AtlasRecord, addDays, money, rpc, minorFromInput } from "./api";
import { AtlasTarget } from "./navigation";

export function GapReview({
  records,
  month,
  journey,
  onCreate,
  onNavigate,
}: {
  records: AtlasRecord[];
  month: string;
  journey: boolean;
  onCreate: (
    kind: string,
    date?: string,
    defaults?: Partial<AtlasRecord>,
  ) => void;
  onNavigate: (target: AtlasTarget) => boolean;
}) {
  const [analysis, setAnalysis] = useState<any>(null);
  const [error, setError] = useState("");
  const [rangeMonths, setRangeMonths] = useState(3);
  useEffect(() => {
    let alive = true;
    const first = `${month}-01`;
    const end = new Date(`${first}T12:00:00Z`);
    end.setUTCMonth(end.getUTCMonth() + (journey ? rangeMonths : 1));
    rpc("planning.gaps", { start: first, end: end.toISOString().slice(0, 10) })
      .then((r) => {
        if (alive) {
          setAnalysis(r);
          setError("");
        }
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, [records, month, journey, rangeMonths]);
  if (!records.some((r) => r.kind === "stay" || r.kind === "transport"))
    return null;
  return (
    <details className="planning-summary" open={journey}>
      <summary>
        Accommodation review · {analysis?.ranges?.length || 0} ranges to review
      </summary>
      {journey && (
        <label className="field">
          <span>Journey review range</span>
          <select
            value={rangeMonths}
            onChange={(e) => setRangeMonths(Number(e.target.value))}
          >
            <option value={3}>3 months</option>
            <option value={6}>6 months</option>
            <option value={12}>12 months</option>
          </select>
        </label>
      )}
      {error && <p role="alert">{error}</p>}
      {analysis?.ranges?.length === 0 && (
        <p>
          Confirmed stays or intentional overnight journeys cover this range.
        </p>
      )}
      {analysis?.ranges?.map((gap: any) => (
        <details className="gap-row" key={`${gap.start}-${gap.status}`}>
          <summary>
            <span className="numeric">
              {gap.start} → {gap.end} · {gap.nights}
            </span>{" "}
            nights ·{" "}
            {gap.status === "tentative"
              ? "tentative accommodation"
              : gap.status === "overlap"
                ? "overlapping reservations"
                : "unplanned accommodation"}
          </summary>
          <p>
            {gap.previousStay
              ? `After ${gap.previousStay.name} (${gap.previousStay.address || "location unknown"})`
              : "Preceding location unknown"}{" "}
            ·{" "}
            {gap.nextStay
              ? `Before ${gap.nextStay.name} (${gap.nextStay.address || "location unknown"})`
              : "Following location unknown"}
          </p>
          <div className="inline-actions">
            <button
              className="secondary"
              onClick={() => onNavigate(gap.target)}
            >
              View date
            </button>
            <button
              className="secondary"
              onClick={() =>
                onCreate("stay", gap.start, {
                  checkOut: gap.end,
                  status: "tentative",
                })
              }
            >
              Add stay
            </button>
            <button
              className="secondary"
              onClick={() =>
                onCreate("transport", gap.start, {
                  origin: gap.previousStay?.address || "",
                  destination: gap.nextStay?.address || "",
                  originStayId: gap.previousStay?.id,
                  destinationStayId: gap.nextStay?.id,
                  status: "tentative",
                })
              }
            >
              Add transport
            </button>
            <button
              className="text-button"
              onClick={() =>
                onNavigate({ module: "Discover", date: gap.start })
              }
            >
              Search in Discover
            </button>
          </div>
        </details>
      ))}
      <p className="field-help">
        Checkout is exclusive. Tentative options remain unconfirmed; explicitly
        marked overnight transport covers travel nights. Discover uses saved
        offers and provider links.
      </p>
    </details>
  );
}

export function DraftAnalysis({
  result,
  error,
  onWorkDraft,
}: {
  result: any;
  error: string;
  onWorkDraft?: (record: AtlasRecord) => void;
}) {
  if (!result)
    return error ? (
      <p className="field-help">Draft calculation: {error}</p>
    ) : null;
  return (
    <section className="planning-summary" aria-label="Draft calculations">
      {result.nights && (
        <p>
          <span className="numeric">{result.nights}</span> occupied nights ·{" "}
          <span className="numeric">
            {money(
              result.nightly.numeratorMinor / result.nightly.denominatorNights,
              result.currency || "USD",
            )}
          </span>{" "}
          per night from the entered total.
        </p>
      )}
      {result.durationMinutes != null && (
        <p>
          <span className="numeric">{result.durationMinutes}</span> minutes
          elapsed · Departure {result.departureLocal} · Arrival{" "}
          {result.arrivalLocal}
        </p>
      )}
      {result.warnings?.map((w: any, i: number) => (
        <p className="info-note" key={i}>
          {w.message}
        </p>
      ))}
      {result.workspace && (
        <details>
          <summary>Workspace: {result.workspace.status}</summary>
          <p>Verified: {result.workspace.verified.join(", ") || "None"}.</p>
          <p>Unknown: {result.workspace.unknown.join(", ") || "None"}.</p>
          <p>Conflicts: {result.workspace.conflicts.join(", ") || "None"}.</p>
          {result.workspace.alternate && (
            <p>
              Alternative: {result.workspace.alternate.name} · Commute{" "}
              {result.workspace.commuteMinutes ?? "unknown"} minutes. Opening
              hours{" "}
              {result.workspace.openingHours
                ? JSON.stringify(result.workspace.openingHours)
                : "unknown"}
              .
            </p>
          )}
        </details>
      )}
      {(
        result.workChecks ||
        (result.work ? [{ date: "", result: result.work }] : [])
      ).map((check: any, index: number) => (
        <details key={index} open={!!check.result.conflicts?.length}>
          <summary>
            Work compatibility · {check.date} ·{" "}
            {check.result.conflicts?.length || 0} conflicts
          </summary>
          {check.result.officeWindows?.map((w: any, i: number) => (
            <p key={i}>
              Employer: {w.officeStart} → {w.officeEnd}
              <br />
              Destination: {w.localStart} → {w.localEnd}
            </p>
          ))}
          {check.result.conflicts?.map((c: any, i: number) => (
            <p className="info-note" key={i}>
              {c.message}
            </p>
          ))}
          {check.result.warnings?.map((c: any, i: number) => (
            <p className="field-help" key={i}>
              {c.message || c}
            </p>
          ))}
          <p className="field-help">
            This is a draft calculation. Review available times in the selected
            workday before saving; confirmed records and protected work are
            unchanged.
          </p>
        </details>
      ))}
    </section>
  );
}

export function BudgetScenario({
  records,
  month,
  currency,
}: {
  records: AtlasRecord[];
  month: string;
  currency: string;
}) {
  const stays = records.filter(
    (r) => r.kind === "stay" && r.status !== "cancelled",
  );
  const [stayId, setStayId] = useState("");
  const [extra, setExtra] = useState(7);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!stayId) {
      setResult(null);
      return;
    }
    let alive = true;
    const timer = setTimeout(() => {
      rpc("planning.what_if", { stayId, additionalNights: extra })
        .then((v) => {
          if (alive) {
            setResult(v);
            setError("");
          }
        })
        .catch((e) => {
          if (alive) {
            setError(String(e));
            setResult(null);
          }
        });
    }, 200);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [stayId, extra, records]);
  return (
    <>
      <details className="planning-summary">
        <summary>Explore an accommodation scenario</summary>
        <p>
          Estimate an extension from the recorded nightly average. Saved
          bookings, payments and exchange-rate history remain unchanged.
        </p>
        <div className="form-grid">
          <label className="field">
            <span>Scenario stay</span>
            <select value={stayId} onChange={(e) => setStayId(e.target.value)}>
              <option value="">Select a saved stay</option>
              {stays.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name} · {r.currency}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>Additional nights</span>
            <input
              type="number"
              min={1}
              max={3660}
              value={extra}
              onChange={(e) => setExtra(Number(e.target.value))}
            />
          </label>
        </div>
        {error && <p role="alert">{error}</p>}
        {result && (
          <>
            <p>
              Estimated extra cost:{" "}
              <strong className="numeric">
                {money(
                  result.scenario.estimatedAdditionalMinor,
                  result.currency,
                )}
              </strong>{" "}
              · New checkout: {result.scenario.checkOut}
            </p>
            {result.monthlyEstimates?.map((r: any) => (
              <p key={r.month}>
                {r.month}: {money(r.originalMinor, result.currency)} →{" "}
                {money(r.estimatedMinor, result.currency)} (
                {money(r.deltaMinor, result.currency)} difference)
              </p>
            ))}
            {result.conflicts.map((r: any, i: number) => (
              <p key={i} className="info-note">
                {r.name}: {r.message}
              </p>
            ))}
            <p className="field-help">
              A fresh quote is required. Fees, discounts and availability may
              differ.
            </p>
          </>
        )}
      </details>
      <TransportScenario records={records} month={month} />
    </>
  );
}

function TransportScenario({
  records,
  month,
}: {
  records: AtlasRecord[];
  month: string;
}) {
  const [id, setId] = useState("");
  const [estimate, setEstimate] = useState("");
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState("");
  const transport = records.find((r) => r.id === id);
  useEffect(() => {
    setResult(null);
    if (!transport || !estimate) return;
    let alive = true;
    const timer = setTimeout(async () => {
      try {
        const totalMinor = minorFromInput(estimate, transport.currency);
        const value = await rpc("planning.what_if", {
          transportId: id,
          totalMinor,
          month,
        });
        if (alive) {
          setResult(value);
          setError("");
        }
      } catch (e) {
        if (alive) setError(String(e));
      }
    }, 200);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [id, estimate, month, records]);
  return (
    <details className="planning-summary">
      <summary>Explore a transport scenario</summary>
      <p>
        Compare an entered price with the saved plan for {month}. This is a
        month-of-use estimate; existing payments and bookings stay unchanged.
      </p>
      <div className="form-grid">
        <label className="field">
          <span>Scenario transport</span>
          <select
            value={id}
            onChange={(e) => {
              setId(e.target.value);
              setEstimate("");
              setError("");
            }}
          >
            <option value="">Select saved transport</option>
            {records
              .filter((r) => r.kind === "transport" && r.status !== "cancelled")
              .map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name} · {r.currency}
                </option>
              ))}
          </select>
        </label>
        <label className="field">
          <span>
            Hypothetical total ({transport?.currency || "original currency"})
          </span>
          <input
            inputMode="decimal"
            value={estimate}
            onChange={(e) => setEstimate(e.target.value)}
            disabled={!transport}
          />
        </label>
      </div>
      {error && <p role="alert">{error}</p>}
      {result && (
        <p>
          Monthly plan:{" "}
          <strong>
            {money(result.before.forecastMinor, result.currency)} →{" "}
            {money(result.after.forecastMinor, result.currency)}
          </strong>
          <br />
          Planned per day:{" "}
          {money(result.before.dailyForecastMinor, result.currency)} →{" "}
          {money(result.after.dailyForecastMinor, result.currency)} across{" "}
          {result.after.daysInMonth} days. This scenario includes the selected
          transport even if it is tentative.
        </p>
      )}
    </details>
  );
}
