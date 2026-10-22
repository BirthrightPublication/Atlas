import {
  ChevronRight,
  Home,
  Plane,
  MapPin,
  Check,
  ArrowUpRight,
  Circle,
} from "lucide-react";
import {
  AtlasRecord,
  addDays,
  dateOf,
  dayKey,
  money,
  nights,
  prettyDate,
  zonedDay,
  zonedTime,
  travelNight,
} from "./api";

type Props = {
  records: AtlasRecord[];
  month: string;
  selectedDate: string;
  view: string;
  onSelect: (r: AtlasRecord) => void;
  onDay: (d: string) => void;
  onCreate: (kind: string, date?: string) => void;
};
const active = (r: AtlasRecord) => r.status !== "cancelled";
export function Calendar({
  records,
  month,
  selectedDate,
  view,
  onSelect,
  onDay,
  onCreate,
}: Props) {
  const stays = records.filter((r) => r.kind === "stay" && active(r));
  const transports = records.filter((r) => r.kind === "transport" && active(r));
  const objectives = records
    .filter((r) => ["objective", "workBlock"].includes(r.kind))
    .map((r) =>
      r.kind === "workBlock"
        ? {
            ...r,
            category: "Protected work",
            date: r.date || zonedDay(r.start, r.timezone),
            startTime: r.startTime || zonedTime(r.start, r.timezone),
          }
        : r,
    );
  const first = dateOf(`${month}-01`);
  const start = new Date(first);
  start.setDate(first.getDate() - ((first.getDay() + 6) % 7));
  const weeks = Array.from({ length: 6 }, (_, w) =>
    Array.from({ length: 7 }, (_, d) => addDays(dayKey(start), w * 7 + d)),
  );
  if (weeks[5][0].slice(0, 7) !== month) weeks.pop();
  const selectedStays = stays.filter(
    (r) => r.checkIn <= selectedDate && r.checkOut > selectedDate,
  );
  const itinerary = records
    .filter(
      (r) => ["stay", "transport", "objective"].includes(r.kind) && active(r),
    )
    .sort((a, b) => recordDate(a).localeCompare(recordDate(b)));
  const periodDays =
    view === "day"
      ? [selectedDate]
      : Array.from({ length: 7 }, (_, i) => addDays(selectedDate, i));

  return (
    <>
      {view === "journey" ? (
        <section className="journey-view" aria-label="Journey timeline">
          <div className="section-heading">
            <div>
              <span className="eyebrow">THE LONG VIEW</span>
              <h2>Your journey, connected</h2>
            </div>
            <span className="muted">All dates · both contexts</span>
          </div>
          {itinerary.length === 0 && (
            <Empty onCreate={() => onCreate("stay")} />
          )}
          {itinerary.map((r, index) => (
            <div
              className={`timeline-item ${r.kind}`}
              key={r.occurrenceId || r.id}
            >
              <div className="timeline-date">
                <strong>{prettyDate(recordDate(r))}</strong>
                <small>{recordDate(r).slice(0, 4)}</small>
              </div>
              <span className="timeline-dot" />
              <button className="timeline-card" onClick={() => onSelect(r)}>
                <div className="record-label">
                  {r.kind === "stay" ? (
                    <Home size={17} />
                  ) : r.kind === "transport" ? (
                    <Plane size={15} />
                  ) : (
                    <MapPin size={15} />
                  )}
                  <span>
                    {r.kind === "stay"
                      ? `${nights(r.checkIn, r.checkOut)} nights · ${r.context === "on_road" ? "On Road" : "On Foot"}`
                      : r.kind}
                  </span>
                  <span className="spacer" />
                  <small>{r.status}</small>
                </div>
                <h3>{r.name}</h3>
                <p>
                  {r.kind === "stay"
                    ? `${r.address || "Address not added"} · ${prettyDate(r.checkIn)} – ${prettyDate(r.checkOut)}`
                    : r.kind === "transport"
                      ? `${r.origin} → ${r.destination}`
                      : r.notes || r.category}
                </p>
              </button>
            </div>
          ))}
        </section>
      ) : (
        <>
          <div
            className={
              view === "month" ? "month-view desktop-calendar" : "hidden"
            }
          >
            <div className="weekdays">
              {["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"].map((d) => (
                <span key={d}>{d}</span>
              ))}
            </div>
            {weeks.map((week, wi) => {
              const weekStays = stays
                .filter((r) => r.checkIn <= week[6] && r.checkOut > week[0])
                .sort((a, b) => a.checkIn.localeCompare(b.checkIn));
              const lanes: string[] = [];
              const bands = weekStays.map((r) => {
                const from = Math.max(
                  0,
                  week.findIndex((d) => d >= r.checkIn),
                );
                const end = week.findIndex((d) => d >= r.checkOut);
                const until = end < 0 ? 7 : end;
                let lane = lanes.findIndex((d) => d <= r.checkIn);
                if (lane < 0) lane = lanes.length;
                lanes[lane] = r.checkOut;
                return { r, from, until, lane };
              });
              return (
                <div
                  className="calendar-week"
                  key={week[0]}
                  style={{
                    minHeight: 138 + Math.max(0, lanes.length - 1) * 34,
                  }}
                >
                  <div className="day-cells">
                    {week.map((d) => (
                      <button
                        key={d}
                        aria-label={`Select ${d}`}
                        className={`day-cell ${d.slice(0, 7) !== month ? "outside" : ""} ${d === selectedDate ? "selected" : ""} ${d === dayKey(new Date()) ? "today" : ""}`}
                        onClick={() => onDay(d)}
                      >
                        <span className="day-number">{Number(d.slice(8))}</span>
                        {d === dayKey(new Date()) && <small>TODAY</small>}
                      </button>
                    ))}
                  </div>
                  <div
                    className="stay-bands"
                    style={{ height: Math.max(1, lanes.length) * 34 }}
                  >
                    {bands.map(({ r, from, until, lane }) => (
                      <button
                        key={r.occurrenceId || r.id}
                        className={`stay-band ${r.context === "on_road" ? "road" : ""} ${r.status === "tentative" ? "tentative" : ""}`}
                        style={{
                          gridColumn: `${from + 1} / ${until + 1}`,
                          gridRow: lane + 1,
                        }}
                        onClick={() => onSelect(r)}
                        title={`${r.name} · ${r.checkIn} – ${r.checkOut}`}
                      >
                        <Home size={12} />
                        <span>{r.name}</span>
                        <small>{nights(r.checkIn, r.checkOut)}n</small>
                      </button>
                    ))}
                  </div>
                  <div className="day-events">
                    {week.map((d) => (
                      <div key={d} className="day-event-cell">
                        {transports.some((r) => travelNight(r, d)) &&
                          !stays.some(
                            (r) => r.checkIn <= d && r.checkOut > d,
                          ) && (
                            <span className="night-marker travel">
                              Travel night
                            </span>
                          )}
                        {stays.length > 0 &&
                          d.slice(0, 7) === month &&
                          !stays.some(
                            (r) => r.checkIn <= d && r.checkOut > d,
                          ) &&
                          !transports.some((r) => travelNight(r, d)) && (
                            <span className="night-marker">Open night</span>
                          )}
                        {objectives
                          .filter((r) => r.date === d)
                          .slice(0, 2)
                          .map((r) => (
                            <button
                              className={`objective-chip ${r.status === "done" ? "done" : ""}`}
                              key={r.occurrenceId || r.id}
                              onClick={() => onSelect(r)}
                            >
                              {r.status === "done" ? (
                                <Check size={11} />
                              ) : (
                                <span className="event-dot" />
                              )}
                              <span>{r.name}</span>
                            </button>
                          ))}
                        {transports
                          .filter(
                            (r) =>
                              zonedDay(r.departure, r.departureTimezone) === d,
                          )
                          .map((r) => (
                            <button
                              className="transport-chip"
                              key={r.occurrenceId || r.id}
                              onClick={() => onSelect(r)}
                            >
                              <Plane size={11} />
                              <span>{r.name}</span>
                            </button>
                          ))}
                      </div>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
          <div className={view === "month" ? "mobile-agenda" : "agenda-view"}>
            <div className="agenda-pager">
              <button
                className="icon-button"
                aria-label={view === "day" ? "Previous day" : "Previous week"}
                onClick={() =>
                  onDay(addDays(selectedDate, view === "day" ? -1 : -7))
                }
              >
                ‹
              </button>
              <label>
                <span className="sr-only">Agenda start date</span>
                <input
                  type="date"
                  aria-label="Agenda start date"
                  value={selectedDate}
                  onChange={(e) => {
                    if (e.target.value) onDay(e.target.value);
                  }}
                />
              </label>
              <button
                className="icon-button"
                aria-label={view === "day" ? "Next day" : "Next week"}
                onClick={() =>
                  onDay(addDays(selectedDate, view === "day" ? 1 : 7))
                }
              >
                ›
              </button>
            </div>
            {(view === "month" ? periodDays : periodDays).map((d) => (
              <section key={d} className="agenda-day">
                <button className="agenda-day-label" onClick={() => onDay(d)}>
                  <strong>
                    {dateOf(d).toLocaleDateString("en", { weekday: "short" })}
                  </strong>
                  <span>{Number(d.slice(8))}</span>
                  <small>{prettyDate(d, { month: "short" })}</small>
                </button>
                <div className="agenda-entries">
                  {transports.some((r) => travelNight(r, d)) &&
                    !stays.some((r) => r.checkIn <= d && r.checkOut > d) && (
                      <div className="agenda-travel-night">
                        <Plane size={13} />
                        Intentionally traveling overnight
                      </div>
                    )}
                  {stays
                    .filter((r) => r.checkIn <= d && r.checkOut > d)
                    .map((r) => (
                      <button
                        className="agenda-stay"
                        key={r.occurrenceId || r.id}
                        onClick={() => onSelect(r)}
                      >
                        <Home size={18} />
                        <div>
                          <strong>{r.name}</strong>
                          <small>
                            {r.address || "Address not added"} · night{" "}
                            {nights(r.checkIn, d) + 1} of{" "}
                            {nights(r.checkIn, r.checkOut)}
                          </small>
                        </div>
                        <ChevronRight size={16} />
                      </button>
                    ))}
                  {objectives
                    .filter((r) => r.date === d)
                    .map((r) => (
                      <button
                        className="agenda-objective"
                        key={r.occurrenceId || r.id}
                        onClick={() => onSelect(r)}
                      >
                        <span className="agenda-time">
                          {r.startTime || "Flexible"}
                        </span>
                        <div>
                          <strong>{r.name}</strong>
                          <small>
                            {r.category}{" "}
                            {r.status === "done" ? "· completed" : ""}
                          </small>
                        </div>
                      </button>
                    ))}
                  {transports
                    .filter(
                      (r) => zonedDay(r.departure, r.departureTimezone) === d,
                    )
                    .map((r) => (
                      <button
                        className="agenda-transport"
                        key={r.occurrenceId || r.id}
                        onClick={() => onSelect(r)}
                      >
                        <Plane size={14} />
                        <span>
                          {r.origin} → {r.destination}
                        </span>
                        <small>
                          {zonedTime(r.departure, r.departureTimezone)}{" "}
                          {r.departureTimezone}
                        </small>
                      </button>
                    ))}
                  {!records.some(
                    (r) =>
                      active(r) &&
                      ((r.kind === "stay" &&
                        r.checkIn <= d &&
                        r.checkOut > d) ||
                        r.date === d ||
                        (r.departure &&
                          zonedDay(r.departure, r.departureTimezone) === d)),
                  ) && (
                    <button
                      className="open-day"
                      onClick={() => onCreate("objective", d)}
                    >
                      Open day <span>+ Add a plan</span>
                    </button>
                  )}
                </div>
              </section>
            ))}
          </div>
          <div className="calendar-legend">
            <span>
              <i className="legend-stay" /> Stay
            </span>
            <span>
              <i className="legend-objective" /> Objective
            </span>
            <span>
              <Plane size={12} /> Transport
            </span>
            <span className="spacer" />
            <span>Checkout is exclusive · all travel contexts shown</span>
          </div>
          <section className="day-summary">
            <div>
              <span className="eyebrow">
                {prettyDate(selectedDate, {
                  weekday: "long",
                  month: "long",
                  day: "numeric",
                }).toUpperCase()}
              </span>
              <h2>{selectedStays[0]?.name || "Room for your next chapter"}</h2>
              <p>
                {selectedStays[0]?.address ||
                  "Add a stay, bring in a booking, or leave space to explore."}
              </p>
            </div>
            <button
              className="text-button"
              onClick={() => onCreate("objective", selectedDate)}
            >
              Plan this day <ArrowUpRight size={16} />
            </button>
          </section>
          {stays.length === 0 && (
            <div className="first-stay">
              <Home size={23} />
              <div>
                <strong>Every journey begins with a place.</strong>
                <span>
                  Add your first stay to bring your calendar and budget
                  together.
                </span>
              </div>
              <button className="secondary" onClick={() => onCreate("stay")}>
                Add a stay
              </button>
            </div>
          )}
          {objectives.some((r) => !r.date && r.status !== "done") && (
            <section className="unscheduled">
              <h3>
                For when there’s time{" "}
                <span className="muted">Unscheduled objectives</span>
              </h3>
              <div className="chips">
                {objectives
                  .filter((r) => !r.date && r.status !== "done")
                  .map((r) => (
                    <button
                      key={r.occurrenceId || r.id}
                      className="chip"
                      onClick={() => onSelect(r)}
                    >
                      <Circle size={12} />
                      {r.name}
                    </button>
                  ))}
              </div>
            </section>
          )}
        </>
      )}
    </>
  );
}
function recordDate(r: AtlasRecord) {
  return (
    r.checkIn ||
    (r.departure ? zonedDay(r.departure, r.departureTimezone) : "") ||
    r.date ||
    "9999-12-31"
  );
}
function Empty({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="empty">
      <Home size={32} />
      <h3>A journey of your own</h3>
      <p>Your stays, transport, and objectives will connect here.</p>
      <button className="primary" onClick={onCreate}>
        Add your first stay
      </button>
    </div>
  );
}
