import { describe, expect, it } from "vite-plus/test";
import { fromTodos, indent, rename, setNote, split, visibleRows, type Outline } from "./outline";
import { UndoHistory } from "./undo";

function todo(id: number, title: string, note = "") {
  return { id, title, note, parentId: null, completedAt: null, collapsed: false };
}

function shown(outline: Outline) {
  return new Set(visibleRows(outline, true).map((row) => row.key));
}

const start = fromTodos([todo(1, "milk"), todo(2, "eggs")]);

function typing(history: UndoHistory) {
  return {
    title(outline: Outline, key: string, title: string, composing = false) {
      const next = rename(outline, key, title);
      history.type(outline, next, key, "title", composing);
      return next;
    },
    note(outline: Outline, key: string, note: string) {
      const next = setNote(outline, key, note);
      history.type(outline, next, key, "note");
      return next;
    },
  };
}

describe("UndoHistory", () => {
  it("takes edits back newest first and returns the cursor to where it was before each one", () => {
    const history = new UndoHistory();
    const added = split(start, "t2", 4, "n1", shown(start));
    history.record(start, added, { key: "t2", pos: 4 });
    const nested = indent(added.outline, "n1", shown(added.outline))!;
    history.record(added.outline, nested, { key: "n1", pos: 0 });

    const unnested = history.undo(nested.outline)!;
    expect(unnested.outline).toEqual(added.outline);
    expect(unnested.ops).toEqual([{ type: "move", key: "n1", parent: "root", index: 2 }]);
    expect(unnested.focus).toEqual({ key: "n1", pos: 0 });

    const removed = history.undo(unnested.outline)!;
    expect(removed.outline).toEqual(start);
    expect(removed.ops).toEqual([{ type: "delete", key: "n1", removed: ["n1"] }]);
    expect(removed.focus).toEqual({ key: "t2", pos: 4 });

    expect(history.undo(removed.outline)).toBeNull();
  });

  it("takes back a burst of typing in one step and starts the next step after a pause of more than a second", () => {
    let now = 0;
    const history = new UndoHistory(() => now);
    const type = typing(history);
    let outline = type.title(start, "t1", "milk!");
    now = 1000;
    outline = type.title(outline, "t1", "milk!!");
    now = 2000;
    outline = type.title(outline, "t1", "milk!!!");
    now = 3001;
    outline = type.title(outline, "t1", "milk!!!?");

    const first = history.undo(outline)!;
    expect(first.ops).toEqual([{ type: "title", key: "t1", title: "milk!!!" }]);
    const second = history.undo(first.outline)!;
    expect(second.ops).toEqual([{ type: "title", key: "t1", title: "milk" }]);
    expect(second.outline).toEqual(start);
  });

  it("keeps an IME conversion in one step even when the user stops to pick a candidate, and splits only on a pause between conversions", () => {
    let now = 0;
    const history = new UndoHistory(() => now);
    const type = typing(history);
    let outline = type.title(start, "t2", "niunai", true);
    now = 3000;
    outline = type.title(outline, "t2", "牛奶", true);
    history.composed();
    now = 3500;
    outline = type.title(outline, "t2", "牛奶和", true);
    history.composed();
    now = 6000;
    outline = type.title(outline, "t2", "牛奶和面包", true);
    history.composed();

    const first = history.undo(outline)!;
    expect(first.outline.t2.title).toBe("牛奶和");
    expect(history.undo(first.outline)!.outline.t2.title).toBe("eggs");
  });

  it("starts a new step for typing after the user leaves the field or makes another edit, so one Cmd+Z never takes back more than that", () => {
    const history = new UndoHistory(() => 0);
    const type = typing(history);
    let outline = type.title(start, "t1", "milk!");
    history.end();
    outline = type.title(outline, "t1", "milk!!");
    const nested = indent(outline, "t2", shown(outline))!;
    history.record(outline, nested);
    outline = type.title(nested.outline, "t1", "milk!!!");

    const titles: string[] = [];
    for (let change = history.undo(outline); change; change = history.undo(change.outline)) titles.push(change.outline.t1.title);
    expect(titles).toEqual(["milk!!", "milk!!", "milk!", "milk"]);
  });

  it("starts a new step for typing right after an undone edit, so the next Cmd+Z takes back only what was typed since", () => {
    const history = new UndoHistory(() => 0);
    const type = typing(history);
    const outline = type.title(start, "t1", "milk!");
    const nested = indent(outline, "t2", shown(outline))!;
    history.record(outline, nested);
    const unnested = history.undo(nested.outline)!;

    const retyped = type.title(unnested.outline, "t1", "milk!!");

    expect(history.undo(retyped)?.outline.t1.title).toBe("milk!");
  });

  it("puts the cursor where the undone typing was, in the note when the note was typed in", () => {
    const history = new UndoHistory(() => 0);
    const type = typing(history);
    const prefixed = type.title(start, "t1", "oat milk");
    history.end();
    const noted = type.note(prefixed, "t2", "free-range");
    history.end();
    const trimmed = type.note(noted, "t2", "free");

    const untrimmed = history.undo(trimmed)!;
    expect(untrimmed.focus).toEqual({ key: "t2", pos: "free-range".length, note: true });
    const unnoted = history.undo(untrimmed.outline)!;
    expect(unnoted.focus).toEqual({ key: "t2", pos: 0, note: true });
    expect(history.undo(unnoted.outline)!.focus).toEqual({ key: "t1", pos: 0, note: false });
  });

  it("puts the cursor back where an emoji was typed in front of another emoji, not after the one left, so the next keystroke lands where the undone one was", () => {
    const history = new UndoHistory(() => 0);
    const thumbsUp = typing(history).title(fromTodos([todo(1, "😀 Dentist")]), "t1", "👍😀 Dentist");

    expect(history.undo(thumbsUp)!.focus).toEqual({ key: "t1", pos: 0, note: false });
  });

  it("puts the cursor after an emoji whose replacement is undone, never between its two halves, where the next keystroke would break it", () => {
    const history = new UndoHistory(() => 0);
    const pleading = typing(history).title(fromTodos([todo(1, "Dance 🕺")]), "t1", "Dance 🥺");

    expect(history.undo(pleading)!.focus).toEqual({ key: "t1", pos: "Dance 🕺".length, note: false });
  });

  it("keeps every step when reloading brings back the same outline, so switching tabs does not cost the user their undo", () => {
    const history = new UndoHistory();
    const added = split(start, "t2", 4, "n1", shown(start));
    history.record(start, added);
    const reloaded = fromTodos([todo(1, "milk"), todo(2, "eggs"), todo(3, "")], (id) => (id === 3 ? "n1" : `t${id}`));

    expect(history.undo(reloaded)?.outline).toEqual(start);
  });

  it("forgets every step once the outline was changed elsewhere, so undo never overwrites an edit made on another device", () => {
    const history = new UndoHistory();
    const added = split(start, "t2", 4, "n1", shown(start));
    history.record(start, added);
    const nested = indent(added.outline, "n1", shown(added.outline))!;
    history.record(added.outline, nested);
    const renamedOnPhone = rename(nested.outline, "t1", "oat milk");

    expect(history.undo(renamedOnPhone)).toBeNull();
    expect(history.undo(nested.outline)).toBeNull();
  });
});
