export type Comment = { id: number; body: string; via: "app" | "mcp"; at: string; editedAt: string | null };

export type TimelineItem = { type: "event"; kind: "created" | "completed" | "reopened"; at: string } | ({ type: "comment" } & Comment);

export type Entry = TimelineItem | { type: "pending"; key: string; body: string; at: string };

export type Day = { label: string; entries: { entry: Entry; time: string }[] };

const DAY_MS = 24 * 60 * 60 * 1000;

function dayKey(date: Date, timeZone?: string) {
  return new Intl.DateTimeFormat("en-CA", { timeZone, year: "numeric", month: "2-digit", day: "2-digit" }).format(date);
}

function dayLabel(date: Date, now: Date, timeZone?: string) {
  const key = dayKey(date, timeZone);
  if (key === dayKey(now, timeZone)) return "Today";
  if (key === dayKey(new Date(now.getTime() - DAY_MS), timeZone)) return "Yesterday";
  const sameYear = key.slice(0, 4) === dayKey(now, timeZone).slice(0, 4);
  return new Intl.DateTimeFormat("en-US", { timeZone, month: "short", day: "numeric", year: sameYear ? undefined : "numeric" }).format(date);
}

export function clockTime(date: Date, timeZone?: string) {
  return new Intl.DateTimeFormat("en-GB", { timeZone, hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(date);
}

export function byDay(entries: readonly Entry[], now: Date, timeZone?: string): Day[] {
  const days: Day[] = [];
  let current: string | null = null;
  for (const entry of entries) {
    const at = new Date(entry.at);
    const key = dayKey(at, timeZone);
    if (key !== current) {
      days.push({ label: dayLabel(at, now, timeZone), entries: [] });
      current = key;
    }
    days.at(-1)!.entries.push({ entry, time: clockTime(at, timeZone) });
  }
  return days;
}

export type Keys = { key: string; shiftKey: boolean; altKey: boolean; isComposing: boolean };

export function isPostKey({ key, shiftKey, altKey, isComposing }: Keys) {
  return key === "Enter" && !isComposing && !altKey && !shiftKey;
}
