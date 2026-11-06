export const MINUTE = 60_000;

export const DAY = 24 * 60 * MINUTE;

export function sqliteTime(ms: number) {
  return new Date(ms).toISOString().slice(0, 19).replace("T", " ");
}

export function inUtc(time: string) {
  return `${time.replace(" ", "T")}Z`;
}
