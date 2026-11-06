import { describe, expect, it } from "vite-plus/test";
import {
  ROOT,
  append,
  applyOps,
  caretIn,
  destinations,
  fromTodos,
  indent,
  mergeIntoPrevious,
  moveBy,
  moveTo,
  outdent,
  remove,
  setCollapsed,
  split,
  toggleDone,
  undoFor,
  visibleRows,
  type Change,
  type Outline,
} from "./outline";
import { MAX_DEPTH, MAX_TITLE_LENGTH } from "./todos";

function todo(id: number, title: string, parentId: number | null = null, extra: { done?: boolean; collapsed?: boolean; note?: string } = {}) {
  return {
    id,
    title,
    note: extra.note ?? "",
    parentId,
    completedAt: extra.done ? "2026-10-23 09:00:00" : null,
    collapsed: extra.collapsed ?? false,
  };
}

function chain(depth: number) {
  return Array.from({ length: depth }, (_, i) => todo(i + 1, `level ${i + 1}`, i === 0 ? null : i));
}

function titles(outline: Outline, parent = ROOT): unknown[] {
  return outline[parent].children.map((key) =>
    outline[key].children.length ? [outline[key].title, titles(outline, key)] : outline[key].title,
  );
}

function shown(outline: Outline, showDone = true, query = "") {
  return new Set(visibleRows(outline, showDone, query).map((row) => row.key));
}

const sample = fromTodos([
  todo(1, "work"),
  todo(2, "release", 1),
  todo(3, "write notes", 2),
  todo(4, "expenses", 1),
  todo(5, "home"),
]);

describe("caretIn", () => {
  it("never puts the caret inside an emoji, so Enter cannot split it into two broken halves", () => {
    expect(caretIn("\u{1F600} party", 1)).toBe(2);
    expect(caretIn("ab", 1)).toBe(1);
  });

  it("keeps the caret within the text and treats a negative position as the end", () => {
    expect(caretIn("ab", 5)).toBe(2);
    expect(caretIn("ab", -1)).toBe(2);
  });
});

describe("fromTodos", () => {
  it("can reuse the keys of rows created in this tab, so reloading keeps their inputs and focus in place", () => {
    const keys = new Map([[1, "new1"]]);
    const outline = fromTodos([todo(1, "made here"), todo(2, "child", 1)], (id) => keys.get(id) ?? `t${id}`);
    expect(outline[ROOT].children).toEqual(["new1"]);
    expect(outline.new1.children).toEqual(["t2"]);
  });
});

describe("split", () => {
  it("keeps the head on the original row and creates the tail first, so a failure in between duplicates text instead of losing it", () => {
    const change = split(sample, "t4", 3, "n1", shown(sample));
    expect(titles(change.outline, "t1")).toEqual([["release", ["write notes"]], "exp", "enses"]);
    expect(change.ops).toEqual([
      { type: "create", key: "n1", parent: "t1", index: 2, title: "enses" },
      { type: "title", key: "t4", title: "exp" },
    ]);
    expect(change.focus).toEqual({ key: "n1", pos: 0 });
  });

  it("starts the first child when Enter is pressed at the end of an open parent, like WorkFlowy", () => {
    const change = split(sample, "t2", "release".length, "n1", shown(sample));
    expect(titles(change.outline, "t2")).toEqual(["", "write notes"]);
    expect(change.ops).toEqual([{ type: "create", key: "n1", parent: "t2", index: 0, title: "" }]);
  });

  it("opens an empty row above when Enter is pressed at the start, leaving the cursor on the original text", () => {
    const change = split(sample, "t5", 0, "n1", shown(sample));
    expect(titles(change.outline)).toEqual([["work", [["release", ["write notes"]], "expenses"]], "", "home"]);
    expect(change.ops).toEqual([{ type: "create", key: "n1", parent: ROOT, index: 1, title: "" }]);
    expect(change.focus).toEqual({ key: "t5", pos: 0 });
  });

  it("adds a sibling at the end of a parent whose children are all hidden as completed, so Enter acts as it does on a row without children", () => {
    const outline = fromTodos([todo(1, "trip"), todo(2, "tickets", 1, { done: true }), todo(3, "home")]);
    const change = split(outline, "t1", "trip".length, "n1", shown(outline, false));
    expect(change.ops).toEqual([{ type: "create", key: "n1", parent: ROOT, index: 1, title: "" }]);
    expect(visibleRows(change.outline, false)).toEqual([
      { key: "t1", depth: 0, under: false },
      { key: "n1", depth: 0, under: false },
      { key: "t3", depth: 0, under: false },
    ]);
  });

  it("adds a sibling at the end of a parent whose children the filter hides, so Enter follows the rows on screen as it does on a row without children", () => {
    const outline = fromTodos([todo(1, "trip"), todo(2, "tickets", 1), todo(3, "home")]);
    const change = split(outline, "t1", "trip".length, "n1", shown(outline, true, "trip"));
    expect(change.ops).toEqual([{ type: "create", key: "n1", parent: ROOT, index: 1, title: "" }]);
  });

  it("starts the first child when some of the parent's children are on screen, even with others hidden, so the new row appears right under it", () => {
    const outline = fromTodos([todo(1, "trip"), todo(2, "tickets", 1, { done: true }), todo(3, "passport", 1), todo(4, "home")]);
    const change = split(outline, "t1", "trip".length, "n1", shown(outline, false));
    expect(change.ops).toEqual([{ type: "create", key: "n1", parent: "t1", index: 0, title: "" }]);
  });

  it("leaves the note with the head of the title, since the note was written about the row it is under", () => {
    const outline = fromTodos([todo(1, "book hotel near the station", null, { note: "budget 20000 yen" })]);
    const change = split(outline, "t1", "book hotel".length, "n1", shown(outline));
    expect([change.outline.t1.note, change.outline.n1.note]).toEqual(["budget 20000 yen", ""]);
  });
});

describe("indent and outdent", () => {
  it("nests a row as the last child of the row above and opens that row if it was collapsed", () => {
    const outline = fromTodos([todo(1, "a", null, { collapsed: true }), todo(2, "a1", 1), todo(3, "b")]);
    const change = indent(outline, "t3", shown(outline))!;
    expect(titles(change.outline)).toEqual([["a", ["a1", "b"]]]);
    expect(change.outline.t1.collapsed).toBe(false);
    expect(change.ops).toEqual([
      { type: "move", key: "t3", parent: "t1", index: 1 },
      { type: "update", key: "t1", fields: { collapsed: false } },
    ]);
  });

  it("skips hidden completed rows so a row never disappears into one the user cannot see", () => {
    const outline = fromTodos([todo(1, "visible"), todo(2, "hidden", null, { done: true }), todo(3, "row")]);
    expect(indent(outline, "t3", shown(outline, false))!.ops).toEqual([{ type: "move", key: "t3", parent: "t1", index: 0 }]);
  });

  it("does nothing when the only rows above are hidden as completed, rather than nesting into one nobody can see", () => {
    const outline = fromTodos([todo(1, "hidden", null, { done: true }), todo(2, "row")]);
    expect(indent(outline, "t2", shown(outline, false))).toBeNull();
  });

  it("skips rows the filter hides, so a row is nested under the match the user sees above it", () => {
    const outline = fromTodos([todo(1, "buy milk"), todo(2, "call mom"), todo(3, "buy eggs")]);
    expect(indent(outline, "t3", shown(outline, true, "buy"))!.ops).toEqual([{ type: "move", key: "t3", parent: "t1", index: 0 }]);
  });

  it("does nothing for the first row in its list", () => {
    expect(indent(sample, "t1", shown(sample))).toBeNull();
  });

  it("does nothing when the row or a row under it would end up deeper than the server allows, rather than making a change that fails to save", () => {
    const parent = MAX_DEPTH - 2;
    const outline = fromTodos([...chain(parent), todo(100, "a", parent), todo(101, "b", parent), todo(102, "under b", 101), todo(103, "c", parent)]);
    expect(indent(outline, "t101", shown(outline))).toBeNull();
    expect(indent(outline, "t103", shown(outline))!.ops).toEqual([{ type: "move", key: "t103", parent: "t101", index: 1 }]);
  });

  it("places an outdented row right after its old parent and leaves the following siblings where they were", () => {
    const change = outdent(sample, "t2")!;
    expect(titles(change.outline)).toEqual([["work", ["expenses"]], ["release", ["write notes"]], "home"]);
    expect(change.ops).toEqual([{ type: "move", key: "t2", parent: ROOT, index: 1 }]);
  });

  it("does nothing for a top-level row, which has no parent to move out of", () => {
    expect(outdent(sample, "t1")).toBeNull();
  });
});

describe("moveBy", () => {
  it("sends the final position among siblings, which is what the server expects", () => {
    const change = moveBy(sample, "t1", 1, shown(sample))!;
    expect(titles(change.outline)).toEqual(["home", ["work", [["release", ["write notes"]], "expenses"]]]);
    expect(change.ops).toEqual([{ type: "move", key: "t1", parent: ROOT, index: 1 }]);
  });

  it("stops at the ends of the list", () => {
    expect(moveBy(sample, "t1", -1, shown(sample))).toBeNull();
    expect(moveBy(sample, "t5", 1, shown(sample))).toBeNull();
  });

  it("skips hidden completed siblings so every press visibly moves the row", () => {
    const outline = fromTodos([todo(1, "A"), todo(2, "Done", null, { done: true }), todo(3, "B")]);
    const down = moveBy(outline, "t1", 1, shown(outline, false))!;
    expect(visibleRows(down.outline, false).map((row) => row.key)).toEqual(["t3", "t1"]);
    expect(down.ops).toEqual([{ type: "move", key: "t1", parent: ROOT, index: 2 }]);
    const up = moveBy(outline, "t3", -1, shown(outline, false))!;
    expect(visibleRows(up.outline, false).map((row) => row.key)).toEqual(["t3", "t1"]);
    expect(up.ops).toEqual([{ type: "move", key: "t3", parent: ROOT, index: 0 }]);
  });

  it("does nothing when the only siblings that way are hidden as completed, rather than making a move nobody can see", () => {
    const outline = fromTodos([todo(1, "A"), todo(2, "Done", null, { done: true })]);
    expect(moveBy(outline, "t1", 1, shown(outline, false))).toBeNull();
  });

  it("skips siblings the filter hides, so the row moves past the next match the user can see", () => {
    const outline = fromTodos([todo(1, "buy milk"), todo(2, "call mom"), todo(3, "buy eggs")]);
    const down = moveBy(outline, "t1", 1, shown(outline, true, "buy"))!;
    expect(visibleRows(down.outline, true, "buy").map((row) => row.key)).toEqual(["t3", "t1"]);
    expect(down.ops).toEqual([{ type: "move", key: "t1", parent: ROOT, index: 2 }]);
  });
});

describe("moveTo", () => {
  it("puts the row with its subtree at the end of the chosen todo's children in a single move", () => {
    const change = moveTo(sample, "t2", "t5");
    expect(titles(change.outline)).toEqual([["work", ["expenses"]], ["home", [["release", ["write notes"]]]]]);
    expect(change.ops).toEqual([{ type: "move", key: "t2", parent: "t5", index: 0 }]);
  });

  it("sends the final position when the row is moved to the end of its own parent, which is what the server expects", () => {
    const change = moveTo(sample, "t2", "t1");
    expect(titles(change.outline)).toEqual([["work", ["expenses", ["release", ["write notes"]]]], "home"]);
    expect(change.ops).toEqual([{ type: "move", key: "t2", parent: "t1", index: 1 }]);
  });

  it("leaves a collapsed todo closed, so the rows the user is working among do not jump around", () => {
    const outline = fromTodos([todo(1, "a", null, { collapsed: true }), todo(2, "a1", 1), todo(3, "b")]);
    const change = moveTo(outline, "t3", "t1");
    expect(change.outline.t1.collapsed).toBe(true);
    expect(change.ops).toEqual([{ type: "move", key: "t3", parent: "t1", index: 1 }]);
  });
});

describe("destinations", () => {
  it("lists every other todo in outline order with the titles above it, so todos with the same name can be told apart", () => {
    expect(destinations(sample, "t5", true, "")).toEqual([
      { key: "t1", title: "work", path: [] },
      { key: "t2", title: "release", path: ["work"] },
      { key: "t3", title: "write notes", path: ["work", "release"] },
      { key: "t4", title: "expenses", path: ["work"] },
    ]);
  });

  it("never offers the row itself or anything under it, since the server refuses to put a todo inside itself", () => {
    expect(destinations(sample, "t1", true, "").map((d) => d.key)).toEqual(["t5"]);
  });

  it("finds todos by title regardless of case, even inside collapsed ones that are out of sight", () => {
    const outline = fromTodos([todo(1, "work", null, { collapsed: true }), todo(2, "Release", 1), todo(3, "home")]);
    expect(destinations(outline, "t3", true, " rel ").map((d) => d.key)).toEqual(["t2"]);
  });

  it("leaves out completed todos and everything under them while completed todos are hidden", () => {
    const outline = fromTodos([todo(1, "old", null, { done: true }), todo(2, "inside old", 1), todo(3, "open"), todo(4, "row")]);
    expect(destinations(outline, "t4", false, "").map((d) => d.key)).toEqual(["t3"]);
    expect(destinations(outline, "t4", true, "").map((d) => d.key)).toEqual(["t1", "t2", "t3"]);
  });

  it("leaves out todos that would put the moved row or a row under it deeper than the server allows", () => {
    const outline = fromTodos([...chain(MAX_DEPTH), todo(100, "branch"), todo(101, "leaf", 100)]);
    const levels = (count: number) => chain(count).map((t) => `t${t.id}`);
    expect(destinations(outline, "t100", true, "").map((d) => d.key)).toEqual(levels(MAX_DEPTH - 2));
    expect(destinations(outline, "t101", true, "").map((d) => d.key)).toEqual([...levels(MAX_DEPTH - 1), "t100"]);
  });

  it("leaves out todos without a title, which could not be told apart in the list", () => {
    const outline = fromTodos([todo(1, " "), todo(2, "under blank", 1), todo(3, "row")]);
    expect(destinations(outline, "t3", true, "")).toEqual([{ key: "t2", title: "under blank", path: [" "] }]);
  });
});

describe("mergeIntoPrevious", () => {
  const rowsOf = (outline: Outline) => visibleRows(outline, true);

  it("joins a row onto the one above and removes it, leaving the cursor at the seam", () => {
    const outline = fromTodos([todo(1, "buy "), todo(2, "milk")]);
    const change = mergeIntoPrevious(outline, rowsOf(outline), "t2")!;
    expect(titles(change.outline)).toEqual(["buy milk"]);
    expect(change.ops).toEqual([
      { type: "title", key: "t1", title: "buy milk" },
      { type: "delete", key: "t2", removed: ["t2"] },
    ]);
    expect(change.focus).toEqual({ key: "t1", pos: 4 });
  });

  it("refuses when the row above sits deeper, so text never jumps into another branch", () => {
    expect(mergeIntoPrevious(sample, rowsOf(sample), "t5")).toBeNull();
  });

  it("refuses when the row has children, so Backspace never deletes a subtree by accident", () => {
    expect(mergeIntoPrevious(sample, rowsOf(sample), "t2")).toBeNull();
  });

  it("refuses when a completed row is involved, so merging never silently changes what is done", () => {
    const intoDone = fromTodos([todo(1, "done", null, { done: true }), todo(2, "open")]);
    expect(mergeIntoPrevious(intoDone, rowsOf(intoDone), "t2")).toBeNull();
    const doneIntoOpen = fromTodos([todo(1, "open"), todo(2, "done", null, { done: true })]);
    expect(mergeIntoPrevious(doneIntoOpen, rowsOf(doneIntoOpen), "t2")).toBeNull();
  });

  it("refuses when the row has a note, so Backspace never throws away what was written under it", () => {
    const outline = fromTodos([todo(1, "buy "), todo(2, "milk", null, { note: "the low-fat one" })]);
    expect(mergeIntoPrevious(outline, rowsOf(outline), "t2")).toBeNull();
  });

  it("refuses when the joined title would be longer than the server accepts", () => {
    const outline = fromTodos([todo(1, "x".repeat(MAX_TITLE_LENGTH - 50)), todo(2, "y".repeat(51))]);
    expect(mergeIntoPrevious(outline, rowsOf(outline), "t2")).toBeNull();
  });
});

describe("remove", () => {
  it("deletes the whole subtree in one request and lists every removed row so their pending edits can be dropped", () => {
    const change = remove(sample, "t1");
    expect(titles(change.outline)).toEqual(["home"]);
    expect(change.ops).toEqual([{ type: "delete", key: "t1", removed: ["t1", "t2", "t3", "t4"] }]);
  });
});

describe("undoFor", () => {
  const folded = fromTodos([todo(1, "trip"), todo(2, "packing", null, { collapsed: true }), todo(3, "socks", 2), todo(4, "passport")]);
  const pair = fromTodos([todo(1, "buy "), todo(2, "milk")]);
  const edits: [string, Outline, Change][] = [
    ["split", sample, split(sample, "t4", 3, "n1", shown(sample))],
    ["split into children", sample, split(sample, "t2", "release".length, "n1", shown(sample))],
    ["insert above", sample, split(sample, "t5", 0, "n1", shown(sample))],
    ["append", sample, append(sample, ROOT, "n1")],
    ["indent into a collapsed row", folded, indent(folded, "t4", shown(folded))!],
    ["outdent", sample, outdent(sample, "t3")!],
    ["move up", sample, moveBy(sample, "t4", -1, shown(sample))!],
    ["move to another branch", sample, moveTo(sample, "t5", "t2")],
    ["move to the end of its own parent", sample, moveTo(sample, "t2", "t1")],
    ["complete", sample, toggleDone(sample, "t3")],
    ["collapse", sample, setCollapsed(sample, "t1", true)!],
    ["delete a subtree", sample, remove(sample, "t1")],
    ["merge", pair, mergeIntoPrevious(pair, visibleRows(pair, true), "t2")!],
  ];

  it("replays each edit's changes into the outline the editor shows, so undo starts from what is on screen", () => {
    for (const [name, before, change] of edits) {
      expect(applyOps(before, change.ops), name).toEqual(change.outline);
    }
  });

  it("takes every edit back to exactly the outline before it, so Cmd+Z never leaves a row in a different place", () => {
    for (const [name, before, change] of edits) {
      expect(applyOps(change.outline, undoFor(before, change.ops)), name).toEqual(before);
    }
  });

  it("recreates a deleted subtree parents first under the same keys, with every note, completion and fold it had", () => {
    const outline = fromTodos([
      todo(1, "trip", null, { collapsed: true }),
      todo(2, "packing", 1, { note: "check the weather" }),
      todo(3, "socks", 2, { done: true }),
      todo(4, "home"),
    ]);
    expect(undoFor(outline, remove(outline, "t1").ops)).toEqual([
      { type: "create", key: "t1", parent: ROOT, index: 0, title: "trip", note: "" },
      { type: "update", key: "t1", fields: { collapsed: true } },
      { type: "create", key: "t2", parent: "t1", index: 0, title: "packing", note: "check the weather" },
      { type: "create", key: "t3", parent: "t2", index: 0, title: "socks", note: "" },
      { type: "update", key: "t3", fields: { completed: true } },
    ]);
  });
});

describe("visibleRows", () => {
  const outline = fromTodos([
    todo(1, "project", null, { done: true }),
    todo(2, "step", 1),
    todo(3, "folded", null, { collapsed: true }),
    todo(4, "inside fold", 3),
  ]);

  it("greys the rows under a completed row without marking them done themselves", () => {
    expect(visibleRows(outline, true).map(({ key, under }) => [key, under])).toEqual([
      ["t1", false],
      ["t2", true],
      ["t3", false],
    ]);
  });

  it("hides a completed row together with its subtree when completed rows are hidden", () => {
    expect(visibleRows(outline, false).map((row) => row.key)).toEqual(["t3"]);
  });

  it("shows filter matches with their parents even inside collapsed rows", () => {
    expect(visibleRows(outline, true, "INSIDE").map((row) => row.key)).toEqual(["t3", "t4"]);
  });

  it("finds a row by its note, so details kept out of the title can still be filtered for", () => {
    const noted = fromTodos([todo(1, "dentist", null, { note: "Only weekday mornings" }), todo(2, "milk")]);
    expect(visibleRows(noted, true, "weekday").map((row) => row.key)).toEqual(["t1"]);
  });

  it("keeps rows created during a filter visible even though their text does not match yet", () => {
    expect(visibleRows(outline, true, "inside", new Set(["t1"])).map((row) => row.key)).toEqual(["t1", "t3", "t4"]);
  });
});
