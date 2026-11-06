import { describe, expect, it } from "vite-plus/test";
import { activityWindow, dailyActivity, DAYS, heatLevel, isTimeZone, summarize, ticks, utcOffsetMinutes, type Day } from "./stats";

function counts(date: string, created = 0, completed = 0, reopened = 0, deleted = 0) {
  return { date, created, completed, reopened, deleted };
}

describe("activityWindow", () => {
  it("counts a todo completed at 08:00 in Tokyo on that Tokyo day, not on the UTC day before", () => {
    const window = activityWindow("Asia/Tokyo", new Date("2026-10-23T23:00:00Z"));
    expect(window.today).toBe("2026-10-24");
    expect(window.modifier).toBe("540 minutes");
  });

  it("starts the query at local midnight of the first day shown so the oldest day is not cut short", () => {
    const window = activityWindow("Asia/Tokyo", new Date("2026-10-23T23:00:00Z"));
    expect(window.since).toBe("2026-04-25 15:00:00");
  });
});

describe("utcOffsetMinutes", () => {
  it("follows daylight saving time, so a viewer in New York gets the offset in effect that day", () => {
    expect(utcOffsetMinutes("America/New_York", new Date("2026-07-01T12:00:00Z"))).toBe(-240);
    expect(utcOffsetMinutes("America/New_York", new Date("2026-01-15T12:00:00Z"))).toBe(-300);
  });
});

describe("isTimeZone", () => {
  it("rejects names that are not time zones, so a bad ?tz falls back instead of failing the page", () => {
    expect(isTimeZone("Asia/Tokyo")).toBe(true);
    expect(isTimeZone("Not/AZone")).toBe(false);
    expect(isTimeZone("")).toBe(false);
    expect(isTimeZone(undefined)).toBe(false);
  });
});

describe("dailyActivity", () => {
  it("returns every day of the period, with quiet days as zero, so charts have no gaps", () => {
    const days = dailyActivity([counts("2026-10-20", 2, 1)], "2026-10-24", 5);
    expect(days).toHaveLength(DAYS);
    expect(days[0].date).toBe("2026-04-26");
    expect(days.at(-1)).toEqual({ date: "2026-10-24", added: 0, completed: 0, open: 5 });
    expect(days.find((day) => day.date === "2026-10-20")).toMatchObject({ added: 2, completed: 1 });
  });

  it("works the open count back from today's, so it matches the outline now even when old history is missing", () => {
    const days = dailyActivity(
      [counts("2026-10-23", 3, 1), counts("2026-10-24", 1, 2, 0, 1)],
      "2026-10-24",
      10,
    );
    expect(days.slice(-3).map((day) => day.open)).toEqual([10, 12, 10]);
  });

  it("counts a reopened todo as open only from the day it was reopened, so the days before show it as done", () => {
    const days = dailyActivity(
      [counts("2026-10-01", 1), counts("2026-10-20", 0, 1), counts("2026-10-24", 0, 0, 1)],
      "2026-10-24",
      1,
    );
    expect(days.slice(-5).map((day) => day.open)).toEqual([0, 0, 0, 0, 1]);
    expect(days.find((day) => day.date === "2026-09-30")?.open).toBe(0);
  });

  it("does not count a todo completed and reopened on the same day as done", () => {
    const days = dailyActivity([counts("2026-10-24", 0, 1, 1)], "2026-10-24", 1);
    expect(days.at(-1)?.completed).toBe(0);
  });

  it("never shows a negative completion count when an older todo is reopened", () => {
    const days = dailyActivity([counts("2026-10-24", 0, 0, 2)], "2026-10-24", 2);
    expect(days.at(-1)?.completed).toBe(0);
  });
});

describe("summarize", () => {
  const days: Day[] = Array.from({ length: DAYS }, (_, index) => ({
    date: `day-${index}`,
    added: 1,
    completed: index >= DAYS - 7 ? 2 : 1,
    open: index,
  }));

  it("compares the chosen period with the one right before it", () => {
    expect(summarize(days, 7)).toMatchObject({ completed: 14, completedBefore: 7, added: 7, addedBefore: 7 });
  });

  it("reports how the open count moved since the day before the period began", () => {
    expect(summarize(days, 30)).toMatchObject({ open: DAYS - 1, openBefore: DAYS - 31, since: `day-${DAYS - 30}` });
  });
});

describe("ticks", () => {
  it("always reaches the busiest day so no column is drawn past the top of the chart", () => {
    for (const max of [0, 1, 3, 7, 12, 48, 130]) {
      const scale = ticks(max);
      expect(scale[0]).toBe(0);
      expect(scale.at(-1)).toBeGreaterThanOrEqual(max);
    }
  });

  it("uses whole, evenly spaced steps because counts are whole numbers", () => {
    expect(ticks(7)).toEqual([0, 2, 4, 6, 8]);
    expect(ticks(2)).toEqual([0, 1, 2]);
    expect(ticks(130)).toEqual([0, 50, 100, 150]);
  });
});

describe("heatLevel", () => {
  it("shades a day by its share of the busiest day, leaving quiet days blank", () => {
    expect(heatLevel(0, 8)).toBe(0);
    expect(heatLevel(1, 8)).toBe(1);
    expect(heatLevel(8, 8)).toBe(4);
    expect(heatLevel(0, 0)).toBe(0);
  });
});
