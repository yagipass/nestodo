import { describe, expect, it } from "vite-plus/test";
import {
  ROOT,
  append,
  applyOps,
  caretIn,
  destinations,
  fromTodos,
  guides,
  indent,
  mergeIntoPrevious,
  moveBy,
  moveTo,
  nextHit,
  openUnder,
  outdent,
  remove,
  searchHits,
  setCollapsed,
  setIcon,
  split,
  subtree,
  toggleDone,
  undoFor,
  visibleRows,
  withoutIcon,
  type Change,
  type Outline,
} from "./outline";
import { MAX_DEPTH, MAX_TITLE_LENGTH } from "./todos";

function todo(id: number, title: string, parentId: number | null = null, extra: { done?: boolean; collapsed?: boolean; comments?: number; icon?: string } = {}) {
  return {
    id,
    title,
    icon: extra.icon ?? null,
    commentCount: extra.comments ?? 0,
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

function shown(outline: Outline, showDone = true) {
  return new Set(visibleRows(outline, showDone).map((row) => row.key));
}

const sample = fromTodos([
  todo(1, "work"),
  todo(2, "release", 1),
  todo(3, "write notes", 2),
  todo(4, "expenses", 1),
  todo(5, "home"),
]);

const errands = fromTodos([
  todo(1, "errands"),
  todo(2, "post office", 1, { done: true }),
  todo(3, "bank", 1, { collapsed: true }),
  todo(4, "pay rent", 3),
  todo(5, "home"),
]);

const trip = fromTodos([
  todo(1, "trip", null, { done: true }),
  todo(2, "tickets", 1, { done: true }),
  todo(3, "hotel", 1, { done: true }),
  todo(4, "unpack"),
]);

function doneTitles(outline: Outline) {
  return subtree(outline, ROOT)
    .filter((key) => outline[key].done)
    .map((key) => outline[key].title);
}

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

  it("reads a todo from a server that predates icons as having none, so the page still loads during a deploy", () => {
    const { icon, ...older } = todo(1, "groceries", null, { icon: "🛒" });
    expect(fromTodos([older]).t1.icon).toBeNull();
    expect(fromTodos([{ ...older, icon }]).t1.icon).toBe("🛒");
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

  it("starts the first child when some of the parent's children are on screen, even with others hidden, so the new row appears right under it", () => {
    const outline = fromTodos([todo(1, "trip"), todo(2, "tickets", 1, { done: true }), todo(3, "passport", 1), todo(4, "home")]);
    const change = split(outline, "t1", "trip".length, "n1", shown(outline, false));
    expect(change.ops).toEqual([{ type: "create", key: "n1", parent: "t1", index: 0, title: "" }]);
  });

  it("leaves the comments with the head of the title, since they were written about the row they are under", () => {
    const outline = fromTodos([todo(1, "book hotel near the station", null, { comments: 2 })]);
    const change = split(outline, "t1", "book hotel".length, "n1", shown(outline));
    expect([change.outline.t1.comments, change.outline.n1.comments]).toEqual([2, 0]);
  });

  it("leaves the icon on the row being split, so the new row does not look like a copy of it", () => {
    const outline = fromTodos([todo(1, "groceries milk", null, { icon: "🛒" })]);
    const middle = split(outline, "t1", "groceries".length, "n1", shown(outline));
    expect([middle.outline.t1.icon, middle.outline.n1.icon]).toEqual(["🛒", null]);
    const start = split(outline, "t1", 0, "n1", shown(outline));
    expect([start.outline.t1.icon, start.outline.n1.icon]).toEqual(["🛒", null]);
  });
});

describe("setIcon", () => {
  it("changes only the icon in one request, so the title being typed is never sent along with it", () => {
    const outline = fromTodos([todo(1, "groceries")]);
    const change = setIcon(outline, "t1", "🛒")!;
    expect(change.outline.t1).toMatchObject({ title: "groceries", icon: "🛒" });
    expect(change.ops).toEqual([{ type: "update", key: "t1", fields: { icon: "🛒" } }]);
    expect(setIcon(change.outline, "t1", null)!.ops).toEqual([{ type: "update", key: "t1", fields: { icon: null } }]);
  });

  it("does nothing when the same icon is picked again, so it adds no undo step and no request", () => {
    const outline = fromTodos([todo(1, "groceries", null, { icon: "🛒" })]);
    expect(setIcon(outline, "t1", "🛒")).toBeNull();
  });
});

describe("withoutIcon", () => {
  const cat = "image:3f2a9c1e-5b7d-4e8f-9a0b-1c2d3e4f5a6b";

  it("takes a deleted image off every row that showed it, matching what the server did, and leaves other icons alone", () => {
    const outline = fromTodos([todo(1, "vet", null, { icon: cat }), todo(2, "food", 1, { icon: cat }), todo(3, "groceries", null, { icon: "🛒" })]);
    const next = withoutIcon(outline, cat);
    expect([next.t1.icon, next.t2.icon, next.t3.icon]).toEqual([null, null, "🛒"]);
    expect(next.t1.title).toBe("vet");
  });

  it("returns the same outline when no row used the image, so nothing is redrawn", () => {
    const outline = fromTodos([todo(1, "groceries", null, { icon: "🛒" })]);
    expect(withoutIcon(outline, cat)).toBe(outline);
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

  it("refuses when the row has comments, so Backspace never throws away the log kept on it", () => {
    const outline = fromTodos([todo(1, "buy "), todo(2, "milk", null, { comments: 1 })]);
    expect(mergeIntoPrevious(outline, rowsOf(outline), "t2")).toBeNull();
  });

  it("refuses when the row has an icon, so Backspace never throws away an icon the member chose", () => {
    const outline = fromTodos([todo(1, "buy "), todo(2, "milk", null, { icon: "🥛" })]);
    expect(mergeIntoPrevious(outline, rowsOf(outline), "t2")).toBeNull();
  });

  it("refuses for an icon kind this app cannot draw yet, so an older app never drops what a newer one set", () => {
    const outline = fromTodos([todo(1, "buy "), todo(2, "milk", null, { icon: "line:cart:green" })]);
    expect(mergeIntoPrevious(outline, rowsOf(outline), "t2")).toBeNull();
  });

  it("keeps the icon of the row above when joining onto it, since that row stays", () => {
    const outline = fromTodos([todo(1, "buy ", null, { icon: "🛒" }), todo(2, "milk")]);
    expect(mergeIntoPrevious(outline, rowsOf(outline), "t2")!.outline.t1.icon).toBe("🛒");
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

describe("completing", () => {
  it("counts the open todos at every depth under a todo, even inside a collapsed one, so the confirmation names everything it will complete", () => {
    expect(openUnder(errands, "t1")).toEqual(["t3", "t4"]);
    expect(openUnder(trip, "t1")).toEqual([]);
  });

  it("completes every open todo under a todo along with it, in one request the server repeats, so a completed todo never hides an open one", () => {
    const change = toggleDone(errands, "t1");
    expect(doneTitles(change.outline)).toEqual(["errands", "post office", "bank", "pay rent"]);
    expect(change.ops).toEqual([{ type: "update", key: "t1", fields: { completed: true } }]);
  });

  it("reopens only the todo itself, leaving the todos under it completed, since reopening a parent says nothing about its children", () => {
    expect(doneTitles(toggleDone(trip, "t1").outline)).toEqual(["tickets", "hotel"]);
  });

  it("reopens the completed todos above a reopened todo, so it does not stay hidden under them", () => {
    expect(doneTitles(toggleDone(trip, "t3").outline)).toEqual(["tickets"]);
  });

  it("reopens a completed todo when Enter adds an empty row under it, as the server does", () => {
    const change = split(trip, "t1", "trip".length, "n1", shown(trip));
    expect(change.outline.t1.children[0]).toBe("n1");
    expect(doneTitles(change.outline)).toEqual(["tickets", "hotel"]);
  });

  it("reopens a completed todo when an open row is indented into it", () => {
    expect(doneTitles(indent(trip, "t4", shown(trip))!.outline)).toEqual(["tickets", "hotel"]);
  });

  it("reopens every completed todo above the spot an open row is moved to", () => {
    expect(doneTitles(moveTo(trip, "t4", "t3").outline)).toEqual(["tickets"]);
  });

  it("keeps a completed todo completed when a completed row is moved into it, since nothing open ends up under it", () => {
    const moved = moveTo(toggleDone(trip, "t4").outline, "t4", "t1");
    expect(doneTitles(moved.outline)).toEqual(["trip", "tickets", "hotel", "unpack"]);
  });
});

describe("undoFor", () => {
  const folded = fromTodos([todo(1, "trip"), todo(2, "packing", null, { collapsed: true }), todo(3, "socks", 2), todo(4, "passport")]);
  const pair = fromTodos([todo(1, "buy "), todo(2, "milk")]);
  const marked = fromTodos([todo(1, "groceries", null, { icon: "🛒" }), todo(2, "dentist")]);
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
    ["complete a todo with open todos under it", errands, toggleDone(errands, "t1")],
    ["reopen a completed parent", trip, toggleDone(trip, "t1")],
    ["reopen under a completed parent", trip, toggleDone(trip, "t2")],
    ["add under a completed parent", trip, split(trip, "t2", "tickets".length, "n1", shown(trip))],
    ["indent into a completed row", trip, indent(trip, "t4", shown(trip))!],
    ["move into a completed branch", trip, moveTo(trip, "t4", "t2")],
    ["delete inside a completed subtree", trip, remove(trip, "t3")],
    ["set an icon", marked, setIcon(marked, "t2", "🦷")!],
    ["change an icon", marked, setIcon(marked, "t1", "🥛")!],
    ["remove an icon", marked, setIcon(marked, "t1", null)!],
    ["delete a row with an icon", marked, remove(marked, "t1")],
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

  it("puts back the icon a row had before, including none, in one request", () => {
    const outline = fromTodos([todo(1, "groceries", null, { icon: "🛒" }), todo(2, "dentist")]);
    expect(undoFor(outline, setIcon(outline, "t1", "🥛")!.ops)).toEqual([{ type: "update", key: "t1", fields: { icon: "🛒" } }]);
    expect(undoFor(outline, setIcon(outline, "t2", "🦷")!.ops)).toEqual([{ type: "update", key: "t2", fields: { icon: null } }]);
  });

  it("reopens exactly the todos a completion took along, leaving the ones completed before it as they were", () => {
    expect(undoFor(errands, toggleDone(errands, "t1").ops)).toEqual([
      { type: "update", key: "t1", fields: { completed: false } },
      { type: "update", key: "t3", fields: { completed: false } },
      { type: "update", key: "t4", fields: { completed: false } },
    ]);
  });

  it("removes a row added under a completed todo before completing that todo again, so the server's cascade never completes the row on its way out", () => {
    const added = split(trip, "t3", "hotel".length, "n1", shown(trip));
    expect(undoFor(trip, added.ops)).toEqual([
      { type: "delete", key: "n1", removed: ["n1"] },
      { type: "update", key: "t1", fields: { completed: true } },
    ]);
  });

  it("restores a deleted subtree in one step under the same keys and place, so the server can bring back the same todos with their comments", () => {
    const outline = fromTodos([
      todo(1, "home"),
      todo(2, "trip", null, { collapsed: true }),
      todo(3, "packing", 2, { comments: 2 }),
      todo(4, "socks", 3, { done: true }),
    ]);
    const removed = remove(outline, "t2");

    const undo = undoFor(outline, removed.ops);

    expect(undo).toEqual([{ type: "restore", key: "t2", parent: ROOT, index: 1, items: [outline.t2, outline.t3, outline.t4] }]);
    expect(applyOps(removed.outline, undo)).toEqual(outline);
  });

  it("undoes a join by restoring the joined row and its old title, so Backspace at the start of a row can be taken back", () => {
    const outline = fromTodos([todo(1, "buy "), todo(2, "milk")]);
    const joined = mergeIntoPrevious(outline, visibleRows(outline, true), "t2")!;

    const undo = undoFor(outline, joined.ops);

    expect(undo).toEqual([
      { type: "restore", key: "t2", parent: ROOT, index: 1, items: [outline.t2] },
      { type: "title", key: "t1", title: "buy " },
    ]);
    expect(applyOps(joined.outline, undo)).toEqual(outline);
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
});

describe("search", () => {
  const outline = fromTodos([
    todo(1, "Trip to Kyoto"),
    todo(2, "book hotel", 1),
    todo(3, "trip insurance", 1, { done: true }),
    todo(4, "groceries", null, { collapsed: true }),
    todo(5, "trip snacks", 4),
    todo(6, "dentist"),
    todo(7, "pack for the TRIP"),
  ]);
  const rows = visibleRows(outline, false);

  it("marks the rows on screen whose titles contain the words, whatever their case, in the order they appear", () => {
    expect(searchHits(outline, rows, " trip ")).toEqual(["t1", "t7"]);
  });

  it("leaves out matches inside collapsed rows and hidden completed rows, so every match is a row the user can see", () => {
    expect(searchHits(outline, rows, "trip")).not.toContain("t5");
    expect(searchHits(outline, rows, "trip")).not.toContain("t3");
    expect(searchHits(outline, visibleRows(outline, true), "trip")).toEqual(["t1", "t3", "t7"]);
  });

  it("marks nothing while the words are blank, so opening the search leaves the outline as it was", () => {
    expect(searchHits(outline, rows, "  ")).toEqual([]);
  });

  it("steps to the next match below and wraps around to the first after the last", () => {
    const hits = ["t1", "t6", "t7"];
    expect(nextHit(rows, hits, "t1", 1)).toBe("t6");
    expect(nextHit(rows, hits, "t7", 1)).toBe("t1");
  });

  it("steps to the previous match above and wraps around to the last before the first", () => {
    const hits = ["t1", "t6", "t7"];
    expect(nextHit(rows, hits, "t6", -1)).toBe("t1");
    expect(nextHit(rows, hits, "t1", -1)).toBe("t7");
  });

  it("goes on from the row of the current match after it stops matching, so editing it does not send the user back to the top", () => {
    expect(nextHit(rows, ["t1", "t7"], "t6", 1)).toBe("t7");
    expect(nextHit(rows, ["t1", "t7"], "t6", -1)).toBe("t1");
  });

  it("starts from the first match, or the last when going back, when there is no current match on screen", () => {
    expect(nextHit(rows, ["t1", "t7"], null, 1)).toBe("t1");
    expect(nextHit(rows, ["t1", "t7"], "t5", -1)).toBe("t7");
    expect(nextHit(rows, [], null, 1)).toBeNull();
  });
});

describe("guides", () => {
  const outline = fromTodos([
    todo(1, "moving"),
    todo(2, "boxes", 1),
    todo(3, "quotes", 2),
    todo(4, "address", 1),
    todo(5, "bank", 4),
    todo(6, "tax"),
    todo(7, "done step", 6, { done: true }),
  ]);

  it("runs each line from a parent's bullet down to its last child and no further", () => {
    expect(guides(visibleRows(outline, true))).toEqual([
      { through: [], stem: true },
      { through: [true], stem: true },
      { through: [true, false], stem: false },
      { through: [false], stem: true },
      { through: [false, false], stem: false },
      { through: [], stem: true },
      { through: [false], stem: false },
    ]);
  });

  it("draws no stem under a parent whose children are all hidden, so no line leads to nothing", () => {
    const rows = visibleRows(outline, false);
    expect(guides(rows).map(({ stem }, index) => [rows[index].key, stem])).toEqual([
      ["t1", true],
      ["t2", true],
      ["t3", false],
      ["t4", true],
      ["t5", false],
      ["t6", false],
    ]);
  });
});
