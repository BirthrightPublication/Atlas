export type AtlasRecord = {
  id: string;
  kind: string;
  version?: number;
  name?: string;
  [key: string]: any;
};
export type Settings = { [key: string]: any };
export const isDesktop = () => "__TAURI_INTERNALS__" in window;

export async function rpc<T = any>(
  method: string,
  payload: Record<string, any> = {},
): Promise<T> {
  if (isDesktop()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>("atlas_call", { method, payload });
  }
  const response = await fetch("/api/rpc", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ method, payload }),
  });
  const body = await response
    .json()
    .catch(() => ({
      error:
        "The Atlas service is not available. Start the local host and try again.",
    }));
  if (!response.ok || body.error)
    throw new Error(body.error || `Request failed (${response.status})`);
  return body as T;
}

export async function openExternal(value: string) {
  const url = new URL(value);
  if (!["https:", "http:"].includes(url.protocol))
    throw new Error("Only HTTPS and HTTP links can be opened.");
  if (isDesktop()) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_external", { url: url.href });
  } else window.open(url.href, "_blank", "noopener,noreferrer");
}

export function download(
  filename: string,
  content: string,
  type = "text/plain",
) {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function currencyDigits(currency = "USD") {
  try {
    return (
      new Intl.NumberFormat("en", {
        style: "currency",
        currency,
      }).resolvedOptions().maximumFractionDigits ?? 2
    );
  } catch {
    return 2;
  }
}
export function money(minor: number | undefined, currency = "USD") {
  if (minor === undefined || minor === null || !Number.isFinite(minor))
    return "—";
  try {
    return new Intl.NumberFormat("en", {
      style: "currency",
      currency,
      maximumFractionDigits: currencyDigits(currency),
    }).format(minor / 10 ** currencyDigits(currency));
  } catch {
    return `${minor / 100} ${currency}`;
  }
}
export function minorFromInput(value: string, currency = "USD") {
  value = value.trim();
  const digits = currencyDigits(currency);
  if (!/^-?\d+(\.\d*)?$/.test(value))
    throw new Error("Enter a valid monetary amount.");
  const [whole, fraction = ""] = value.split(".");
  if (fraction.length > digits)
    throw new Error(`${currency} supports ${digits} decimal places.`);
  const result = Number(
    BigInt(whole || "0") * BigInt(10 ** digits) +
      BigInt(fraction.padEnd(digits, "0") || "0") *
        (value.startsWith("-") ? -1n : 1n),
  );
  if (!Number.isSafeInteger(result))
    throw new Error("This amount is too large.");
  return result;
}
export const dayKey = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
export const dateOf = (key: string) => new Date(`${key.slice(0, 10)}T12:00:00`);
export const addDays = (key: string, amount: number) => {
  const date = dateOf(key);
  date.setDate(date.getDate() + amount);
  return dayKey(date);
};
export const nights = (start: string, end: string) =>
  Math.round(
    (Date.parse(`${end}T00:00:00Z`) - Date.parse(`${start}T00:00:00Z`)) /
      86400000,
  );
export const prettyDate = (
  key?: string,
  options?: Intl.DateTimeFormatOptions,
) =>
  key
    ? dateOf(key).toLocaleDateString(
        "en",
        options || { month: "short", day: "numeric" },
      )
    : "Unscheduled";
export function zonedDay(instant: string, timezone = "UTC") {
  try {
    const parts = new Intl.DateTimeFormat("en-CA", {
      timeZone: timezone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).formatToParts(new Date(instant));
    return ["year", "month", "day"]
      .map((type) => parts.find((p) => p.type === type)?.value)
      .join("-");
  } catch {
    return instant?.slice(0, 10) || "";
  }
}
export function zonedTime(instant: string, timezone = "UTC") {
  try {
    return new Intl.DateTimeFormat("en", {
      timeZone: timezone,
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
    }).format(new Date(instant));
  } catch {
    return "Unknown time";
  }
}
export function travelNight(record: AtlasRecord, day: string) {
  if (
    record.kind !== "transport" ||
    record.status !== "confirmed" ||
    !record.overnight
  )
    return false;
  const start = zonedDay(record.departure, record.departureTimezone);
  const end = zonedDay(record.arrival, record.arrivalTimezone);
  return day === start || (day > start && day < end);
}
