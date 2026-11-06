import { DAY, MINUTE, sqliteTime } from "./time";

export const DAYS = 182;
export const RANGES = [7, 30, 90] as const;

export type Range = (typeof RANGES)[number];

export type DayCounts = { date: string; created: number; completed: number; reopened: number; deleted: number };

export type Day = { date: string; added: number; completed: number; open: number };

function isoDate(ms: number) {
  return new Date(ms).toISOString().slice(0, 10);
}

export function addDays(date: string, days: number) {
  return isoDate(Date.parse(date) + days * DAY);
}

export function isTimeZone(value: unknown): value is string {
  if (typeof value !== "string" || value === "") return false;
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: value });
    return true;
  } catch {
    return false;
  }
}

export function utcOffsetMinutes(timeZone: string, at: Date) {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone,
    hourCycle: "h23",
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
  }).formatToParts(at);
  const part = (type: Intl.DateTimeFormatPartTypes) => Number(parts.find((p) => p.type === type)?.value);
  const wallClock = Date.UTC(part("year"), part("month") - 1, part("day"), part("hour"), part("minute"));
  return Math.round((wallClock - Math.floor(at.getTime() / MINUTE) * MINUTE) / MINUTE);
}

export function activityWindow(timeZone: string, now: Date) {
  const offset = utcOffsetMinutes(timeZone, now);
  const today = isoDate(now.getTime() + offset * MINUTE);
  const since = sqliteTime(Date.parse(addDays(today, 1 - DAYS)) - offset * MINUTE);
  return { today, since, modifier: `${offset} minutes` };
}

export function dailyActivity(counts: readonly DayCounts[], today: string, openNow: number): Day[] {
  const byDate = new Map(counts.map((count) => [count.date, count]));
  const days: Day[] = [];
  let open = openNow;
  for (let back = 0; back < DAYS; back++) {
    const date = addDays(today, -back);
    const { created = 0, completed = 0, reopened = 0, deleted = 0 } = byDate.get(date) ?? {};
    days.push({ date, added: created, completed: Math.max(0, completed - reopened), open: Math.max(0, open) });
    open -= created - completed + reopened - deleted;
  }
  return days.reverse();
}

export function total(days: readonly Day[], key: "added" | "completed") {
  return days.reduce((sum, day) => sum + day[key], 0);
}

export function summarize(days: readonly Day[], range: Range) {
  const current = days.slice(-range);
  const previous = days.slice(-2 * range, -range);
  return {
    since: current[0].date,
    completed: total(current, "completed"),
    completedBefore: total(previous, "completed"),
    added: total(current, "added"),
    addedBefore: total(previous, "added"),
    open: current[current.length - 1].open,
    openBefore: previous.length ? previous[previous.length - 1].open : current[0].open,
  };
}

export function ticks(max: number, count = 4) {
  const rough = Math.max(1, max) / count;
  const magnitude = 10 ** Math.floor(Math.log10(rough));
  const step = Math.max(1, [1, 2, 5, 10].map((m) => m * magnitude).find((s) => s >= rough) ?? 10 * magnitude);
  const top = Math.max(step, Math.ceil(max / step) * step);
  return Array.from({ length: top / step + 1 }, (_, index) => index * step);
}

export function heatLevel(count: number, busiest: number) {
  return count <= 0 || busiest <= 0 ? 0 : Math.ceil((count / busiest) * 4);
}
