import { useEffect, useState } from "react";
import {
  ArrowDownLeft,
  ArrowUpRight,
  Wallet,
  Plus,
  Settings2,
  CalendarDays,
  Home,
  ArrowRight,
  Shield,
  Landmark,
  RefreshCw,
} from "lucide-react";
import { AtlasRecord, money, prettyDate, rpc } from "./api";
import { BudgetScenario } from "./Planning";

export function Budget({
  records,
  summary,
  month,
  currency,
  onEditBudget,
  onCreate,
  onSelect,
}: {
  records: AtlasRecord[];
  summary: any;
  month: string;
  currency: string;
  onEditBudget: () => void;
  onCreate: (kind: string) => void;
  onSelect: (r: AtlasRecord) => void;
}) {
  const [view, setView] = useState("Month of use");
  const [periodStart, setPeriodStart] = useState(`${month}-01`);
  const [periodEnd, setPeriodEnd] = useState(`${month}-28`);
  const [period, setPeriod] = useState<any>(null);
  const [error, setError] = useState("");
  const payments = records
    .filter((r) => r.kind === "payment" && r.date?.slice(0, 7) === month)
    .sort((a, b) => a.date.localeCompare(b.date));
  const expenses = records.filter(
    (r) => r.kind === "expense" && r.date?.slice(0, 7) === month,
  );
  const budgets = records.filter(
    (r) => r.kind === "budget" && r.month === month,
  );
  const available = summary?.availableMinor;
  const costs = [
    ["Accommodation", summary?.accommodationMinor, Home],
    ["Transport", summary?.transportMinor, ArrowUpRight],
    ["Objectives", summary?.objectivesMinor, CalendarDays],
    ["Recorded expenses", summary?.expensesMinor, Wallet],
    ["Recurring commitments", summary?.recurringMinor, RefreshCw],
  ] as const;
  return (
    <div className="budget-page">
      <div className="page-intro budget-intro">
        <div>
          <h1>Budget</h1>
          <p>One financial history. Three useful ways to see it.</p>
        </div>
        <button className="secondary" onClick={onEditBudget}>
          <Settings2 size={15} />
          Set monthly plan
        </button>
      </div>
      {summary?.hasBudget === false && (
        <div className="info-note">
          No monthly cash plan has been entered. Add your income, opening
          balance, allowance, and reserve to make available-cash projections
          useful.
        </div>
      )}
      {summary?.warnings?.map((warning: any, i: number) => (
        <div className="info-note" key={i}>
          {typeof warning === "string"
            ? warning
            : warning.message || warning.reason || JSON.stringify(warning)}
        </div>
      ))}
      {!summary && (
        <div className="info-note">
          Loading the exact budget from your local records…
        </div>
      )}
      <div className="budget-metrics">
        <div className="metric primary-metric">
          <span>FORECAST MONTHLY COST</span>
          <strong>{money(summary?.forecastMinor, currency)}</strong>
          <small>
            {month} · {summary?.daysInMonth || "—"} calendar days
          </small>
        </div>
        <div className="metric">
          <span>TOTAL PLANNED COST PER DAY</span>
          <strong>{money(summary?.dailyForecastMinor, currency)}</strong>
          <small>
            Forecast ÷ {summary?.daysInMonth || "—"} days this month
          </small>
        </div>
        <div className="metric">
          <span>FLEXIBLE PER REMAINING DAY</span>
          <strong>{money(summary?.dailyAllowanceMinor, currency)}</strong>
          <small>
            {money(summary?.allowanceRemainingMinor, currency)} ÷{" "}
            {summary?.remainingDays ?? "—"} eligible days
          </small>
        </div>
      </div>
      <div className="budget-daily-details" aria-label="Daily budget details">
        <p>
          Accommodation per night:{" "}
          <strong className="numeric">
            {money(summary?.nightlyAccommodationMinor, currency)}
          </strong>{" "}
          across {summary?.occupiedNights ?? "—"} occupied nights.
        </p>
        <p>
          Projected available funds:{" "}
          <strong className="numeric">
            {money(summary?.availableMinor, currency)}
          </strong>{" "}
          after scheduled cash flow, remaining allowance and{" "}
          {money(summary?.reserveMinor, currency)} reserve.
        </p>
        <p>
          Flexible spending: {money(summary?.allowanceSpentMinor, currency)}{" "}
          recorded · {money(summary?.allowanceCommittedMinor, currency)}{" "}
          committed · {money(summary?.allowanceRemainingMinor, currency)}{" "}
          remaining. Recorded expenses are deducted once.
        </p>
      </div>
      <BudgetScenario records={records} month={month} currency={currency} />
      <div className="subnav">
        <div className="segmented">
          {["Month of use", "Cash flow", "Available cash"].map((v) => (
            <button
              className={view === v ? "active" : ""}
              key={v}
              onClick={() => setView(v)}
            >
              {v}
            </button>
          ))}
        </div>
        <span className="spacer" />
        <button className="text-button" onClick={() => onCreate("expense")}>
          <Plus size={15} />
          Record expense
        </button>
      </div>
      {view === "Month of use" && (
        <div className="budget-columns">
          <section className="budget-breakdown">
            <div className="section-heading">
              <h2>The cost of being here.</h2>
              <span className="muted">{currency}</span>
            </div>
            {costs.map(([label, value, Icon]) => (
              <div className="budget-row" key={label}>
                <span className="budget-row-icon">
                  <Icon size={17} />
                </span>
                <span>{label}</span>
                <strong>{money(value, currency)}</strong>
                <div className="budget-bar">
                  <i
                    style={{
                      width: `${summary?.forecastMinor > 0 ? Math.min(100, Math.max(0, ((value || 0) / summary.forecastMinor) * 100)) : 0}%`,
                    }}
                  />
                </div>
              </div>
            ))}
            <div className="budget-row total">
              <span>Committed + flexible forecast</span>
              <strong>{money(summary?.forecastMinor, currency)}</strong>
            </div>
            <p className="field-help">
              Stays are allocated across occupied nights. Payment dates belong
              to cash flow. Tentative alternatives are excluded.
            </p>
          </section>
          <section className="budget-side">
            <div className="daily-card">
              <Home size={23} />
              <span>ACCOMMODATION PER NIGHT</span>
              <strong>
                {money(summary?.nightlyAccommodationMinor, currency)}
              </strong>
              <p>
                {money(summary?.accommodationMinor, currency)} ÷{" "}
                {summary?.occupiedNights ?? "—"} occupied nights in this month
              </p>
            </div>
            <div className="allowance-card">
              <div>
                <span>Flexible allowance</span>
                <strong>{money(summary?.allowanceMinor, currency)}</strong>
              </div>
              <div>
                <span>Remaining after spending</span>
                <strong>
                  {money(summary?.allowanceRemainingMinor, currency)}
                </strong>
              </div>
              <p>
                Recorded grocery and errand spending consumes the allowance
                once.
              </p>
            </div>
          </section>
        </div>
      )}
      {view === "Cash flow" && (
        <>
          <div className="cash-summary">
            <div>
              <span>Expected / actual net outflow</span>
              <strong>{money(summary?.cashFlowMinor, currency)}</strong>
            </div>
            <div>
              <span>Actually settled</span>
              <strong>{money(summary?.settledCashFlowMinor, currency)}</strong>
            </div>
            <button className="secondary" onClick={() => onCreate("payment")}>
              <Plus size={15} />
              Record payment
            </button>
          </div>
          <div className="transaction-table">
            <div className="transaction-head">
              <span>PAYMENT / BOOKING</span>
              <span>DATE</span>
              <span>STATE</span>
              <span>AMOUNT</span>
            </div>
            {payments.length === 0 ? (
              <div className="empty small">
                <Wallet size={27} />
                <h3>No payments this month.</h3>
                <p>
                  Record when money changes hands, even if your stay happens
                  later.
                </p>
              </div>
            ) : (
              payments.map((r) => (
                <button
                  className="transaction-row"
                  key={r.id}
                  onClick={() => onSelect(r)}
                >
                  <span>
                    <strong>
                      {records.find((b) => b.id === r.bookingId)?.name ||
                        r.name ||
                        "Booking payment"}
                    </strong>
                    <small>{r.type?.replace("_", " ")}</small>
                  </span>
                  <span>{prettyDate(r.date)}</span>
                  <span className="status-badge">{r.status}</span>
                  <strong>{money(r.amountMinor, r.currency)}</strong>
                </button>
              ))
            )}
          </div>
          <p className="field-help">
            Refunds are negative cash movements linked to the original booking.
            Refundable deposits affect cash while tied up, not ordinary consumed
            travel cost.
          </p>
        </>
      )}
      {view === "Available cash" && (
        <div className="available-layout">
          <div className="available-card">
            <Shield size={26} />
            <span>PROJECTED AFTER YOUR RESERVE</span>
            <strong className={available < 0 ? "danger" : ""}>
              {money(available, currency)}
            </strong>
            <p>
              {month} · Based on your entered cash position and scheduled
              commitments
            </p>
          </div>
          <div className="available-math">
            <div>
              <span>Opening cash balance</span>
              <strong>{money(summary?.openingBalanceMinor, currency)}</strong>
            </div>
            <div>
              <span>Take-home income</span>
              <strong>+ {money(summary?.incomeMinor, currency)}</strong>
            </div>
            <div>
              <span>Cash outflows</span>
              <strong>− {money(summary?.cashFlowMinor, currency)}</strong>
            </div>
            <div>
              <span>Remaining flexible allowance</span>
              <strong>
                − {money(summary?.allowanceRemainingMinor, currency)}
              </strong>
            </div>
            <div>
              <span>Protected reserve</span>
              <strong>− {money(summary?.reserveMinor, currency)}</strong>
            </div>
            <button className="text-button" onClick={onEditBudget}>
              Edit your cash plan
              <ArrowRight size={14} />
            </button>
            <p className="field-help">
              Your income is never assumed. Confirm the opening balance and
              payment schedule before relying on this projection.
            </p>
          </div>
        </div>
      )}
      {summary?.excludedCurrencies?.length > 0 && (
        <div className="info-note">
          Separate currencies excluded from this {currency} total:{" "}
          {summary.excludedCurrencies.join(", ")}. Original amounts are
          preserved; no live conversion is assumed.
        </div>
      )}
      {Array.isArray(summary?.allocations) &&
        summary.allocations.length > 0 && (
          <section className="allocations">
            <div className="section-heading">
              <h3>Stay allocation</h3>
              <span className="muted">Occupied nights, not payment month</span>
            </div>
            {summary.allocations.map((a: any, i: number) => (
              <div
                className="allocation-row"
                key={a.stayId || a.id || a.bookingId || i}
              >
                <span>
                  {a.name ||
                    records.find(
                      (r) => r.id === (a.stayId || a.id || a.bookingId),
                    )?.name ||
                    "Stay"}
                </span>
                <small>
                  {a.nightCount ?? a.nights ?? a.occupiedNights ?? "—"} nights
                  this month
                </small>
                <strong>
                  {money(
                    a.amountMinor ?? a.allocatedMinor ?? a.costMinor,
                    currency,
                  )}
                </strong>
              </div>
            ))}
          </section>
        )}
      <section className="period-panel">
        <div>
          <span className="eyebrow">YOUR OWN TIMEFRAME</span>
          <h3>What does this chapter cost per day?</h3>
        </div>
        <div className="inline-actions">
          <label className="field">
            <span>From (inclusive)</span>
            <input
              type="date"
              value={periodStart}
              onChange={(e) => setPeriodStart(e.target.value)}
            />
          </label>
          <label className="field">
            <span>To (exclusive)</span>
            <input
              type="date"
              value={periodEnd}
              onChange={(e) => setPeriodEnd(e.target.value)}
            />
          </label>
          <button
            className="secondary"
            onClick={async () => {
              try {
                setPeriod(
                  await rpc("budget.period", {
                    start: periodStart,
                    end: periodEnd,
                    currency,
                  }),
                );
                setError("");
              } catch (e) {
                setError(String(e));
              }
            }}
          >
            Calculate
          </button>
        </div>
        {period && (
          <p className="period-result">
            {money(period.dailyForecastMinor, currency)} / day{" "}
            <span>
              {periodStart} → {periodEnd} ·{" "}
              {period.days ?? period.dayCount ?? "—"} days ·{" "}
              {money(period.totalMinor ?? period.forecastMinor, currency)} total
            </span>
          </p>
        )}
        {error && <div className="error-banner">{error}</div>}
      </section>
      {expenses.length > 0 && (
        <section>
          <div className="section-heading">
            <h3>Recorded spending</h3>
            <span className="muted">{expenses.length} entries</span>
          </div>
          {expenses.map((r) => (
            <button
              className="transaction-row expense-row"
              key={r.id}
              onClick={() => onSelect(r)}
            >
              <span>
                <strong>{r.name}</strong>
                <small>{r.category}</small>
              </span>
              <span>{prettyDate(r.date)}</span>
              <strong>{money(r.amountMinor, r.currency)}</strong>
            </button>
          ))}
        </section>
      )}
    </div>
  );
}
