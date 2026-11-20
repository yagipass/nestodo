import { describe, expect, it } from "vite-plus/test";
import { byDay, isPostKey, type Entry, type Keys } from "./thread";

function event(at: string): Entry {
  return { type: "event", kind: "completed", at };
}

describe("byDay", () => {
  const now = new Date("2026-11-16T03:00:00Z");

  it("groups the log by the member's own days, so a comment written after midnight there is not filed under the day before", () => {
    const days = byDay([event("2026-11-15T14:30:00Z"), event("2026-11-15T15:30:00Z")], now, "Asia/Tokyo");

    expect(days.map((day) => [day.label, day.entries.map((entry) => entry.time)])).toEqual([
      ["Yesterday", ["23:30"]],
      ["Today", ["00:30"]],
    ]);
  });

  it("names older days by date and adds the year only when it is not this year, so the labels stay short", () => {
    const days = byDay([event("2025-12-31T01:00:00Z"), event("2026-11-12T00:14:00.123Z")], now, "Asia/Tokyo");

    expect(days.map((day) => day.label)).toEqual(["Dec 31, 2025", "Nov 12"]);
  });

  it("shows times on a 24-hour clock with two-digit hours, so morning and evening entries line up", () => {
    const [today] = byDay([event("2026-11-16T05:02:00Z"), event("2026-11-16T21:05:00Z")], now, "UTC");
    expect(today.entries.map((entry) => entry.time)).toEqual(["05:02", "21:05"]);
  });
});

describe("isPostKey", () => {
  const enter: Keys = { key: "Enter", shiftKey: false, altKey: false, isComposing: false };

  it("posts on Enter, so a comment is sent the way a chat message is", () => {
    expect(isPostKey(enter)).toBe(true);
  });

  it("leaves ⇧Enter alone, so a comment can have more than one line", () => {
    expect(isPostKey({ ...enter, shiftKey: true })).toBe(false);
  });

  it("never posts while an input method is composing, since its Enter only confirms the text being typed", () => {
    expect(isPostKey({ ...enter, isComposing: true })).toBe(false);
  });
});
