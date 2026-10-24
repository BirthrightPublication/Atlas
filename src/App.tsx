import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
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
import { Readiness } from "./Readiness";
import { ImportPanel } from "./Imports";
import { Discover, MapView } from "./Discover";
import { Budget } from "./Budget";
import { SettingsPanel, SettingsActions } from "./SettingsPanel";
import { RightPanel, Utility } from "./RightPanel";
const AssistantPanel = lazy(() =>
  import("./AssistantPanel").then((m) => ({ default: m.AssistantPanel })),
);
const NotificationsPanel = lazy(() =>
  import("./NotificationsPanel").then((m) => ({
    default: m.NotificationsPanel,
  })),
);
const UpdatesPanel = lazy(() =>
  import("./UpdatesPanel").then((m) => ({ default: m.UpdatesPanel })),
);
import { GapReview } from "./Planning";
import { AtlasTarget, Module, validTarget } from "./navigation";
import { useDialog } from "./useDialog";
import { APP_VERSION } from "./version";
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
  const [tab, setTab] = useState<Module>("Calendar");
  const workspace = useRef<HTMLDivElement>(null);
  useEffect(() => {
    workspace.current?.scrollTo(0, 0);
  }, [tab]);
  const [month, setMonth] = useState(today.slice(0, 7));
  const [selectedDate, setSelectedDate] = useState(today);
  const [view, setView] = useState("month");
  const [panel, setPanel] = useState("");
  const [utility, setUtility] = useState<Utility | null>(null);
  const [visitedPanels, setVisitedPanels] = useState<Utility[]>([]);
  const [settingsVisited, setSettingsVisited] = useState(false);
  const [settingsDirty, setSettingsDirty] = useState(false);
  const settingsController = useRef<SettingsActions | null>(null);
  const [pendingTarget, setPendingTarget] = useState<AtlasTarget | null>(null);
  const [contextRecordId, setContextRecordId] = useState<string | undefined>();
  const [notificationData, setNotificationData] = useState<any>({
    items: [],
    unread: 0,
  });
  const [updateStatus, setUpdateStatus] = useState<any>(null);
  const toggleUtility = (value: Utility) => {
    setUtility((old) => (old === value ? null : value));
    setVisitedPanels((old) => (old.includes(value) ? old : [...old, value]));
  };
  const openUtility = (value: Utility) => {
    setUtility(value);
    setVisitedPanels((old) => (old.includes(value) ? old : [...old, value]));
  };
  const [settingsCategory, setSettingsCategory] = useState("General");
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
        setUtility(null);
        setNewMenu(false);
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
        if (!disposed) {
          setUpdatesAvailable(!!result.available);
          setUpdateStatus(result);
        }
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
  const refreshNotifications = useCallback(async () => {
    try {
      setNotificationData(await rpc("notifications.list"));
    } catch (e) {
      setError(String(e));
    }
  }, []);
  useEffect(() => {
    if (!loaded) return;
    void refreshNotifications();
    const timer = setInterval(refreshNotifications, 60000);
    return () => clearInterval(timer);
  }, [loaded, records, refreshNotifications]);
  const handleUpdateStatus = useCallback(
    (status: any) => {
      setUpdateStatus(status);
      setUpdatesAvailable(!!status.available);
      void refreshNotifications();
    },
    [refreshNotifications],
  );
  function navigate(target: AtlasTarget, bypassGuard = false): boolean {
    if (!validTarget(target)) {
      setNotice("This Atlas location is invalid.");
      return false;
    }
    if (
      settingsDirty &&
      !bypassGuard &&
      (target.module !== "Settings" || target.recordId || target.action)
    ) {
      setPendingTarget(target);
      return false;
    }
    const record = target.recordId
      ? records.find((r) => r.id === target.recordId)
      : undefined;
    if (target.recordId && !record) {
      setNotice("This record has been deleted or is no longer available.");
      return false;
    }
    if (target.module) {
      setTab(target.module);
      setPanel("");
    }
    if (target.module === "Settings") {
      setSettingsVisited(true);
      setSettingsCategory(target.settingsCategory || "General");
    }
    if (target.month) setMonth(target.month);
    const date =
      target.date ||
      record?.checkIn ||
      record?.date ||
      (record?.departure ? record.departure.slice(0, 10) : undefined);
    if (date) {
      setSelectedDate(date);
      if (!target.month) setMonth(date.slice(0, 7));
    }
    if (target.action === "updates") {
      openUtility("updates");
      return true;
    }
    if (record?.kind === "draft") {
      const candidate = record.record || record.candidate || {};
      setImportId(record.id);
      setSelected({
        ...newRecord(candidate.kind || "stay", date || selectedDate, settings),
        ...candidate,
        id: candidate.id || "",
      });
    } else if (record) {
      setImportId(undefined);
      setSelected(record);
      setContextRecordId(record.id);
    } else if (target.action === "imports") setPanel("imports");
    else if (target.action === "readiness") setPanel("readiness");
    if (target.action === "work") {
      setView("day");
      requestAnimationFrame(() => {
        const details = document.querySelector<HTMLDetailsElement>(
          ".day-context details",
        );
        if (details) {
          details.open = true;
          details.scrollIntoView({ block: "nearest" });
        }
      });
    }
    setNewMenu(false);
    setUtility(null);
    return true;
  }
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
  const create = (
    kind: string,
    date = selectedDate,
    defaults: Partial<AtlasRecord> = {},
  ) => {
    setImportId(undefined);
    setSelected({ ...newRecord(kind, date, settings), ...defaults });
    setPanel("");
    setNewMenu(false);
  };
  const select = (r: AtlasRecord) => {
    setSelected(records.find((x) => x.id === r.id) || r);
    setContextRecordId(r.id);
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
      <main className="main-shell">
        <header className="topbar">
          <div className="header-identity">
            <button
              className="header-brand"
              onClick={() => navigate({ module: "Calendar" })}
              aria-label="Birthright: Atlas home"
              title="Birthright: Atlas home"
            >
              <span>Birthright:</span> Atlas
            </button>
            <span
              className="workspace-status"
              data-connection={loaded ? "ready" : "connecting"}
              title={loaded ? "Local workspace" : "Connecting to local service"}
            >
              <span className={`status-dot ${loaded ? "online" : ""}`} />
              <span>{loaded ? "Local workspace" : "Connecting…"}</span>
            </span>
          </div>
          <div className="topbar-actions">
            <div className="context-control">
              <div className="context-toggle">
                <button
                  className={
                    (settings.context || "on_foot") === "on_foot"
                      ? "active"
                      : ""
                  }
                  onClick={() => context("on_foot")}
                  title="Use On Foot defaults for new entries"
                  aria-pressed={(settings.context || "on_foot") === "on_foot"}
                >
                  <Footprints size={14} />
                  On Foot
                </button>
                <button
                  className={settings.context === "on_road" ? "active" : ""}
                  onClick={() => context("on_road")}
                  title="Use On Road defaults for new entries"
                  aria-pressed={settings.context === "on_road"}
                >
                  <Car size={15} />
                  On Road
                </button>
              </div>
            </div>
            <button
              className="icon-button notification-button"
              aria-label="Notifications"
              title="Notifications and reminders"
              aria-expanded={utility === "notifications"}
              aria-controls="utility-panel"
              onClick={() => toggleUtility("notifications")}
            >
              <Bell size={18} />
              {notificationData.unread > 0 && <i />}
            </button>
            <button
              className="icon-button"
              aria-label="Application updates"
              title="Application updates"
              aria-expanded={utility === "updates"}
              aria-controls="utility-panel"
              onClick={() => toggleUtility("updates")}
            >
              <Download size={18} />
              {updatesAvailable && <span className="update-dot" />}
            </button>
            <button
              className="icon-button"
              aria-label="Atlas Assistant"
              title="Atlas Assistant"
              aria-expanded={utility === "assistant"}
              aria-controls="utility-panel"
              onClick={() => toggleUtility("assistant")}
            >
              <Sparkles size={18} />
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
        <div className={`workspace-layout ${utility ? "utility-open" : ""}`}>
          <div className="workspace-content" ref={workspace}>
            {panel === "readiness" && (
              <Readiness
                date={selectedDate}
                records={records}
                onClose={() => setPanel("")}
                onNavigate={navigate}
                onCreate={create}
                onRefresh={refresh}
              />
            )}
            {panel === "imports" && (
              <ImportPanel
                onConnections={() =>
                  navigate({
                    module: "Settings",
                    settingsCategory: "Email & Integrations",
                  })
                }
                onSelect={select}
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
                  setPanel("imports");
                }}
              />
            )}

            <div hidden={panel === "imports" || panel === "readiness"}>
              {(tab === "Calendar" || tab === "Budget") && (
                <div className="calendar-toolbar">
                  <div className="month-title">
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
                          title="Previous month"
                          aria-label="Previous month"
                          onClick={() => changeMonth(-1)}
                        >
                          <ChevronLeft size={17} />
                        </button>
                        <button
                          className="icon-button"
                          title="Next month"
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
                      title="Import a booking or review the import inbox"
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
                        aria-controls="entry-menu"
                      >
                        <Plus size={17} />
                        New entry
                      </button>
                      {newMenu && (
                        <div
                          className="entry-menu"
                          id="entry-menu"
                          aria-label="Create a record"
                        >
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
                    <button
                      className="text-button"
                      onClick={() => setPanel("readiness")}
                    >
                      Trip Readiness
                    </button>
                    <div className="segmented view-switch">
                      {["month", "week", "day", "journey"].map((v) => (
                        <button
                          key={v}
                          className={view === v ? "active" : ""}
                          aria-pressed={view === v}
                          onClick={() => setView(v)}
                        >
                          {v[0].toUpperCase() + v.slice(1)}
                        </button>
                      ))}
                    </div>
                    <span className="spacer" />
                    <button
                      className="text-button muted export-button"
                      title="Export calendar"
                      aria-label="Export calendar"
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
                        records={
                          calendarRecords.length ? calendarRecords : records
                        }
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
                  </div>
                  <GapReview
                    records={records}
                    month={month}
                    journey={view === "journey"}
                    onCreate={create}
                    onNavigate={navigate}
                  />
                  <section
                    className="day-context"
                    aria-label="Selected day information"
                  >
                    <details className="context-section">
                      <summary>
                        <Clock size={16} />
                        <span>Clocks & work</span>
                        <small>
                          {work?.isWorkday === false
                            ? "Office day off"
                            : `${work?.requiredHours ?? 8}h work · ${work?.minOverlapHours ?? 4}h overlap`}
                        </small>
                        <ChevronRight size={14} />
                      </summary>
                      <div className="context-details">
                        <Clocks
                          localZone={currentZone}
                          officeZone={officeZone}
                          settings={settings}
                        />
                        <div className="rail-section">
                          <h3>{prettyDate(selectedDate)} · Work schedule</h3>
                          <WorkWindow
                            work={work}
                            stay={currentStay}
                            date={selectedDate}
                          />
                          {work?.suggestedBlocks?.map(
                            (block: any, index: number) => (
                              <button
                                className="secondary"
                                key={index}
                                onClick={() =>
                                  create("workBlock", selectedDate, {
                                    name: "Proposed work block",
                                    start: block.start,
                                    end: block.end,
                                    startTime: block.localStart.slice(11, 16),
                                    endTime: block.localEnd.slice(11, 16),
                                    timezone:
                                      currentStay?.timezone || currentZone,
                                  })
                                }
                              >
                                Draft work: {block.localStart.slice(11, 16)}–
                                {block.localEnd.slice(11, 16)} local
                              </button>
                            ),
                          )}
                          <button
                            className="text-button"
                            onClick={() => create("workBlock")}
                          >
                            Protect a work block <Plus size={14} />
                          </button>
                        </div>
                      </div>
                    </details>
                    <details className="context-section">
                      <summary>
                        <Wallet size={16} />
                        <span>Daily allowance</span>
                        <small>
                          {money(summary?.dailyAllowanceMinor, currency)} / day
                        </small>
                        <ChevronRight size={14} />
                      </summary>
                      <div className="context-details compact">
                        <p>
                          {summary?.remainingDays ?? "—"} remaining eligible
                          days after committed and recorded spending.
                        </p>
                        <button
                          className="text-button"
                          onClick={() => setTab("Budget")}
                        >
                          Open budget <ArrowUpRight size={14} />
                        </button>
                      </div>
                    </details>
                    <details className="context-section">
                      <summary>
                        <Home size={16} />
                        <span>Accommodation</span>
                        <small>
                          {currentStay
                            ? currentStay.name
                            : records.some((r) => travelNight(r, selectedDate))
                              ? "Traveling overnight"
                              : "No confirmed stay"}
                        </small>
                        <ChevronRight size={14} />
                      </summary>
                      <div className="context-details compact">
                        <GapNote
                          records={records}
                          date={selectedDate}
                          onCreate={() => create("stay")}
                        />
                        {currentStay && (
                          <button
                            className="text-button"
                            onClick={() => select(currentStay)}
                          >
                            View {currentStay.name} <ArrowUpRight size={14} />
                          </button>
                        )}
                      </div>
                    </details>
                  </section>
                </>
              )}
              {tab === "Discover" && (
                <Discover
                  onImports={() => setPanel("imports")}
                  records={records}
                  settings={settings}
                  onCreate={create}
                  onSelect={select}
                  onRefresh={refresh}
                  onSettings={() =>
                    navigate({
                      module: "Settings",
                      settingsCategory: "Travel Preferences",
                    })
                  }
                />
              )}
              {tab === "Map" && (
                <MapView
                  onRefresh={refresh}
                  records={records}
                  onCreate={create}
                  onSelect={select}
                />
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
                    setSelected({
                      ...record,
                      id: record.id || "",
                      _edit: true,
                    });
                  }}
                />
              )}
              {settingsVisited && (
                <div hidden={tab !== "Settings"}>
                  <SettingsPanel
                    onImports={() => navigate({ action: "imports" })}
                    settings={settings}
                    initialCategory={settingsCategory}
                    updates={
                      <button
                        className="secondary"
                        onClick={() => openUtility("updates")}
                      >
                        Open Updates panel
                      </button>
                    }
                    onNotifications={() => openUtility("notifications")}
                    onBudget={() => navigate({ module: "Budget" })}
                    controller={settingsController}
                    onDirtyChange={setSettingsDirty}
                    onRefresh={refresh}
                  />
                </div>
              )}
            </div>
          </div>
          <RightPanel active={utility} onClose={() => setUtility(null)}>
            <Suspense fallback={<p>Opening panel…</p>}>
              {visitedPanels.includes("assistant") && (
                <div hidden={utility !== "assistant"}>
                  <AssistantPanel
                    module={tab}
                    context={{
                      date: selectedDate,
                      month,
                      recordId: contextRecordId,
                    }}
                    records={records}
                    onNavigate={navigate}
                    onRefresh={refresh}
                  />
                </div>
              )}
              {visitedPanels.includes("notifications") && (
                <div hidden={utility !== "notifications"}>
                  <NotificationsPanel
                    data={notificationData}
                    onRefresh={refreshNotifications}
                    onNavigate={navigate}
                    onCreate={() => create("reminder")}
                  />
                </div>
              )}
              {visitedPanels.includes("updates") && (
                <div hidden={utility !== "updates"}>
                  <UpdatesPanel
                    hasUnsaved={settingsDirty || !!selected}
                    onStatus={handleUpdateStatus}
                  />
                </div>
              )}
            </Suspense>
          </RightPanel>
        </div>
        <nav className="bottom-nav" aria-label="Main navigation">
          <div className="destination-nav">
            {navigation.map(([label, Icon]) => (
              <button
                key={label}
                aria-current={tab === label ? "page" : undefined}
                className={tab === label ? "active" : ""}
                title={label}
                onClick={() => {
                  if (settingsDirty) navigate({ module: label });
                  else {
                    setTab(label);
                    setNewMenu(false);
                  }
                }}
              >
                <Icon size={19} strokeWidth={1.7} />
                <span>{label}</span>
              </button>
            ))}
          </div>
          <button
            className={`settings-gear ${tab === "Settings" ? "active" : ""}`}
            aria-current={tab === "Settings" ? "page" : undefined}
            aria-label="Settings"
            title="Settings"
            onClick={() => {
              setSettingsVisited(true);
              setSettingsCategory("General");
              setTab("Settings");
            }}
          >
            <Settings size={20} strokeWidth={1.7} />
          </button>
        </nav>
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
      {pendingTarget && (
        <section
          className="unsaved-navigation"
          role="alert"
          aria-label="Unsaved settings"
        >
          <h3>Save your settings before leaving?</h3>
          <p>Your edits are retained until you choose.</p>
          <div className="inline-actions">
            <button
              className="primary"
              onClick={async () => {
                if (await settingsController.current?.save()) {
                  const target = pendingTarget;
                  setPendingTarget(null);
                  navigate(target, true);
                }
              }}
            >
              Save and continue
            </button>
            <button
              className="secondary"
              onClick={() => {
                settingsController.current?.discard();
                const target = pendingTarget;
                setPendingTarget(null);
                navigate(target, true);
              }}
            >
              Discard and continue
            </button>
            <button
              className="text-button"
              onClick={() => setPendingTarget(null)}
            >
              Keep editing
            </button>
          </div>
        </section>
      )}
    </div>
  );
}

function Clocks({
  localZone,
  officeZone,
  settings,
}: {
  localZone: string;
  officeZone: string;
  settings: AtlasSettings;
}) {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 30000);
    return () => clearInterval(timer);
  }, []);
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
