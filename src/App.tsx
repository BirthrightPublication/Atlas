import { useCallback, useEffect, useRef, useState } from "react";
import {
  CalendarDays,
  Compass,
  Map,
  Wallet,
  Settings,
  Bell,
  Download,
  Sparkles,
  ChevronLeft,
  ChevronRight,
  Plus,
  ArrowDownToLine,
  ArrowUpRight,
  ArrowRight,
  Footprints,
  Car,
  Check,
  X,
  Home,
  Plane,
  Flag,
  Clock,
  ShieldCheck,
  Menu,
  RefreshCw,
  Send,
  CircleHelp,
  CircleDollarSign,
  FileText,
} from "lucide-react";
import {
  AtlasRecord,
  Settings as AtlasSettings,
  addDays,
  dateOf,
  dayKey,
  download,
  isDesktop,
  money,
  openExternal,
  prettyDate,
  rpc,
  travelNight,
} from "./api";
import { Calendar } from "./Calendar";
import { RecordForm, newRecord } from "./RecordForm";
import { ImportPanel } from "./Imports";
import { Discover, MapView } from "./Discover";
import { Budget } from "./Budget";
import { SettingsPanel } from "./SettingsPanel";
import { CompanionApp } from "./companion/CompanionApp";

export default function App() {
  if (window.location.pathname.startsWith("/companion"))
    return <CompanionApp />;
  return <DesktopApp />;
}

function DesktopApp() {
  const today = dayKey(new Date());
  const [records, setRecords] = useState<AtlasRecord[]>([]);
  const [settings, setSettings] = useState<AtlasSettings>({});
  const [tab, setTab] = useState("Calendar");
  const [month, setMonth] = useState(today.slice(0, 7));
  const [selectedDate, setSelectedDate] = useState(today);
  const [view, setView] = useState("month");
  const [panel, setPanel] = useState("");
  const [selected, setSelected] = useState<AtlasRecord | null>(null);
  const [importId, setImportId] = useState<string | undefined>();
  const [newMenu, setNewMenu] = useState(false);
  const [error, setError] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [summary, setSummary] = useState<any>(null);
  const [work, setWork] = useState<any>(null);
  const [calendarRecords, setCalendarRecords] = useState<AtlasRecord[]>([]);
  const [updatesAvailable, setUpdatesAvailable] = useState(false);
  const [notice, setNotice] = useState("");
  const [mobileNav, setMobileNav] = useState(false);
  const [now, setNow] = useState(new Date());
  const currency = settings.currency || "USD";
  const currentZone =
    settings.currentTimezone ||
    Intl.DateTimeFormat().resolvedOptions().timeZone;
  const officeZone = settings.officeTimezone || "America/Los_Angeles";
  const refresh = useCallback(async () => {
    try {
      const data = await rpc<{
        records: AtlasRecord[];
        settings: AtlasSettings;
      }>("snapshot");
      setRecords(data.records);
      setSettings(data.settings || {});
      setLoaded(true);
      setError("");
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    }
  }, []);
  useEffect(() => {
    void refresh();
    const tick = setInterval(() => setNow(new Date()), 30000);
    return () => clearInterval(tick);
  }, [refresh]);
  useEffect(() => {
    if (!loaded) return;
    rpc("budget.summary", { month, currency })
      .then(setSummary)
      .catch((e) => setError(String(e)));
  }, [loaded, month, currency, records]);
  useEffect(() => {
    if (!loaded) return;
    const planned = records.find(
      (r) =>
        r.kind === "stay" &&
        r.status !== "cancelled" &&
        r.checkIn <= selectedDate &&
        r.checkOut > selectedDate,
    );
    rpc("work.window", {
      date: selectedDate,
      timezone: planned?.timezone || currentZone,
    })
      .then(setWork)
      .catch((e) => setWork({ error: String(e) }));
  }, [loaded, selectedDate, currentZone, records]);
  useEffect(() => {
    if (!loaded || isDesktop()) return;
    const check = async () => {
      try {
        const due = await rpc<any[]>("reminders.due");
        if (due.length) {
          setNotice(
            `${due.length} reminder${due.length === 1 ? "" : "s"} due. Open notifications to review.`,
          );
          await refresh();
        }
      } catch {
        /* service state shown by snapshot failures */
      }
    };
    void check();
    const timer = setInterval(check, 30000);
    return () => clearInterval(timer);
  }, [loaded, refresh]);
  useEffect(() => {
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setPanel("");
        setSelected(null);
        setNewMenu(false);
        setMobileNav(false);
      }
    };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, []);
  useEffect(() => {
    if (!loaded) return;
    const start = addDays(`${month}-01`, -7);
    const end = addDays(`${month}-01`, 100);
    rpc<AtlasRecord[]>("calendar.occurrences", { start, end })
      .then(setCalendarRecords)
      .catch((e) => setError(String(e)));
  }, [loaded, month, records]);
  useEffect(() => {
    if (!isDesktop()) return;
    let disposed = false;
    const stops: Array<() => void> = [];
    (async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      try {
        const result: any = await invoke("update_status");
        if (!disposed) setUpdatesAvailable(!!result.available);
      } catch {
        /* updates panel reports detail */
      }
      const { listen } = await import("@tauri-apps/api/event");
      for (const event of ["atlas:reminder", "atlas:email-synced"]) {
        const stop = await listen(event, () => {
          void refresh();
        });
        if (disposed) stop();
        else stops.push(stop);
      }
    })();
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, [refresh]);
  const drafts = records.filter(
    (r) => r.kind === "draft" && !["accepted", "dismissed"].includes(r.status),
  );
  const reminders = records.filter(
    (r) => r.kind === "reminder" && !["dismissed"].includes(r.state),
  );
  const currentStay = records.find(
    (r) =>
      r.kind === "stay" &&
      r.status !== "cancelled" &&
      r.checkIn <= selectedDate &&
      r.checkOut > selectedDate,
  );
  const changeMonth = (amount: number) => {
    const date = dateOf(`${month}-01`);
    date.setMonth(date.getMonth() + amount);
    setMonth(dayKey(date).slice(0, 7));
    setSelectedDate(dayKey(date));
  };
  const create = (kind: string, date = selectedDate) => {
    setImportId(undefined);
    setSelected(newRecord(kind, date, settings));
    setPanel("");
    setNewMenu(false);
  };
  const select = (r: AtlasRecord) => {
    setSelected(records.find((x) => x.id === r.id) || r);
    setImportId(undefined);
    setPanel("");
  };
  const close = () => {
    setSelected(null);
    setImportId(undefined);
  };
  const saveHandler = async (r?: AtlasRecord) => {
    if (r && !r.id) {
      setSelected(r);
      return;
    }
    await refresh();
    setNotice("Saved to your local Atlas.");
  };
  async function context(value: string) {
    try {
      await rpc("settings.save", { settings: { context: value } });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }
  async function exportCalendar() {
    try {
      const result = await rpc<{ filename: string; content: string }>(
        "calendar.export",
      );
      download(result.filename, result.content, "text/calendar");
      setNotice("Calendar exported.");
    } catch (e) {
      setError(String(e));
    }
  }
  const navigation = [
    ["Calendar", CalendarDays],
    ["Discover", Compass],
    ["Map", Map],
    ["Budget", Wallet],
  ] as const;
  return (
    <div className="atlas-app">
      <aside className={`sidebar ${mobileNav ? "mobile-open" : ""}`}>
        <button
          className="brand"
          onClick={() => {
            setTab("Calendar");
            setMobileNav(false);
          }}
          aria-label="Birthright: Atlas home"
        >
          <span className="brand-symbol">
            A<span />
          </span>
          <span className="brand-words">
            <small>BIRTHRIGHT</small>
            <strong>
              Atlas<span>.</span>
            </strong>
          </span>
        </button>
        <div className="sidebar-label">A LIFE IN MOTION</div>
        <nav className="main-nav" aria-label="Main navigation">
          {navigation.map(([label, Icon]) => (
            <button
              key={label}
              className={tab === label ? "active" : ""}
              onClick={() => {
                setTab(label);
                setMobileNav(false);
              }}
            >
              <Icon size={19} strokeWidth={1.7} />
              <span>{label}</span>
              {tab === label && <i />}
            </button>
          ))}
        </nav>
        <div className="sidebar-line" />
        <button
          className="inbox-nav"
          onClick={() => {
            setPanel("imports");
            setMobileNav(false);
          }}
        >
          <ArrowDownToLine size={17} />
          <span>Import inbox</span>
          {drafts.length > 0 && <b>{drafts.length}</b>}
        </button>
        <div className="sidebar-bottom">
          <div className="journey-note">
            <span className="eyebrow">YOUR NEXT CHAPTER</span>
            <p>{currentStay?.name || "Leave room for possibility."}</p>
            <small>
              {currentStay
                ? `${prettyDate(currentStay.checkIn)} — ${prettyDate(currentStay.checkOut)}`
                : "Plans can change. Make them yours."}
            </small>
          </div>
          <button
            className="sidebar-setting"
            onClick={() => setPanel("settings")}
          >
            <Settings size={17} />
            Settings<span>⌘</span>
          </button>
          <div className="local-status">
            <span className={`status-dot ${loaded ? "online" : ""}`} />
            {loaded ? "Local workspace" : "Connecting to local service"}
            <ShieldCheck size={13} />
          </div>
        </div>
      </aside>
      <main className="main-shell">
        <header className="topbar">
          <button
            className="icon-button mobile-menu"
            aria-label="Open navigation"
            onClick={() => setMobileNav(!mobileNav)}
          >
            <Menu size={21} />
          </button>
          <div className="breadcrumb">
            YOUR WORLD<span>/</span>
            <strong>{tab}</strong>
          </div>
          <div className="topbar-actions">
            <div className="context-control">
              <span>NEW ENTRY DEFAULT</span>
              <div className="context-toggle">
                <button
                  className={
                    (settings.context || "on_foot") === "on_foot"
                      ? "active"
                      : ""
                  }
                  onClick={() => context("on_foot")}
                  title="Use On Foot defaults for new entries"
                >
                  <Footprints size={14} />
                  On Foot
                </button>
                <button
                  className={settings.context === "on_road" ? "active" : ""}
                  onClick={() => context("on_road")}
                  title="Use On Road defaults for new entries"
                >
                  <Car size={15} />
                  On Road
                </button>
              </div>
            </div>
            <span className="toolbar-separator" />
            <button
              className="icon-button notification-button"
              aria-label="Notifications"
              title="Notifications and reminders"
              onClick={() => setPanel("notifications")}
            >
              <Bell size={18} />
              {reminders.some((r) => r.state === "delivered") && <i />}
            </button>
            <button
              className="icon-button"
              aria-label="Application updates"
              title="Application updates"
              onClick={() => setPanel("updates")}
            >
              <Download size={18} />
              {updatesAvailable && <span className="update-dot" />}
            </button>
            <button
              className="icon-button"
              aria-label="Open settings"
              title="Settings"
              onClick={() => setPanel("settings")}
            >
              <Settings size={18} />
            </button>
          </div>
        </header>
        {error && (
          <div role="alert" className="error-banner global-error">
            <span>{error}</span>
            <button className="text-button" onClick={refresh}>
              <RefreshCw size={14} />
              Retry
            </button>
          </div>
        )}
        {notice && (
          <div role="status" className="toast">
            <Check size={16} />
            {notice}
            <button
              className="icon-button"
              aria-label="Dismiss message"
              onClick={() => setNotice("")}
            >
              <X size={14} />
            </button>
          </div>
        )}
        <div className="workspace-content">
          {(tab === "Calendar" || tab === "Budget") && (
            <div className="calendar-toolbar">
              <div className="month-title">
                <span className="eyebrow">
                  {tab === "Calendar"
                    ? "YOUR DAYS, BY DESIGN"
                    : "MAKE ROOM FOR WHAT MATTERS"}
                </span>
                <div>
                  <h1>
                    {dateOf(`${month}-01`).toLocaleDateString("en", {
                      month: "long",
                    })}
                    <span>{month.slice(0, 4)}</span>
                  </h1>
                  <div className="month-navigation">
                    <button
                      className="icon-button"
                      aria-label="Previous month"
                      onClick={() => changeMonth(-1)}
                    >
                      <ChevronLeft size={17} />
                    </button>
                    <button
                      className="icon-button"
                      aria-label="Next month"
                      onClick={() => changeMonth(1)}
                    >
                      <ChevronRight size={17} />
                    </button>
                  </div>
                </div>
              </div>
              <div className="calendar-toolbar-actions">
                <button
                  className="secondary import-button"
                  onClick={() => setPanel("imports")}
                >
                  <ArrowDownToLine size={15} />
                  Import booking
                  {drafts.length > 0 && (
                    <span className="count-badge">{drafts.length}</span>
                  )}
                </button>
                <div className="new-entry">
                  <button
                    className="primary"
                    onClick={() => setNewMenu(!newMenu)}
                    aria-expanded={newMenu}
                  >
                    <Plus size={17} />
                    New entry
                  </button>
                  {newMenu && (
                    <div className="entry-menu">
                      {[
                        ["stay", "Stay", Home],
                        ["transport", "Transport", Plane],
                        ["objective", "Objective", Flag],
                        ["expense", "Expense", Wallet],
                        ["payment", "Payment", CircleDollarSign],
                        ["reminder", "Reminder", Bell],
                        ["workBlock", "Work block", Clock],
                      ].map(([kind, label, Icon]) => {
                        const I = Icon as typeof Home;
                        return (
                          <button
                            key={String(kind)}
                            onClick={() => create(String(kind))}
                          >
                            <I size={16} />
                            {String(label)}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
          {tab === "Calendar" && (
            <>
              <div className="calendar-subtoolbar">
                <div className="segmented view-switch">
                  {["month", "week", "day", "journey"].map((v) => (
                    <button
                      key={v}
                      className={view === v ? "active" : ""}
                      onClick={() => setView(v)}
                    >
                      {v[0].toUpperCase() + v.slice(1)}
                    </button>
                  ))}
                </div>
                <span className="spacer" />
                <button
                  className="text-button muted export-button"
                  onClick={exportCalendar}
                >
                  <Download size={14} />
                  Export calendar
                </button>
                <button
                  className="today-button"
                  onClick={() => {
                    setMonth(today.slice(0, 7));
                    setSelectedDate(today);
                  }}
                >
                  Today
                </button>
              </div>
              <div className="calendar-layout">
                <section className="calendar-main">
                  <Calendar
                    records={calendarRecords.length ? calendarRecords : records}
                    month={month}
                    selectedDate={selectedDate}
                    view={view}
                    onSelect={select}
                    onDay={(d) => {
                      setSelectedDate(d);
                      if (
                        view !== "month" ||
                        window.matchMedia("(max-width:600px)").matches
                      )
                        setMonth(d.slice(0, 7));
                    }}
                    onCreate={create}
                  />
                </section>
                <aside className="day-rail">
                  <div className="rail-date">
                    <span className="eyebrow">A LITTLE PERSPECTIVE</span>
                    <h3>
                      {prettyDate(selectedDate, {
                        weekday: "long",
                        month: "short",
                        day: "numeric",
                      })}
                    </h3>
                  </div>
                  <Clocks
                    now={now}
                    localZone={currentZone}
                    officeZone={officeZone}
                    settings={settings}
                  />
                  <div className="rail-divider" />
                  <div className="rail-section">
                    <div className="rail-title">
                      <Clock size={15} />
                      <h3>Work, in local time</h3>
                    </div>
                    <WorkWindow
                      work={work}
                      stay={currentStay}
                      date={selectedDate}
                    />
                    <button
                      className="text-button"
                      onClick={() => create("workBlock")}
                    >
                      Protect a work block
                      <Plus size={13} />
                    </button>
                  </div>
                  <div className="rail-divider" />
                  <div className="rail-section">
                    <div className="rail-title">
                      <Wallet size={15} />
                      <h3>A little breathing room</h3>
                    </div>
                    <div className="rail-money">
                      {money(summary?.dailyAllowanceMinor, currency)}
                      <small>/ day</small>
                    </div>
                    <p>
                      {summary?.remainingDays ?? "—"} remaining eligible days
                      <br />
                      after committed and recorded spending
                    </p>
                    <button
                      className="text-button"
                      onClick={() => setTab("Budget")}
                    >
                      Open budget
                      <ArrowUpRight size={13} />
                    </button>
                  </div>
                  <div className="rail-divider" />
                  <GapNote
                    records={records}
                    date={selectedDate}
                    onCreate={() => create("stay")}
                  />
                </aside>
              </div>
            </>
          )}
          {tab === "Discover" && (
            <Discover
              records={records}
              settings={settings}
              onCreate={create}
              onSelect={select}
              onRefresh={refresh}
              onSettings={() => setPanel("settings")}
            />
          )}
          {tab === "Map" && (
            <MapView records={records} onCreate={create} onSelect={select} />
          )}
          {tab === "Budget" && (
            <Budget
              records={records}
              summary={summary}
              month={month}
              currency={currency}
              onCreate={create}
              onSelect={select}
              onEditBudget={() => {
                const record =
                  records.find(
                    (r) =>
                      r.kind === "budget" &&
                      r.month === month &&
                      r.currency === currency,
                  ) || newRecord("budget", `${month}-01`, settings);
                setSelected({ ...record, id: record.id || "", _edit: true });
              }}
            />
          )}
        </div>
        <footer className="workspace-footer">
          <span>
            Birthright: Atlas <span className="muted">/ 0.1.0</span>
          </span>
          <span>Your plans. Your pace.</span>
          <button
            className="assistant-launcher"
            onClick={() => setPanel("assistant")}
          >
            <Sparkles size={14} />
            Atlas Assistant<span>↗</span>
          </button>
        </footer>
      </main>
      {selected && (
        <RecordForm
          key={`${selected.id || selected.kind}-${importId || ""}-${selected._edit ? "edit" : ""}`}
          record={selected}
          records={records}
          onSaved={saveHandler}
          onClose={close}
          importId={importId}
        />
      )}
      {panel === "imports" && (
        <ImportPanel
          records={records}
          onClose={() => setPanel("")}
          onRefresh={refresh}
          onReview={(draft) => {
            const record = draft.record || draft.candidate || {};
            setImportId(draft.id);
            setSelected({
              ...newRecord(record.kind || "stay", selectedDate, settings),
              ...record,
              id: record.id || "",
            });
            setPanel("");
          }}
        />
      )}
      {panel === "settings" && (
        <SettingsPanel
          settings={settings}
          onClose={() => setPanel("")}
          onRefresh={refresh}
        />
      )}
      {panel === "assistant" && (
        <Assistant
          onClose={() => setPanel("")}
          records={records}
          onSelect={select}
          onRefresh={refresh}
        />
      )}
      {panel === "notifications" && (
        <SimplePanel
          title="Small things, remembered."
          eyebrow="NOTIFICATIONS & REMINDERS"
          onClose={() => setPanel("")}
        >
          <button className="primary full" onClick={() => create("reminder")}>
            <Plus size={16} />
            Create a reminder
          </button>
          {reminders.length === 0 ? (
            <div className="empty small">
              <Bell size={28} />
              <h3>Nothing to remember. Yet.</h3>
              <p>
                Set a reminder for checkout, a payment, or a cancellation
                deadline.
              </p>
            </div>
          ) : (
            reminders.map((r) => (
              <button
                className="reminder-card"
                key={r.id}
                onClick={() => select(r)}
              >
                <Bell size={17} />
                <div>
                  <strong>{r.name}</strong>
                  <small>
                    {new Date(r.dueAt).toLocaleString()} · {r.state}
                  </small>
                </div>
                <ChevronRight size={16} />
              </button>
            ))
          )}
          <p className="field-help">
            The native scheduler delivers while Atlas runs. Persisted reminders
            catch up on launch according to your chosen policy.
          </p>
        </SimplePanel>
      )}
      {panel === "updates" && <Updates onClose={() => setPanel("")} />}
    </div>
  );
}

function Clocks({
  now,
  localZone,
  officeZone,
  settings,
}: {
  now: Date;
  localZone: string;
  officeZone: string;
  settings: AtlasSettings;
}) {
  const format = (zone: string, options: Intl.DateTimeFormatOptions) => {
    try {
      const fixed =
        zone === officeZone && settings.officeTimezoneMode === "fixed";
      const offset = Number(settings.officeFixedOffsetMinutes || 0);
      const fixedLabel = `UTC${offset < 0 ? "−" : "+"}${String(Math.floor(Math.abs(offset) / 60)).padStart(2, "0")}:${String(Math.abs(offset) % 60).padStart(2, "0")}`;
      return new Intl.DateTimeFormat("en", {
        timeZone: fixed ? "UTC" : zone,
        ...options,
      })
        .format(fixed ? new Date(now.getTime() + offset * 60000) : now)
        .replace(fixed ? "GMT" : "__never__", fixedLabel);
    } catch {
      return "Invalid timezone";
    }
  };
  const hour = format(officeZone, {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  });
  const day = format(officeZone, { weekday: "short" });
  const workday = (settings.workdays || [1, 2, 3, 4, 5]).includes(
    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].indexOf(day) + 1,
  );
  const open =
    workday &&
    hour >= (settings.workStart || "08:00") &&
    hour < (settings.workEnd || "17:00");
  return (
    <div className="clocks">
      <div className="clock-line">
        <div>
          <small>
            {settings.currentLocation
              ? "YOUR CONFIRMED LOCATION"
              : "LOCAL CLOCK · LOCATION NOT SET"}
          </small>
          <strong>
            {format(localZone, {
              hour: "2-digit",
              minute: "2-digit",
              hour12: false,
            })}
          </strong>
          <span>
            {settings.currentLocation ||
              localZone.split("/").pop()?.replaceAll("_", " ")}
          </span>
        </div>
        <small>
          {format(localZone, {
            weekday: "short",
            day: "numeric",
            month: "short",
            timeZoneName: "shortOffset",
          })}
        </small>
      </div>
      <div className="clock-line office">
        <div>
          <small>EMPLOYER OFFICE</small>
          <strong>
            {format(officeZone, {
              hour: "2-digit",
              minute: "2-digit",
              hour12: false,
            })}
            <span className={`office-status ${open ? "open" : ""}`}>
              {open ? "Open" : "Closed"}
            </span>
          </strong>
          <span>
            {settings.officeTimezoneMode === "fixed"
              ? "Fixed office offset"
              : officeZone.split("/").pop()?.replaceAll("_", " ")}
          </span>
        </div>
        <small>
          {format(officeZone, {
            weekday: "short",
            day: "numeric",
            month: "short",
            timeZoneName: "shortOffset",
          })}
        </small>
      </div>
    </div>
  );
}
function WorkWindow({
  work,
  stay,
  date,
}: {
  work: any;
  stay?: AtlasRecord;
  date: string;
}) {
  const start = work?.localStart || work?.destinationStart || work?.officeStart;
  const end = work?.localEnd || work?.destinationEnd || work?.officeEnd;
  const window =
    work?.destinationWindow || work?.localWindow || work?.officeWindow;
  return (
    <>
      {work?.error && <p className="rail-warning">{work.error}</p>}
      <span className="rail-caption">
        {stay
          ? `Planned at ${stay.name}`
          : "Using your configured local timezone"}
      </span>
      {work?.isWorkday === false || work?.workday === false ? (
        <div className="work-hours">Office day off</div>
      ) : (
        <div className="work-hours">
          {start
            ? String(start).replace("T", " ").slice(0, 16)
            : window?.start
              ? String(window.start).replace("T", " ").slice(0, 16)
              : "Office window"}
          <span>
            {end
              ? String(end).replace("T", " ").slice(0, 16)
              : window?.end
                ? String(window.end).replace("T", " ").slice(0, 16)
                : `${work?.officeTimezone || "America/Los_Angeles"}`}
          </span>
        </div>
      )}
      <p>
        {work?.requiredHours ?? 8}h per employer workday ·{" "}
        {work?.minOverlapHours ?? 4}h minimum office overlap
      </p>
      {work?.conflicts?.length > 0 && (
        <ul className="work-conflicts">
          {work.conflicts.slice(0, 4).map((c: any, i: number) => (
            <li key={i}>
              {typeof c === "string" ? c : c.message || c.reason || c.type}
            </li>
          ))}
        </ul>
      )}
      {stay?.facilities?.internet !== true && (
        <p className="rail-warning">
          Connectivity at the planned workspace is unverified.
        </p>
      )}
    </>
  );
}
function GapNote({
  records,
  date,
  onCreate,
}: {
  records: AtlasRecord[];
  date: string;
  onCreate: () => void;
}) {
  const stay = records.some(
    (r) =>
      r.kind === "stay" &&
      r.status === "confirmed" &&
      r.checkIn <= date &&
      r.checkOut > date,
  );
  const transit = records.some((r) => travelNight(r, date));
  return (
    <div className="gap-note">
      <span className="eyebrow">TONIGHT’S PLAN</span>
      <h3>
        {stay
          ? "A place to land."
          : transit
            ? "A night on the move."
            : "An open night."}
      </h3>
      <p>
        {stay
          ? "Your confirmed stay covers this night."
          : transit
            ? "Marked as intentionally traveling overnight."
            : "No confirmed stay or overnight journey covers this date."}
      </p>
      {!stay && !transit && (
        <button className="text-button" onClick={onCreate}>
          Find your base
          <ArrowUpRight size={13} />
        </button>
      )}
    </div>
  );
}

function SimplePanel({
  title,
  eyebrow,
  onClose,
  children,
}: {
  title: string;
  eyebrow: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div
      className="drawer-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <aside
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-label={eyebrow}
      >
        <div className="drawer-top">
          <span className="eyebrow">{eyebrow}</span>
          <button
            className="icon-button"
            aria-label="Close panel"
            onClick={onClose}
          >
            <X size={20} />
          </button>
        </div>
        <div className="drawer-heading">
          <h2>{title}</h2>
        </div>
        <div className="detail-content">{children}</div>
      </aside>
    </div>
  );
}

function Assistant({
  onClose,
  records,
  onSelect,
  onRefresh,
}: {
  onClose: () => void;
  records: AtlasRecord[];
  onSelect: (r: AtlasRecord) => void;
  onRefresh: () => Promise<void>;
}) {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<
    {
      role: string;
      text: string;
      records?: AtlasRecord[];
      undo?: { method: string; payload: any };
    }[]
  >([]);
  const [busy, setBusy] = useState(false);
  async function ask(text: string) {
    setInput("");
    setMessages((m) => [...m, { role: "you", text }]);
    setBusy(true);
    try {
      const result = await rpc("assistant.ask", { text });
      setMessages((m) => [
        ...m,
        {
          role: "atlas",
          text: result.message,
          records: result.records,
          undo: result.undo,
        },
      ]);
      if (result.undo) await onRefresh();
    } catch (e) {
      setMessages((m) => [...m, { role: "atlas", text: String(e) }]);
    } finally {
      setBusy(false);
    }
  }
  return (
    <SimplePanel
      title="A little perspective."
      eyebrow="ATLAS ASSISTANT"
      onClose={onClose}
    >
      <div className="assistant-intro">
        <Sparkles size={25} />
        <p>
          Ask about your saved plans and exact budget. Answers use your local
          records; live availability requires a connected source.
        </p>
      </div>
      {messages.length === 0 && (
        <div className="assistant-suggestions">
          {[
            "What is my daily cost this month?",
            "Which stays have a verified workspace?",
            "Show my upcoming reminders",
            "What is on my calendar?",
          ].map((q) => (
            <button key={q} onClick={() => ask(q)}>
              {q}
              <ArrowUpRight size={13} />
            </button>
          ))}
        </div>
      )}
      <div className="assistant-messages">
        {messages.map((m, i) => (
          <article key={i} className={m.role}>
            <span>{m.role === "you" ? "YOU" : "ATLAS"}</span>
            <p>{m.text}</p>
            {m.records?.map((r) => (
              <button
                className="text-button"
                key={r.id}
                onClick={() =>
                  onSelect(records.find((x) => x.id === r.id) || r)
                }
              >
                {r.name}
                <ArrowRight size={13} />
              </button>
            ))}
            {m.undo && (
              <button
                className="secondary"
                onClick={async () => {
                  try {
                    await rpc(m.undo!.method, m.undo!.payload);
                    await onRefresh();
                    setMessages((previous) =>
                      previous.map((message, index) =>
                        index === i
                          ? {
                              ...message,
                              text:
                                message.text + "\nThis reminder was undone.",
                              records: [],
                              undo: undefined,
                            }
                          : message,
                      ),
                    );
                  } catch (e) {
                    setMessages((previous) => [
                      ...previous,
                      { role: "atlas", text: String(e) },
                    ]);
                  }
                }}
              >
                Undo reminder
              </button>
            )}
          </article>
        ))}
        {busy && <p className="muted">Looking at your records…</p>}
      </div>
      <form
        className="assistant-input"
        onSubmit={(e) => {
          e.preventDefault();
          if (input.trim()) void ask(input.trim());
        }}
      >
        <input
          aria-label="Ask Atlas"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Ask, or Remind me: name at 2026-12-01T09:00:00Z"
          disabled={busy}
        />
        <button
          className="icon-button"
          aria-label="Send question"
          disabled={busy || !input.trim()}
        >
          <Send size={17} />
        </button>
      </form>
    </SimplePanel>
  );
}

function Updates({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<any>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function check() {
    if (!isDesktop()) {
      setStatus({
        configured: false,
        reason:
          "Update checks are available in the installed desktop app. Web clients receive the host’s current version.",
      });
      return;
    }
    setBusy(true);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      setStatus(await invoke("update_status"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    void check();
  }, []);
  return (
    <SimplePanel
      title="Keep moving forward."
      eyebrow="APPLICATION UPDATES"
      onClose={onClose}
    >
      <div className="update-version">
        <span className="brand-symbol">A</span>
        <div>
          <strong>Birthright: Atlas</strong>
          <span>Version {status?.currentVersion || "0.1.0"}</span>
        </div>
      </div>
      {error && <div className="error-banner">{error}</div>}
      {status?.available ? (
        <>
          <h3>Version {status.version} is available.</h3>
          <p className="preserve-lines">{status.notes}</p>
          <p className="field-help">
            Save any open edits before updating. Atlas backs up saved data,
            verifies the update signature, installs, and reopens.
          </p>
          <button
            className="primary"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                const { invoke } = await import("@tauri-apps/api/core");
                await invoke("update_install");
              } catch (e) {
                setError(String(e));
                setBusy(false);
              }
            }}
          >
            <Download size={15} />
            {busy ? "Installing…" : "Update now"}
          </button>
        </>
      ) : (
        <p className="info-note">
          {busy
            ? "Checking for updates…"
            : status?.reason || "No update available."}
        </p>
      )}
      <div className="inline-actions">
        <button className="secondary" disabled={busy} onClick={check}>
          <RefreshCw size={14} />
          Check again
        </button>
        <button className="text-button" onClick={onClose}>
          Later
        </button>
      </div>
    </SimplePanel>
  );
}
