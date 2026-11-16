import { describe, expect, it } from "vite-plus/test";
import { byDay, isPostKey, snippet, type Entry, type Keys } from "./thread";

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

  it("shows times on a 24-hour clock, the same width for every entry", () => {
    expect(byDay([event("2026-11-16T05:02:00Z")], now, "UTC")[0].entries[0].time).toBe("05:02");
  });
});

describe("isPostKey", () => {
  const enter: Keys = { key: "Enter", shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, isComposing: false };

  it("posts on ⇧Enter, and on Ctrl/⌘ Enter for those used to it from other apps", () => {
    expect([{ shiftKey: true }, { ctrlKey: true }, { metaKey: true }].map((held) => isPostKey({ ...enter, ...held }))).toEqual([true, true, true]);
  });

  it("leaves Enter alone, so a comment can have more than one line", () => {
    expect(isPostKey(enter)).toBe(false);
  });

  it("never posts while an input method is composing, since its Enter only confirms the text being typed", () => {
    expect(isPostKey({ ...enter, shiftKey: true, isComposing: true })).toBe(false);
  });
});

describe("snippet", () => {
  it("shows the part of a long comment around the match, so the row shows why it was found", () => {
    const body = `${"a".repeat(100)} the quote from B ${"z".repeat(100)}`;

    const shown = snippet(body, "QUOTE", 10);

    expect(shown.startsWith("…")).toBe(true);
    expect(shown).toContain("the quote from B");
    expect(shown.length).toBeLessThan(40);
  });

  it("puts a multi-line comment on one line, since the row has room for one", () => {
    expect(snippet("asked\n\ntwo   companies", "two")).toBe("asked two companies");
  });
});
