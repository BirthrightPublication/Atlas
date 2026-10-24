import { useState, useEffect, FormEvent } from "react";
import {
  X,
  Save,
  Trash2,
  ArrowUpRight,
  Clock,
  Link as LinkIcon,
  MapPin,
  Check,
  FileText,
} from "lucide-react";
import {
  AtlasRecord,
  Settings,
  addDays,
  currencyDigits,
  minorFromInput,
  money,
  nights,
  openExternal,
  prettyDate,
  rpc,
  zonedDay,
  zonedTime,
} from "./api";
import { DraftAnalysis } from "./Planning";

import { useDialog } from "./useDialog";

export function newRecord(
  kind: string,
  date: string,
  settings: Settings,
): AtlasRecord {
  const currency = settings.currency || "USD";
  const timezone =
    settings.currentTimezone ||
    Intl.DateTimeFormat().resolvedOptions().timeZone;
  const base: AtlasRecord = {
    id: "",
    kind,
    name: "",
    currency,
    timezone,
    context: settings.context || "on_foot",
    notes: "",
  };
  if (kind === "stay")
    return {
      ...base,
      type: "apartment",
      offerType: "stay",
      address: "",
      checkIn: date,
      checkOut: addDays(date, 7),
      checkInTime: "15:00",
      checkOutTime: "11:00",
      status: "tentative",
      totalMinor: 0,
      provider: "",
      reservationId: "",
      facilities: {
        refrigerator: null,
        workspace: null,
        kitchen: null,
        internet: null,
      },
    };
  if (kind === "transport")
    return {
      ...base,
      mode: settings.context === "on_road" ? "car" : "train",
      origin: "",
      destination: "",
      departure: `${date}T09:00:00Z`,
      arrival: `${date}T12:00:00Z`,
      departureTimezone: timezone,
      arrivalTimezone: timezone,
      overnight: false,
      status: "tentative",
      totalMinor: 0,
    };
  if (kind === "objective")
    return {
      ...base,
      category: "experience",
      date,
      startTime: "",
      endTime: "",
      status: "planned",
      totalMinor: 0,
    };
  if (kind === "payment")
    return {
      ...base,
      bookingId: "",
      date,
      type: "payment",
      status: "settled",
      amountMinor: 0,
    };
  if (kind === "expense")
    return { ...base, date, category: "groceries", amountMinor: 0 };
  if (kind === "reminder")
    return {
      ...base,
      dueAt: `${date}T09:00:00Z`,
      state: "pending",
      catchUp: "deliver",
    };
  if (kind === "quote")
    return {
      ...base,
      type: "apartment",
      address: "",
      checkIn: date,
      checkOut: addDays(date, 7),
      totalMinor: 0,
      status: "tentative",
      url: "",
      feesComplete: false,
      facilities: {
        refrigerator: null,
        workspace: null,
        kitchen: null,
        internet: null,
      },
    };
  if (kind === "place")
    return { ...base, address: "", url: "", category: "place" };
  if (kind === "workBlock")
    return {
      ...base,
      date,
      startTime: "09:00",
      endTime: "17:00",
      start: `${date}T09:00:00Z`,
      end: `${date}T17:00:00Z`,
    };
  if (kind === "budget")
    return {
      ...base,
      month: date.slice(0, 7),
      incomeMinor: 0,
      openingBalanceMinor: 0,
      reserveMinor: 0,
      allowanceMinor: 0,
      recurringExpenses: [],
    };
  return base;
}

export function RecordForm({
  record,
  records,
  onSaved,
  onClose,
  importId,
}: {
  record: AtlasRecord;
  records: AtlasRecord[];
  onSaved: (record?: AtlasRecord) => void;
  onClose: () => void;
  importId?: string;
}) {
  const dialog = useDialog(onClose);
  const [form, setForm] = useState<AtlasRecord>({ ...record });
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState(
    !record.id || !!importId || !!record._edit,
  );
  const [history, setHistory] = useState<any[] | null>(null);
  const [scenario, setScenario] = useState<any>(null);
  const [analysis, setAnalysis] = useState<any>(null);
  const [analysisError, setAnalysisError] = useState("");
  useEffect(() => {
    if (!["stay", "quote", "transport"].includes(form.kind)) return;
    let alive = true;
    const timer = setTimeout(() => {
      rpc("planning.preview", { record: form })
        .then((result) => {
          if (alive) {
            setAnalysis({ ...result, currency: form.currency });
            setAnalysisError("");
          }
        })
        .catch((e) => {
          if (alive) {
            setAnalysis(null);
            setAnalysisError(String(e));
          }
        });
    }, 200);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [form]);
  async function attach(file?: File) {
    if (!file) return;
    if (file.size > 20 * 1024 * 1024) {
      setError("Attachments must be at most 20 MiB");
      return;
    }
    setBusy(true);
    setError("");
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      let raw = "";
      for (let i = 0; i < bytes.length; i += 8192)
        raw += String.fromCharCode(...bytes.subarray(i, i + 8192));
      const result = await rpc("documents.attach", {
        filename: file.name,
        contentBase64: btoa(raw),
      });
      setForm((f) => ({
        ...f,
        documentIds: [...(f.documentIds || []), result.document.id],
      }));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function localInstant(field: string, value: string, zone: string) {
    if (!value) return;
    try {
      const result = await rpc("planning.instant", {
        date: value.slice(0, 10),
        time: value.slice(11, 16),
        timezone: zone,
      });
      set(field, result.instant);
      setError("");
    } catch (e) {
      setError(
        `${String(e)} Use the explicit instant field to disambiguate a repeated clock time.`,
      );
    }
  }
  function duplicate() {
    const draft: AtlasRecord = {
      ...record,
      id: "",
      name: `${record.name} (draft)`,
      status: "tentative",
      reservationId: "",
      sourceIds: [],
      _edit: true,
    };
    for (const key of [
      "version",
      "createdAt",
      "updatedAt",
      "paymentSchedule",
      "_importKey",
    ])
      delete draft[key];
    onSaved(draft);
  }
  const set = (key: string, value: any) =>
    setForm((f) => ({ ...f, [key]: value }));
  const field = (
    name: string,
    label: string,
    type = "text",
    required = false,
    placeholder = "",
  ) => (
    <label className="field" key={name}>
      <span>{label}</span>
      <input
        name={name}
        type={type}
        value={form[name] ?? ""}
        onChange={(e) =>
          set(
            name,
            type === "number" && e.target.value !== ""
              ? Number(e.target.value)
              : e.target.value,
          )
        }
        required={required}
        placeholder={placeholder}
      />
    </label>
  );
  const select = (name: string, label: string, options: [string, string][]) => (
    <label className="field" key={name}>
      <span>{label}</span>
      <select
        value={form[name] || ""}
        onChange={(e) => set(name, e.target.value)}
      >
        {options.map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </select>
    </label>
  );
  const amount = (name: string, label: string) => (
    <MoneyField
      key={name + form.currency}
      label={label}
      value={form[name]}
      currency={form.currency || "USD"}
      onChange={(v) => set(name, v)}
    />
  );
  const bookings = records.filter((r) =>
    ["stay", "transport"].includes(r.kind),
  );
  async function save(e: FormEvent) {
    e.preventDefault();
    setError("");
    setBusy(true);
    try {
      const clean = { ...form };
      if (!clean.id) delete (clean as any).id;
      delete clean._edit;
      if (
        clean.kind === "transport" &&
        (!/[zZ]|[+-]\d\d:\d\d$/.test(clean.departure) ||
          !/[zZ]|[+-]\d\d:\d\d$/.test(clean.arrival))
      )
        throw new Error(
          "Departure and arrival must include an offset (for example +01:00) or Z.",
        );
      const saved = importId
        ? await rpc("imports.accept", { id: importId, record: clean })
        : await rpc("records.save", {
            record: clean,
            expectedVersion: form.id ? record.version : undefined,
          });
      onSaved(saved);
      onClose();
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }
  async function remove() {
    setBusy(true);
    try {
      await rpc("records.delete", {
        id: record.id,
        expectedVersion: record.version,
      });
      onSaved();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  const facilities = [
    "refrigerator",
    "workspace",
    "kitchen",
    "internet",
    "quiet",
    "power",
    "private",
    "scenic",
    "utilitiesIncluded",
  ];
  const linkedStay = records.find((r) => r.id === record.stayId);
  return (
    <div
      className="drawer-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <aside
        className="drawer"
        ref={dialog}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label={record.name || `New ${record.kind}`}
      >
        <div className="drawer-top">
          <span className="eyebrow">
            {importId
              ? "REVIEW IMPORT"
              : record.id
                ? `${record.kind.toUpperCase()} DETAILS`
                : `NEW ${record.kind.toUpperCase()}`}
          </span>
          <button
            className="icon-button"
            aria-label="Close details"
            onClick={onClose}
          >
            <X size={20} />
          </button>
        </div>
        {editing ? (
          <form onSubmit={save}>
            <div className="drawer-heading">
              <h2>
                {importId
                  ? "Make it yours"
                  : record.id
                    ? "Edit your plan"
                    : "A new chapter"}
              </h2>
              <p>
                {importId
                  ? "Review every field before adding this record to your journey."
                  : "The details that make the journey work."}
              </p>
            </div>
            {error && (
              <div role="alert" className="error-banner">
                {error}
              </div>
            )}
            <div className="form-content">
              {form.kind !== "budget" &&
                field(
                  "name",
                  form.kind === "reminder" ? "Reminder" : "Name",
                  "text",
                  true,
                  "Give it a name",
                )}
              {form.kind === "quote" &&
                select("offerType", "Offer category", [
                  ["stay", "Accommodation"],
                  ["transport", "Transportation"],
                ])}
              {(form.kind === "stay" ||
                (form.kind === "quote" && form.offerType !== "transport")) && (
                <>
                  {!record.id && (
                    <label className="field">
                      <span>Reuse details from a saved stay (optional)</span>
                      <select
                        defaultValue=""
                        onChange={(e) => {
                          const previous = records.find(
                            (r) => r.id === e.target.value,
                          );
                          if (previous)
                            setForm((f) => ({
                              ...f,
                              name: previous.name,
                              type: previous.type,
                              address: previous.address,
                              timezone: previous.timezone,
                              provider: previous.provider,
                              facilities: { ...previous.facilities },
                              notes:
                                "Details copied for review; dates and reservation are unconfirmed.",
                              status: "tentative",
                            }));
                        }}
                      >
                        <option value="">Enter new accommodation</option>
                        {records
                          .filter((r) => r.kind === "stay")
                          .map((r) => (
                            <option key={r.id} value={r.id}>
                              {r.name}
                            </option>
                          ))}
                      </select>
                    </label>
                  )}
                  <div className="form-grid">
                    {select("type", "Accommodation", [
                      ["apartment", "Entire apartment"],
                      ["home", "Entire home"],
                      ["room", "Private room"],
                      ["hotel", "Hotel"],
                      ["hostel", "Hostel"],
                      ["vehicle", "Vehicle / nature"],
                    ])}
                    {select("context", "Travel context", [
                      ["on_foot", "On Foot"],
                      ["on_road", "On Road"],
                    ])}
                  </div>
                  {field("address", "Address")}
                  <div className="form-grid">
                    {field("checkIn", "Check-in", "date", true)}
                    {field("checkOut", "Check-out (exclusive)", "date", true)}
                  </div>
                  <div className="form-grid">
                    {field("checkInTime", "Check-in time", "time")}
                    {field("checkOutTime", "Check-out time", "time")}
                  </div>
                  {field(
                    "timezone",
                    "Local timezone",
                    "text",
                    true,
                    "Europe/Lisbon",
                  )}
                  <div className="form-grid">
                    {amount("totalMinor", "Full payable total")}
                    {field("currency", "Currency", "text", true, "USD")}
                  </div>
                  {select("status", "Booking status", [
                    ["tentative", "Tentative / alternative"],
                    ["confirmed", "Confirmed"],
                    ["cancelled", "Cancelled"],
                  ])}
                  <p className="field-help">
                    Payments are recorded separately. Tentative alternatives are
                    excluded from committed budgets.
                  </p>
                  <div className="form-grid">
                    {field("provider", "Provider")}
                    {field("reservationId", "Reservation ID")}
                  </div>
                  {field("url", "Listing / reservation URL", "url")}
                  <DraftAnalysis result={analysis} error={analysisError} />
                  <details className="record-section">
                    <summary>Facilities, evidence & booking details</summary>
                    {field("hostUrl", "Host conversation URL", "url")}
                    {field("contact", "Contact details")}
                    <h3 className="form-section">Facilities & evidence</h3>
                    <div className="form-grid">
                      {facilities.map((key) => (
                        <label className="field" key={key}>
                          <span>{key[0].toUpperCase() + key.slice(1)}</span>
                          <select
                            value={
                              form.facilities?.[key] === true
                                ? "yes"
                                : form.facilities?.[key] === false
                                  ? "no"
                                  : "unknown"
                            }
                            onChange={(e) =>
                              set("facilities", {
                                ...form.facilities,
                                [key]:
                                  e.target.value === "unknown"
                                    ? null
                                    : e.target.value === "yes",
                              })
                            }
                          >
                            <option value="unknown">
                              Unknown / not verified
                            </option>
                            <option value="yes">Verified available</option>
                            <option value="no">Not available</option>
                          </select>
                        </label>
                      ))}
                    </div>
                    {field("internetEvidence", "Internet reliability evidence")}
                    {field("noiseExpectations", "Noise expectations")}
                    {field("amenities", "Other amenities")}
                    {field("monthlyDiscount", "Monthly discount evidence")}
                    {field("taxesAndFees", "Taxes and mandatory fees evidence")}
                    {select("alternateWorkspaceId", "Alternate workspace", [
                      ["", "No alternate workspace"],
                      ...records
                        .filter((r) => r.kind === "place")
                        .map((r) => [r.id, r.name] as [string, string]),
                    ])}
                    {field(
                      "workspaceCommuteMinutes",
                      "Known workspace commute (minutes)",
                      "number",
                    )}
                    <div className="form-grid">
                      {field("latitude", "Latitude", "number")}
                      {field("longitude", "Longitude", "number")}
                    </div>
                    {field(
                      "cancellationAt",
                      "Cancellation deadline (ISO with offset)",
                      "text",
                      false,
                      "2026-12-01T18:00:00+01:00",
                    )}
                    {form.type === "vehicle" && (
                      <>
                        {field("landManager", "Land manager")}
                        {field("rulesUrl", "Official rules URL", "url")}
                        {select("overnightPermission", "Overnight permission", [
                          ["unknown", "Unknown"],
                          ["permitted", "Confirmed permitted"],
                          ["prohibited", "Prohibited"],
                        ])}
                        {field("rulesCheckedAt", "Rules checked", "date")}
                        {field("stayLimit", "Confirmed stay limit")}
                        {field(
                          "accessNotes",
                          "Road access / vehicle suitability",
                        )}
                        {field("fallbackPlace", "Fallback place")}
                      </>
                    )}
                    {form.kind === "quote" && (
                      <>
                        <label className="check-field">
                          <input
                            type="checkbox"
                            checked={!!form.feesComplete}
                            onChange={(e) =>
                              set("feesComplete", e.target.checked)
                            }
                          />{" "}
                          Mandatory fees and taxes included in this quoted total
                        </label>
                        {field("checkedAt", "Quote checked", "date")}
                        {field(
                          "stretchReason",
                          "Reason to consider an over-target offer",
                        )}
                      </>
                    )}
                  </details>
                </>
              )}
              {(form.kind === "transport" ||
                (form.kind === "quote" && form.offerType === "transport")) && (
                <>
                  <div className="form-grid">
                    {select("mode", "Mode", [
                      ["train", "Train"],
                      ["flight", "Flight"],
                      ["bus", "Bus"],
                      ["ferry", "Ferry"],
                      ["car", "Car"],
                      ["taxi", "Taxi / rideshare"],
                      ["rental", "Rental vehicle"],
                      ["walk", "Walking"],
                    ])}
                    {select("context", "Travel context", [
                      ["on_foot", "On Foot"],
                      ["on_road", "On Road"],
                    ])}
                  </div>
                  <div className="form-grid">
                    <label className="field">
                      <span>Origin from saved stay or segment</span>
                      <select
                        defaultValue=""
                        onChange={(e) => {
                          const r = records.find(
                            (r) => r.id === e.target.value,
                          );
                          if (r)
                            setForm((f) => ({
                              ...f,
                              origin:
                                r.kind === "stay" ? r.address : r.destination,
                              departureTimezone:
                                r.timezone || r.arrivalTimezone,
                              originStayId:
                                r.kind === "stay" ? r.id : undefined,
                            }));
                        }}
                      >
                        <option value="">Choose an editable suggestion</option>
                        {records
                          .filter(
                            (r) =>
                              r.kind === "stay" ||
                              (r.kind === "transport" &&
                                r.status === "confirmed"),
                          )
                          .map((r) => (
                            <option key={r.id} value={r.id}>
                              {r.name}
                            </option>
                          ))}
                      </select>
                    </label>
                    <label className="field">
                      <span>Destination stay</span>
                      <select
                        value={form.destinationStayId || ""}
                        onChange={(e) => {
                          const r = records.find(
                            (r) => r.id === e.target.value,
                          );
                          if (r)
                            setForm((f) => ({
                              ...f,
                              destination: r.address,
                              arrivalTimezone: r.timezone,
                              destinationStayId: r.id,
                            }));
                        }}
                      >
                        <option value="">No linked destination</option>
                        {records
                          .filter((r) => r.kind === "stay")
                          .map((r) => (
                            <option key={r.id} value={r.id}>
                              {r.name}
                            </option>
                          ))}
                      </select>
                    </label>
                    {field("origin", "Origin", "text", true)}
                    {field("destination", "Destination", "text", true)}
                  </div>
                  <div className="form-grid">
                    <label className="field">
                      <span>Departure local date/time</span>
                      <input
                        type="datetime-local"
                        value={`${zonedDay(form.departure, form.departureTimezone)}T${zonedTime(form.departure, form.departureTimezone)}`}
                        onChange={(e) =>
                          localInstant(
                            "departure",
                            e.target.value,
                            form.departureTimezone,
                          )
                        }
                      />
                    </label>
                    <label className="field">
                      <span>Arrival local date/time</span>
                      <input
                        type="datetime-local"
                        value={`${zonedDay(form.arrival, form.arrivalTimezone)}T${zonedTime(form.arrival, form.arrivalTimezone)}`}
                        onChange={(e) =>
                          localInstant(
                            "arrival",
                            e.target.value,
                            form.arrivalTimezone,
                          )
                        }
                      />
                    </label>
                  </div>
                  {field(
                    "departure",
                    "Departure instant (ISO + offset)",
                    "text",
                    true,
                    "2026-12-01T09:00:00+01:00",
                  )}
                  {field(
                    "departureTimezone",
                    "Origin timezone",
                    "text",
                    true,
                    "Europe/Lisbon",
                  )}
                  {field(
                    "arrival",
                    "Arrival instant (ISO + offset)",
                    "text",
                    true,
                    "2026-12-01T13:00:00+02:00",
                  )}
                  {field(
                    "arrivalTimezone",
                    "Destination timezone",
                    "text",
                    true,
                    "Europe/Athens",
                  )}
                  <p className="field-help">
                    Offsets preserve the actual instant across overnight travel
                    and date-line crossings. Local display follows the named
                    IANA timezones.
                  </p>
                  <label className="check-field">
                    <input
                      type="checkbox"
                      checked={!!form.overnight}
                      onChange={(e) => set("overnight", e.target.checked)}
                    />{" "}
                    This journey intentionally covers an accommodation night
                  </label>
                  <div className="form-grid">
                    {amount("totalMinor", "Full payable total")}
                    {field("currency", "Currency", "text", true)}
                  </div>
                  {select("status", "Booking status", [
                    ["tentative", "Tentative"],
                    ["confirmed", "Confirmed"],
                    ["cancelled", "Cancelled"],
                  ])}
                  <div className="form-grid">
                    {field("provider", "Operator")}
                    {field("reservationId", "Reservation ID")}
                  </div>
                  {field("url", "Booking URL", "url")}
                  <DraftAnalysis result={analysis} error={analysisError} />
                  <details className="record-section">
                    <summary>Transfer requirements & ticket details</summary>
                    {field(
                      "minimumTransferMinutes",
                      "Known minimum transfer (minutes)",
                      "number",
                    )}
                    {field(
                      "arrivalRequiredAt",
                      "Arrival required by (ISO with offset)",
                    )}
                    {field("baggage", "Baggage included")}
                    {field("transfers", "Connections / transfer requirements")}
                  </details>
                </>
              )}
              {form.kind === "objective" && (
                <>
                  {select("category", "Category", [
                    ["experience", "Experience"],
                    ["architecture", "Architecture"],
                    ["hike", "Hike"],
                    ["groceries", "Groceries"],
                    ["butcher", "Butcher"],
                    ["water", "Water refill"],
                    ["laundry", "Laundry"],
                    ["coworking", "Coworking"],
                    ["appointment", "Appointment"],
                  ])}
                  {select("stayId", "Linked stay", [
                    ["", "No linked stay"],
                    ...records
                      .filter((r) => r.kind === "stay")
                      .map((r) => [r.id, r.name || r.id] as [string, string]),
                  ])}
                  {field("date", "Date (leave blank for unscheduled)", "date")}
                  <div className="form-grid">
                    {field("startTime", "Start / earliest", "time")}
                    {field("endTime", "End / latest", "time")}
                  </div>
                  {field("timezone", "Local timezone", "text", true)}
                  {select("timing", "Timing", [
                    ["flexible", "Flexible window"],
                    ["fixed", "Fixed appointment"],
                  ])}
                  {select("recurrence", "Repeat", [
                    ["", "Does not repeat"],
                    ["daily", "Daily"],
                    ["weekly", "Weekly"],
                    ["monthly", "Monthly"],
                  ])}
                  {form.recurrence && (
                    <>
                      {field(
                        "recurrenceUntil",
                        "Repeat through (optional)",
                        "date",
                      )}
                      <label className="field">
                        <span>Skip dates (one per line)</span>
                        <textarea
                          rows={2}
                          value={(form.recurrenceExceptions || []).join("\n")}
                          onChange={(e) =>
                            set(
                              "recurrenceExceptions",
                              e.target.value
                                .split("\n")
                                .map((s) => s.trim())
                                .filter(Boolean),
                            )
                          }
                        />
                      </label>
                    </>
                  )}
                  {select("status", "Progress", [
                    ["planned", "Planned"],
                    ["done", "Completed"],
                  ])}
                  <div className="form-grid">
                    {amount("totalMinor", "Estimated spending")}
                    {field("currency", "Currency", "text", true)}
                  </div>
                  {field("address", "Place / address")}
                  {field("url", "Link", "url")}
                </>
              )}
              {form.kind === "payment" && (
                <>
                  {select("bookingId", "Booking obligation", [
                    ["", "Choose a booking"],
                    ...bookings.map(
                      (r) => [r.id, r.name || r.id] as [string, string],
                    ),
                  ])}
                  <div className="form-grid">
                    {field("date", "Payment date", "date", true)}
                    {select("type", "Payment type", [
                      ["payment", "Payment"],
                      ["deposit", "Refundable deposit"],
                      ["refund", "Refund"],
                      ["deposit_refund", "Deposit refund"],
                    ])}
                  </div>
                  <div className="form-grid">
                    {amount("amountMinor", "Amount (negative for refunds)")}
                    {field("currency", "Currency", "text", true)}
                  </div>
                  {select("status", "Settlement", [
                    ["planned", "Expected / scheduled"],
                    ["settled", "Actually settled"],
                  ])}
                  <p className="field-help">
                    Use the original booking currency unless recording an actual
                    settlement with its original conversion basis in notes. A
                    refundable deposit ties up cash without counting as consumed
                    travel cost.
                  </p>
                </>
              )}
              {form.kind === "expense" && (
                <>
                  <div className="form-grid">
                    {field("date", "Date", "date", true)}
                    {field("category", "Category", "text", true)}
                  </div>
                  <div className="form-grid">
                    {amount("amountMinor", "Actual spending")}
                    {field("currency", "Currency", "text", true)}
                  </div>
                  <p className="field-help">
                    Recorded flexible spending consumes the month’s allowance.
                  </p>
                </>
              )}
              {form.kind === "reminder" && (
                <>
                  {field(
                    "dueAt",
                    "Due instant (ISO with timezone offset)",
                    "text",
                    true,
                    "2026-12-01T09:00:00+01:00",
                  )}
                  {select("bookingId", "Related booking", [
                    ["", "General reminder"],
                    ...bookings.map(
                      (r) => [r.id, r.name || r.id] as [string, string],
                    ),
                  ])}
                  {select("catchUp", "If Atlas was closed", [
                    ["deliver", "Deliver when Atlas opens"],
                    ["skip", "Skip if overdue"],
                  ])}
                  <p className="field-help">
                    Reminders are saved locally. Desktop delivery requires Atlas
                    to be running; catch-up runs when it opens. Mobile calendar
                    export is available separately.
                  </p>
                </>
              )}
              {["place", "watchlist"].includes(form.kind) && (
                <>
                  {field("address", "Address")}
                  {field("category", "Category")}
                  {field("url", "Website", "url")}
                  <details className="record-section">
                    <summary>Known workspace hours</summary>
                    <p className="field-help">
                      Enter owner-verified venue hours in its recorded timezone.
                      Blank hours remain unknown.
                    </p>
                    {field("timezone", "Venue timezone")}
                    <div className="form-grid">
                      {(["start", "end"] as const).map((key) => (
                        <label className="field" key={key}>
                          <span>
                            {key === "start"
                              ? "Workspace opens"
                              : "Workspace closes"}
                          </span>
                          <input
                            type="time"
                            value={form.workspaceHours?.[key] || ""}
                            onChange={(e) =>
                              set("workspaceHours", {
                                ...form.workspaceHours,
                                [key]: e.target.value,
                              })
                            }
                          />
                        </label>
                      ))}
                    </div>
                    <div className="inline-actions">
                      {["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map(
                        (day, i) => (
                          <label key={day}>
                            <input
                              type="checkbox"
                              checked={(
                                form.workspaceHours?.weekdays || [
                                  1, 2, 3, 4, 5, 6, 7,
                                ]
                              ).includes(i + 1)}
                              onChange={(e) => {
                                const days = form.workspaceHours?.weekdays || [
                                  1, 2, 3, 4, 5, 6, 7,
                                ];
                                set("workspaceHours", {
                                  ...form.workspaceHours,
                                  weekdays: e.target.checked
                                    ? [...days, i + 1]
                                    : days.filter((n: number) => n !== i + 1),
                                });
                              }}
                            />
                            {day}
                          </label>
                        ),
                      )}
                    </div>
                    <button
                      type="button"
                      className="text-button"
                      onClick={() => set("workspaceHours", null)}
                    >
                      Clear unverified hours
                    </button>
                  </details>
                  <div className="form-grid">
                    {field("latitude", "Latitude", "number")}
                    {field("longitude", "Longitude", "number")}
                  </div>
                </>
              )}
              {form.kind === "workBlock" && (
                <>
                  {field("date", "Employer workday", "date", true)}
                  {field("start", "Start instant (ISO + offset)", "text", true)}
                  {field("end", "End instant (ISO + offset)", "text", true)}
                  {field("timezone", "Local timezone", "text", true)}
                </>
              )}
              {form.kind === "budget" && (
                <>
                  {field("month", "Month", "month", true)}
                  {field("currency", "Currency", "text", true)}
                  {amount("incomeMinor", "Take-home income")}
                  {amount("openingBalanceMinor", "Opening cash balance")}
                  {amount("reserveMinor", "Protected reserve")}
                  {amount("allowanceMinor", "Flexible spending allowance")}
                  <h3 className="form-section">Recurring commitments</h3>
                  {(form.recurringExpenses || []).map((r: any, i: number) => (
                    <div className="recurring-line" key={i}>
                      <input
                        aria-label={`Recurring expense ${i + 1} name`}
                        value={r.name}
                        placeholder="Insurance, internet…"
                        onChange={(e) =>
                          set(
                            "recurringExpenses",
                            form.recurringExpenses.map((x: any, j: number) =>
                              i === j ? { ...x, name: e.target.value } : x,
                            ),
                          )
                        }
                      />
                      <MoneyField
                        label="Amount"
                        value={r.amountMinor}
                        currency={form.currency}
                        onChange={(v) =>
                          set(
                            "recurringExpenses",
                            form.recurringExpenses.map((x: any, j: number) =>
                              i === j ? { ...x, amountMinor: v } : x,
                            ),
                          )
                        }
                      />
                      <button
                        type="button"
                        className="icon-button"
                        aria-label="Remove recurring expense"
                        onClick={() =>
                          set(
                            "recurringExpenses",
                            form.recurringExpenses.filter(
                              (_: any, j: number) => j !== i,
                            ),
                          )
                        }
                      >
                        <X size={15} />
                      </button>
                    </div>
                  ))}
                  <button
                    type="button"
                    className="text-button"
                    onClick={() =>
                      set("recurringExpenses", [
                        ...(form.recurringExpenses || []),
                        { name: "", amountMinor: 0 },
                      ])
                    }
                  >
                    + Add recurring commitment
                  </button>
                </>
              )}
              {["stay", "transport", "quote"].includes(form.kind) && (
                <label className="field">
                  <span>Attach booking document</span>
                  <input
                    type="file"
                    disabled={busy}
                    onChange={(e) => attach(e.target.files?.[0])}
                  />
                  <small>
                    {form.documentIds?.length || 0} linked documents. Files are
                    retained locally when you save; importing pasted
                    confirmation text is available from Import booking.
                  </small>
                </label>
              )}
              {["quote", "watchlist"].includes(form.kind) && (
                <details className="advanced-settings">
                  <summary>Offer evidence & watchlist details</summary>
                  {select("category", "Watchlist category", [
                    ["accommodation", "Accommodation"],
                    ["transport", "Transportation"],
                    ["destination", "Destination"],
                    ["experience", "Experience"],
                    ["resource", "Travel resource"],
                  ])}
                  {field("url", "Source URL", "url")}
                  {field("checkedAt", "Last checked date", "date")}
                  {field("provider", "Provider")}
                  {select("availability", "Availability", [
                    ["unknown", "Unknown"],
                    ["available", "Available at last check"],
                    ["unavailable", "Unavailable"],
                    ["stale", "Needs recheck"],
                  ])}
                  {field("availabilityEvidence", "Availability evidence")}
                  {field("cancellationPolicy", "Cancellation terms")}
                  {field(
                    "discountEvidence",
                    "Verified discount details (included in total)",
                  )}
                  {field("feesEvidence", "Fees / taxes evidence")}
                  {form.kind === "watchlist" && (
                    <>
                      {field("checkIn", "Travel start", "date")}
                      {field("checkOut", "Travel end", "date")}
                      {field("currency", "Currency")}
                      {amount("targetBudgetMinor", "Target budget")}
                    </>
                  )}
                  {form.offerType === "transport" && (
                    <>
                      {amount(
                        "baggageMinor",
                        "Baggage charges (blank = unknown)",
                      )}
                      {amount(
                        "transferMinor",
                        "Transfer costs (blank = unknown)",
                      )}
                      {amount(
                        "overnightMinor",
                        "Overnight accommodation (blank = unknown)",
                      )}
                      {field("baggage", "Baggage allowance")}
                      {field("connections", "Connections / transfers")}
                    </>
                  )}
                </details>
              )}
              {["stay", "transport"].includes(form.kind) && (
                <details className="advanced-settings">
                  <summary>
                    Booking instructions, deadlines & payment evidence
                  </summary>
                  {field("paymentDueAt", "Payment due instant (ISO + offset)")}
                  {field(
                    "cancellationAt",
                    "Cancellation deadline (ISO + offset)",
                  )}
                  {field("cancellationPolicy", "Cancellation terms")}
                  {field("instructions", "Check-in / ticket instructions")}
                  {field("contact", "Provider contact")}
                  {field("flightNumber", "Service / flight number")}
                  {field("terminal", "Terminal")}
                  {field("platform", "Platform")}
                  {field("baggage", "Baggage allowance")}
                </details>
              )}
              <label className="field">
                <span>Notes & source evidence</span>
                <textarea
                  rows={3}
                  value={form.notes || ""}
                  onChange={(e) => set("notes", e.target.value)}
                  placeholder="Useful details, verified information, and anything to check."
                />
              </label>
            </div>
            <div className="drawer-footer">
              <button type="button" className="secondary" onClick={onClose}>
                Cancel
              </button>
              <button type="submit" className="primary" disabled={busy}>
                <Save size={15} />
                {busy
                  ? "Saving…"
                  : importId
                    ? "Accept reviewed booking"
                    : "Save " + form.kind}
              </button>
            </div>
          </form>
        ) : (
          <>
            <div className="drawer-heading">
              <span
                className={`status-badge ${record.status === "confirmed" ? "confirmed" : ""}`}
              >
                {record.status || record.state || record.kind}
              </span>
              <h2>{record.name}</h2>
              <p>
                {record.address ||
                  record.category ||
                  `${record.origin || ""}${record.destination ? " → " + record.destination : ""}`}
              </p>
            </div>
            <div className="detail-content">
              {error && (
                <div role="alert" className="error-banner">
                  {error}
                </div>
              )}
              {record.kind === "stay" && (
                <>
                  <div className="detail-dates">
                    <div>
                      <span>CHECK IN</span>
                      <strong>{prettyDate(record.checkIn)}</strong>
                      <small>
                        {record.checkInTime} · {record.timezone}
                      </small>
                    </div>
                    <span className="date-divider" />
                    <div>
                      <span>CHECK OUT</span>
                      <strong>{prettyDate(record.checkOut)}</strong>
                      <small>
                        {record.checkOutTime} · {record.timezone}
                      </small>
                    </div>
                  </div>
                  <div className="detail-total">
                    <div>
                      <small>
                        {nights(record.checkIn, record.checkOut)} occupied
                        nights
                      </small>
                      <strong>
                        {money(record.totalMinor, record.currency)}
                      </strong>
                    </div>
                    <span>
                      {money(
                        record.totalMinor /
                          nights(record.checkIn, record.checkOut),
                        record.currency,
                      )}{" "}
                      / night
                    </span>
                  </div>
                  <h3>Make yourself at home</h3>
                  <div className="facility-list">
                    {facilities.map((f) => (
                      <div key={f}>
                        <span>{f}</span>
                        <strong
                          className={
                            record.facilities?.[f] === true ? "good" : "muted"
                          }
                        >
                          {record.facilities?.[f] === true
                            ? "Verified available"
                            : record.facilities?.[f] === false
                              ? "Not available"
                              : "Unknown"}
                        </strong>
                      </div>
                    ))}
                  </div>
                  <div className="detail-row">
                    <span>Travel context</span>
                    <strong>
                      {record.context === "on_road" ? "On Road" : "On Foot"}
                    </strong>
                  </div>
                  <div className="detail-row">
                    <span>Reservation</span>
                    <strong>{record.reservationId || "Not provided"}</strong>
                  </div>
                  <div className="detail-row">
                    <span>Provider</span>
                    <strong>{record.provider || "Manual entry"}</strong>
                  </div>
                  <button
                    className="secondary full"
                    onClick={() => {
                      setForm({
                        ...newRecord("payment", record.checkIn, {
                          currency: record.currency,
                        }),
                        bookingId: record.id,
                        name: `Payment for ${record.name}`,
                      });
                      setEditing(true);
                    }}
                  >
                    Record a payment
                  </button>
                  <button
                    className="text-button"
                    onClick={async () => {
                      try {
                        setScenario(
                          await rpc("planning.what_if", {
                            stayId: record.id,
                            additionalNights: 7,
                          }),
                        );
                        setError("");
                      } catch (e) {
                        setError(String(e));
                      }
                    }}
                  >
                    Explore one extra week
                    <ArrowUpRight size={14} />
                  </button>
                  {scenario && (
                    <div className="scenario-card">
                      <span className="eyebrow">WHAT IF · ESTIMATE ONLY</span>
                      <h3>Seven more nights.</h3>
                      <p>
                        {scenario.scenario.checkIn} →{" "}
                        {scenario.scenario.checkOut}
                      </p>
                      <div className="detail-row">
                        <span>Estimated additional cost</span>
                        <strong>
                          {money(
                            scenario.scenario.estimatedAdditionalMinor,
                            scenario.currency,
                          )}
                        </strong>
                      </div>
                      <div className="detail-row">
                        <span>Estimated full stay</span>
                        <strong>
                          {money(
                            scenario.scenario.estimatedTotalMinor,
                            scenario.currency,
                          )}
                        </strong>
                      </div>
                      <p className="field-help">
                        {scenario.basis?.description} A new provider quote is
                        required; your confirmed booking is unchanged.
                      </p>
                      {scenario.monthlyEstimates?.map((m: any) => (
                        <div className="detail-row" key={m.month}>
                          <span>{m.month} change</span>
                          <strong>
                            {money(m.deltaMinor, scenario.currency)}
                          </strong>
                        </div>
                      ))}
                      {scenario.conflicts?.map((c: any, i: number) => (
                        <p className="scenario-conflict" key={i}>
                          {c.message}
                        </p>
                      ))}
                    </div>
                  )}
                </>
              )}
              {record.kind === "transport" && (
                <>
                  <div className="detail-row">
                    <span>Departure</span>
                    <strong>
                      {record.departure}
                      <small>{record.departureTimezone}</small>
                    </strong>
                  </div>
                  <div className="detail-row">
                    <span>Arrival</span>
                    <strong>
                      {record.arrival}
                      <small>{record.arrivalTimezone}</small>
                    </strong>
                  </div>
                  <div className="detail-row">
                    <span>Fare</span>
                    <strong>{money(record.totalMinor, record.currency)}</strong>
                  </div>
                  {record.overnight && (
                    <p className="info-note">
                      This journey intentionally covers an accommodation night.
                    </p>
                  )}
                </>
              )}
              {record.kind === "objective" && (
                <>
                  <div className="detail-row">
                    <span>When</span>
                    <strong>
                      {prettyDate(record.date)} {record.startTime || "Flexible"}
                      {record.endTime ? " – " + record.endTime : ""}
                    </strong>
                  </div>
                  <div className="detail-row">
                    <span>Base</span>
                    <strong>{linkedStay?.name || "No stay linked"}</strong>
                  </div>
                  {record.address && (
                    <button
                      className="secondary full"
                      onClick={() =>
                        openExternal(
                          `https://www.google.com/maps/dir/?api=1&origin=${encodeURIComponent(linkedStay?.address || "")}&destination=${encodeURIComponent(record.address)}`,
                        )
                      }
                    >
                      <MapPin size={15} />
                      Plan route from stay
                      <ArrowUpRight size={14} />
                    </button>
                  )}
                  <button
                    className="primary full"
                    onClick={async () => {
                      try {
                        await rpc("records.save", {
                          record: {
                            ...record,
                            status:
                              record.status === "done" ? "planned" : "done",
                          },
                          expectedVersion: record.version,
                        });
                        onSaved();
                        onClose();
                      } catch (e) {
                        setError(String(e));
                      }
                    }}
                  >
                    <Check size={15} />
                    {record.status === "done"
                      ? "Mark as planned"
                      : "Mark complete"}
                  </button>
                </>
              )}
              {["payment", "expense"].includes(record.kind) && (
                <>
                  <div className="detail-total">
                    <strong>
                      {money(record.amountMinor, record.currency)}
                    </strong>
                    <span>{prettyDate(record.date)}</span>
                  </div>
                  <div className="detail-row">
                    <span>Type</span>
                    <strong>{record.type || record.category}</strong>
                  </div>
                </>
              )}
              {record.kind === "reminder" && (
                <>
                  <div className="detail-row">
                    <span>Due</span>
                    <strong>{new Date(record.dueAt).toLocaleString()}</strong>
                  </div>
                  <div className="inline-actions">
                    <button
                      className="secondary"
                      onClick={async () => {
                        try {
                          await rpc("reminders.snooze", {
                            id: record.id,
                            minutes: 30,
                          });
                          onSaved();
                          onClose();
                        } catch (e) {
                          setError(String(e));
                        }
                      }}
                    >
                      <Clock size={15} />
                      Snooze 30 min
                    </button>
                    <button
                      className="secondary"
                      onClick={async () => {
                        try {
                          await rpc("reminders.dismiss", { id: record.id });
                          onSaved();
                          onClose();
                        } catch (e) {
                          setError(String(e));
                        }
                      }}
                    >
                      Dismiss
                    </button>
                  </div>
                </>
              )}
              {record.notes && (
                <>
                  <h3>Notes</h3>
                  <p className="preserve-lines">{record.notes}</p>
                </>
              )}
              {(record.url || record.hostUrl) && (
                <div className="detail-links">
                  {record.url && (
                    <button
                      className="text-button"
                      onClick={() => openExternal(record.url)}
                    >
                      <LinkIcon size={14} />
                      Open provider / source
                      <ArrowUpRight size={14} />
                    </button>
                  )}
                  {record.hostUrl && (
                    <button
                      className="text-button"
                      onClick={() => openExternal(record.hostUrl)}
                    >
                      Host conversation
                      <ArrowUpRight size={14} />
                    </button>
                  )}
                </div>
              )}
              {record.documentIds?.length > 0 && (
                <div className="detail-links">
                  {record.documentIds.map((id: string) => (
                    <button
                      className="text-button"
                      key={id}
                      onClick={async () => {
                        try {
                          const doc = await rpc("documents.read", { id });
                          const bytes = Uint8Array.from(
                            atob(doc.contentBase64),
                            (c) => c.charCodeAt(0),
                          );
                          const href = URL.createObjectURL(
                            new Blob([bytes], {
                              type: doc.mimeType || "application/octet-stream",
                            }),
                          );
                          const a = document.createElement("a");
                          a.href = href;
                          a.download = doc.filename;
                          a.click();
                          setTimeout(() => URL.revokeObjectURL(href), 1000);
                        } catch (e) {
                          setError(String(e));
                        }
                      }}
                    >
                      <FileText size={15} />
                      {records.find((r) => r.id === id)?.name ||
                        "Download local attachment"}
                    </button>
                  ))}
                </div>
              )}
              <button
                className="text-button muted"
                onClick={async () => {
                  try {
                    setHistory(await rpc("records.history", { id: record.id }));
                  } catch (e) {
                    setError(String(e));
                  }
                }}
              >
                View change history
              </button>
              {history && (
                <div className="history-list">
                  {history.length === 0 ? (
                    <p>No earlier revisions.</p>
                  ) : (
                    history.map((h, i) => (
                      <div key={i}>
                        <strong>{h.action || "Updated"}</strong>
                        <small>
                          {h.createdAt ||
                            h.at ||
                            h.timestamp ||
                            "Stored revision"}{" "}
                          · version {h.version || i + 1}
                        </small>
                      </div>
                    ))
                  )}
                </div>
              )}
            </div>
            <div className="drawer-footer">
              <DeleteButton onDelete={remove} busy={busy} />
              {["stay", "transport"].includes(record.kind) && (
                <button className="secondary" onClick={duplicate}>
                  Duplicate as draft
                </button>
              )}
              <button className="primary" onClick={() => setEditing(true)}>
                Edit {record.kind}
              </button>
            </div>
          </>
        )}
      </aside>
    </div>
  );
}

function DeleteButton({
  onDelete,
  busy,
}: {
  onDelete: () => void;
  busy: boolean;
}) {
  const [confirm, setConfirm] = useState(false);
  return (
    <button
      className={`text-button ${confirm ? "danger" : ""}`}
      disabled={busy}
      onClick={() => (confirm ? onDelete() : setConfirm(true))}
    >
      <Trash2 size={15} />
      {confirm ? "Confirm delete" : "Delete"}
    </button>
  );
}
export function MoneyField({
  label,
  value,
  currency,
  onChange,
}: {
  label: string;
  value: number | null | undefined;
  currency: string;
  onChange: (n: number) => void;
}) {
  const [text, setText] = useState(
    value === null || value === undefined
      ? ""
      : String(value / 10 ** currencyDigits(currency)),
  );
  const [error, setError] = useState("");
  return (
    <label className="field">
      <span>{label}</span>
      <div className="money-input">
        <span>{currency}</span>
        <input
          inputMode="decimal"
          value={text}
          required
          aria-label={label}
          onChange={(e) => {
            setText(e.target.value);
            try {
              onChange(minorFromInput(e.target.value, currency));
              setError("");
              e.target.setCustomValidity("");
            } catch (err) {
              const message = String(err instanceof Error ? err.message : err);
              setError(message);
              e.target.setCustomValidity(message);
            }
          }}
        />
      </div>
      {error && <small className="danger">{error}</small>}
    </label>
  );
}
