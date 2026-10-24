export type Module = "Calendar" | "Discover" | "Map" | "Budget" | "Settings";
export type AtlasTarget = {
  module?: Module;
  recordId?: string;
  date?: string;
  month?: string;
  settingsCategory?: string;
  action?: "imports" | "updates" | "work" | "readiness";
};

/** Targets are local, typed locations, never executable URLs or model commands. */
export function validTarget(target: AtlasTarget): boolean {
  return (
    !!target &&
    (!target.module ||
      ["Calendar", "Discover", "Map", "Budget", "Settings"].includes(
        target.module,
      )) &&
    (!target.date ||
      (/^\d{4}-\d{2}-\d{2}$/.test(target.date) &&
        !Number.isNaN(Date.parse(target.date)) &&
        new Date(target.date).toISOString().slice(0, 10) === target.date)) &&
    (!target.month || /^\d{4}-(0[1-9]|1[0-2])$/.test(target.month)) &&
    (!target.action || ["imports", "updates", "work", "readiness"].includes(target.action))
  );
}
