import { describe, expect, it } from "vite-plus/test";
import { MINUTE, sqliteTime } from "./time";
import { clampIndex, depthOf, heightOf, isWithin, outlineOrder, todosAwaitingNote } from "./todos";

function row(id: number, parentId: number | null, position: number) {
  return { id, parentId, position };
}

describe("outlineOrder", () => {
  it("lists every todo right after its parent so clients can draw the tree in one pass", () => {
    const rows = [row(4, 1, 0), row(2, null, 1), row(1, null, 0), row(5, 4, 0), row(3, 1, 1)];
    expect(outlineOrder(rows).map((r) => r.id)).toEqual([1, 4, 5, 3, 2]);
  });

  it("orders siblings by position, falling back to creation order when positions collide", () => {
    const rows = [row(3, null, 0), row(1, null, 1), row(2, null, 0)];
    expect(outlineOrder(rows).map((r) => r.id)).toEqual([2, 3, 1]);
  });
});

describe("clampIndex", () => {
  it("appends when no index is given, which is where a plain `add` lands", () => {
    expect(clampIndex(undefined, 3)).toBe(3);
  });

  it("keeps an index past the end from leaving a gap in the sibling positions", () => {
    expect(clampIndex(10, 3)).toBe(3);
    expect(clampIndex(1, 3)).toBe(1);
  });
});

describe("isWithin", () => {
  const rows = [row(1, null, 0), row(2, 1, 0), row(3, 2, 0), row(4, null, 1)];

  it("finds a todo anywhere under the given ancestor so a move cannot detach a subtree into a loop", () => {
    expect(isWithin(rows, 3, 1)).toBe(true);
    expect(isWithin(rows, 1, 1)).toBe(true);
  });

  it("does not treat siblings or unrelated branches as descendants", () => {
    expect(isWithin(rows, 4, 1)).toBe(false);
    expect(isWithin(rows, 1, 2)).toBe(false);
  });
});

describe("depthOf and heightOf", () => {
  const rows = [row(1, null, 0), row(2, 1, 0), row(3, 2, 0), row(4, 1, 1), row(5, null, 1)];

  it("count the levels above and below a todo, which together decide whether a move keeps everything within the depth limit", () => {
    expect([depthOf(rows, 1), depthOf(rows, 3), depthOf(rows, 5)]).toEqual([1, 3, 1]);
    expect([heightOf(rows, 1), heightOf(rows, 2), heightOf(rows, 5)]).toEqual([3, 2, 1]);
  });

  it("stop at a parent loop, which two racing moves can leave behind, instead of looping forever", () => {
    const looped = [row(1, 2, 0), row(2, 1, 0), row(3, 1, 0)];
    expect(depthOf(looped, 3)).toBe(3);
    expect(heightOf(looped, 1)).toBe(2);
  });
});

describe("todosAwaitingNote", () => {
  const now = new Date("2026-10-30T12:00:00Z");

  function todo(id: number, parentId: number | null, fields: { title?: string; note?: string; completed?: boolean; age?: number } = {}) {
    const createdAt = sqliteTime(now.getTime() - (fields.age ?? 5) * MINUTE);
    return {
      id,
      parentId,
      position: 0,
      title: fields.title ?? `todo ${id}`,
      note: fields.note ?? "",
      completedAt: fields.completed ? createdAt : null,
      createdAt,
    };
  }

  const root = todo(1, null, { title: "Side projects", age: 60 });

  function awaiting(rows: ReturnType<typeof todo>[], at = now) {
    return todosAwaitingNote(rows, "Side projects", at).map((row) => row.id);
  }

  it("picks open todos without a note anywhere under the root, so the receiver can write notes for new tasks in that project", () => {
    expect(awaiting([root, todo(2, 1, { age: 60 }), todo(3, 2), todo(4, 1)])).toEqual([3, 4]);
  });

  it("skips a todo with todos under it, even completed ones, since it is a heading rather than a task", () => {
    expect(awaiting([root, todo(2, 1), todo(3, 2, { completed: true, age: 60 })])).toEqual([]);
  });

  it("skips a completed todo, since there is nothing left to do for it", () => {
    expect(awaiting([root, todo(2, 1, { completed: true })])).toEqual([]);
  });

  it("never picks the root itself, even before anything is added under it", () => {
    expect(awaiting([todo(1, null, { title: "Side projects" })])).toEqual([]);
  });

  it("leaves other projects alone, including a nested todo that happens to share the root's title", () => {
    const rows = [root, todo(2, null, { title: "Work", age: 60 }), todo(3, 2), todo(4, 2, { title: "Side projects", age: 60 }), todo(5, 4)];
    expect(awaiting(rows)).toEqual([]);
  });

  it("finds the root even with spaces around its title, so a stray space does not silently stop the webhook", () => {
    expect(awaiting([todo(1, null, { title: "\u00a0Side projects\u3000", age: 60 }), todo(2, 1)])).toEqual([2]);
  });

  it("treats a blank note or one that is only a URL as missing, so links saved for later get a note too", () => {
    const notes = ["", " \n\t", "\u3000", " https://example.com/a?b=c#d \n", "HTTP://EXAMPLE.COM"];
    const rows = [root, ...notes.map((note, i) => todo(i + 2, 1, { note }))];
    expect(awaiting(rows)).toEqual([2, 3, 4, 5, 6]);
  });

  it("leaves a note with anything besides a single URL alone, so what the member wrote is never overwritten", () => {
    const notes = ["memo", "see https://example.com", "https://a.example https://b.example", "https://a.example\nhttps://b.example", "https://", "ftp://example.com"];
    const rows = [root, ...notes.map((note, i) => todo(i + 2, 1, { note }))];
    expect(awaiting(rows)).toEqual([]);
  });

  it("skips a todo with a blank title, since a row left empty after Enter gives the receiver nothing to write a note about", () => {
    const rows = [root, ...["", "   ", "\u3000"].map((title, i) => todo(i + 2, 1, { title }))];
    expect(awaiting(rows)).toEqual([]);
  });

  it("skips todos caught in a parent loop, which two racing moves can leave behind, instead of looping forever", () => {
    expect(awaiting([root, todo(2, 3), todo(3, 2), todo(4, 1)])).toEqual([4]);
  });

  it("checks a large outline in one pass, so one member's thousands of project-titled headings cannot stall the shared webhook run for everyone", () => {
    const headings = Array.from({ length: 1000 }, (_, i) => todo(i + 1, null, { title: "Side projects", age: 60 }));
    const other = todo(2000, null, { title: "Elsewhere", age: 60 });
    const elsewhere = Array.from({ length: 100 }, (_, i) => todo(3000 + i, 2000));
    const started = performance.now();
    expect(awaiting([...headings, other, ...elsewhere, todo(5000, 1)])).toEqual([5000]);
    expect(performance.now() - started).toBeLessThan(500);
  });

  it("waits 3 minutes after a todo is added, since it is often still being edited right after", () => {
    expect(awaiting([root, todo(2, 1, { age: 2.5 }), todo(3, 1, { age: 3 })])).toEqual([3]);
  });

  it("picks each todo in exactly one of the runs 10 minutes apart, so the receiver is called once per todo and not again for one it left alone", () => {
    const runs = Array.from({ length: 12 }, (_, i) => new Date(now.getTime() + (i - 8) * 10 * MINUTE));
    for (let age = 0; age < 60; age += 0.5) {
      const rows = [root, todo(2, 1, { age })];
      expect(runs.filter((at) => awaiting(rows, at).length > 0)).toHaveLength(1);
    }
  });
});
