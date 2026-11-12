import { MAX_DEPTH, MAX_TITLE_LENGTH } from "./todos";

export const ROOT = "root";

export type Item = {
  key: string;
  title: string;
  note: string;
  done: boolean;
  collapsed: boolean;
  children: string[];
};

export type Outline = Readonly<Record<string, Item>>;

export type Op =
  | { type: "create"; key: string; parent: string; index: number; title: string; note?: string }
  | { type: "title"; key: string; title: string }
  | { type: "note"; key: string; note: string }
  | { type: "update"; key: string; fields: { completed?: boolean; collapsed?: boolean } }
  | { type: "move"; key: string; parent: string; index: number }
  | { type: "delete"; key: string; removed: string[] };

export type Focus = { key: string; pos: number; note?: boolean };

export type Change = { outline: Outline; ops: Op[]; focus?: Focus };

export type Row = { key: string; depth: number; under: boolean };

export type Destination = { key: string; title: string; path: string[] };

type Todo = { id: number; parentId: number | null; title: string; note: string; completedAt: string | null; collapsed: boolean };

export function keyFor(id: number) {
  return `t${id}`;
}

export function fromTodos(todos: readonly Todo[], keyOf: (id: number) => string = keyFor): Outline {
  const outline: Record<string, Item> = { [ROOT]: { key: ROOT, title: "", note: "", done: false, collapsed: false, children: [] } };
  for (const todo of todos) {
    const key = keyOf(todo.id);
    outline[key] = { key, title: todo.title, note: todo.note, done: todo.completedAt !== null, collapsed: todo.collapsed, children: [] };
  }
  for (const todo of todos) {
    const parent = todo.parentId === null ? ROOT : keyOf(todo.parentId);
    outline[parent]?.children.push(keyOf(todo.id));
  }
  return outline;
}

export function splitsPair(text: string, at: number) {
  return at > 0 && /[\uD800-\uDBFF][\uDC00-\uDFFF]/.test(text.slice(at - 1, at + 1));
}

export function caretIn(title: string, pos: number) {
  const at = pos < 0 ? title.length : Math.min(pos, title.length);
  return splitsPair(title, at) ? at + 1 : at;
}

export function parentOf(outline: Outline, key: string) {
  for (const item of Object.values(outline)) {
    if (item.children.includes(key)) return item.key;
  }
  return null;
}

export function subtree(outline: Outline, key: string): string[] {
  return [key, ...outline[key].children.flatMap((child) => subtree(outline, child))];
}

function depthOf(outline: Outline, key: string) {
  let depth = 0;
  for (let current = key; current !== ROOT; current = parentOf(outline, current)!) depth += 1;
  return depth;
}

function heightOf(outline: Outline, key: string): number {
  return outline[key].children.reduce((height, child) => Math.max(height, heightOf(outline, child)), 0) + 1;
}

export function visibleRows(outline: Outline, showDone: boolean, query = "", keep: ReadonlySet<string> = new Set()): Row[] {
  const q = query.trim().toLowerCase();
  const hit = new Set<string>();
  const mark = (key: string): boolean => {
    const { title, note } = outline[key];
    let any = title.toLowerCase().includes(q) || note.toLowerCase().includes(q) || keep.has(key);
    for (const child of outline[key].children) if (mark(child)) any = true;
    if (any) hit.add(key);
    return any;
  };
  if (q) outline[ROOT].children.forEach(mark);
  const rows: Row[] = [];
  const walk = (key: string, depth: number, under: boolean) => {
    for (const child of outline[key].children) {
      const item = outline[child];
      if (!showDone && item.done) continue;
      if (q && !hit.has(child)) continue;
      rows.push({ key: child, depth, under });
      if (q || !item.collapsed) walk(child, depth + 1, under || item.done);
    }
  };
  walk(ROOT, 0, false);
  return rows;
}

export type Guide = { through: boolean[]; stem: boolean };

export function guides(rows: readonly Row[]): Guide[] {
  const followed = new Array<boolean>(rows.length);
  const later: boolean[] = [];
  for (let i = rows.length - 1; i >= 0; i--) {
    const { depth } = rows[i];
    followed[i] = later[depth] ?? false;
    later.length = depth;
    later[depth] = true;
  }
  const open: boolean[] = [];
  return rows.map((row, i) => {
    open.length = row.depth;
    open[row.depth] = followed[i];
    return { through: open.slice(1), stem: (rows[i + 1]?.depth ?? 0) > row.depth };
  });
}

function shownSibling(siblings: readonly string[], from: number, step: number, shown: ReadonlySet<string>) {
  for (let i = from + step; i >= 0 && i < siblings.length; i += step) {
    if (shown.has(siblings[i])) return i;
  }
  return -1;
}

function edit(outline: Outline, keys: string[]) {
  const next: Record<string, Item> = { ...outline };
  for (const key of keys) next[key] = { ...outline[key], children: [...outline[key].children] };
  return next;
}

export function rename(outline: Outline, key: string, title: string): Outline {
  return { ...outline, [key]: { ...outline[key], title } };
}

export function setNote(outline: Outline, key: string, note: string): Outline {
  return { ...outline, [key]: { ...outline[key], note } };
}

export function split(outline: Outline, key: string, pos: number, fresh: string, shown: ReadonlySet<string>): Change {
  const parent = parentOf(outline, key)!;
  const item = outline[key];
  if (pos === 0 && item.title !== "") {
    const next = edit(outline, [parent]);
    const index = next[parent].children.indexOf(key);
    next[fresh] = { key: fresh, title: "", note: "", done: false, collapsed: false, children: [] };
    next[parent].children.splice(index, 0, fresh);
    return { outline: next, ops: [{ type: "create", key: fresh, parent, index, title: "" }], focus: { key, pos: 0 } };
  }
  const before = item.title.slice(0, pos);
  const after = item.title.slice(pos);
  const intoChildren = after === "" && !item.collapsed && item.children.some((child) => shown.has(child));
  const target = intoChildren ? key : parent;
  const next = edit(outline, [key, target]);
  next[key].title = before;
  next[fresh] = { key: fresh, title: after, note: "", done: false, collapsed: false, children: [] };
  const index = intoChildren ? 0 : next[parent].children.indexOf(key) + 1;
  next[target].children.splice(index, 0, fresh);
  const ops: Op[] = [{ type: "create", key: fresh, parent: target, index, title: after }];
  if (after !== "") ops.push({ type: "title", key, title: before });
  return { outline: next, ops, focus: { key: fresh, pos: 0 } };
}

export function indent(outline: Outline, key: string, shown: ReadonlySet<string>): Change | null {
  const parent = parentOf(outline, key)!;
  const siblings = outline[parent].children;
  const i = siblings.indexOf(key);
  const j = shownSibling(siblings, i, -1, shown);
  if (j < 0) return null;
  const target = siblings[j];
  if (depthOf(outline, target) + heightOf(outline, key) > MAX_DEPTH) return null;
  const next = edit(outline, [parent, target]);
  next[parent].children.splice(i, 1);
  const index = next[target].children.length;
  next[target].children.push(key);
  const ops: Op[] = [{ type: "move", key, parent: target, index }];
  if (next[target].collapsed) {
    next[target].collapsed = false;
    ops.push({ type: "update", key: target, fields: { collapsed: false } });
  }
  return { outline: next, ops };
}

export function outdent(outline: Outline, key: string): Change | null {
  const parent = parentOf(outline, key)!;
  if (parent === ROOT) return null;
  const grandparent = parentOf(outline, parent)!;
  const next = edit(outline, [parent, grandparent]);
  next[parent].children.splice(next[parent].children.indexOf(key), 1);
  const index = next[grandparent].children.indexOf(parent) + 1;
  next[grandparent].children.splice(index, 0, key);
  return { outline: next, ops: [{ type: "move", key, parent: grandparent, index }] };
}

export function moveBy(outline: Outline, key: string, step: number, shown: ReadonlySet<string>): Change | null {
  const parent = parentOf(outline, key)!;
  const siblings = outline[parent].children;
  const from = siblings.indexOf(key);
  const index = shownSibling(siblings, from, step, shown);
  if (index < 0) return null;
  const next = edit(outline, [parent]);
  next[parent].children.splice(from, 1);
  next[parent].children.splice(index, 0, key);
  return { outline: next, ops: [{ type: "move", key, parent, index }] };
}

export function destinations(outline: Outline, key: string, showDone: boolean, query: string): Destination[] {
  const q = query.trim().toLowerCase();
  const found: Destination[] = [];
  const height = heightOf(outline, key);
  const walk = (parent: string, path: string[]) => {
    for (const child of outline[parent].children) {
      const { title, done } = outline[child];
      if (child === key || (!showDone && done) || path.length + 1 + height > MAX_DEPTH) continue;
      if (title.trim() !== "" && title.toLowerCase().includes(q)) found.push({ key: child, title, path });
      walk(child, [...path, title]);
    }
  };
  walk(ROOT, []);
  return found;
}

export function moveTo(outline: Outline, key: string, target: string): Change {
  const parent = parentOf(outline, key)!;
  const next = edit(outline, [parent, target]);
  next[parent].children.splice(next[parent].children.indexOf(key), 1);
  const index = next[target].children.length;
  next[target].children.push(key);
  return { outline: next, ops: [{ type: "move", key, parent: target, index }] };
}

export function toggleDone(outline: Outline, key: string): Change {
  const done = !outline[key].done;
  return {
    outline: { ...outline, [key]: { ...outline[key], done } },
    ops: [{ type: "update", key, fields: { completed: done } }],
  };
}

export function setCollapsed(outline: Outline, key: string, collapsed: boolean): Change | null {
  if (outline[key].children.length === 0 || outline[key].collapsed === collapsed) return null;
  return {
    outline: { ...outline, [key]: { ...outline[key], collapsed } },
    ops: [{ type: "update", key, fields: { collapsed } }],
  };
}

export function remove(outline: Outline, key: string): Change {
  const parent = parentOf(outline, key)!;
  const removed = subtree(outline, key);
  const next = edit(outline, [parent]);
  next[parent].children.splice(next[parent].children.indexOf(key), 1);
  for (const gone of removed) delete next[gone];
  return { outline: next, ops: [{ type: "delete", key, removed }] };
}

export function mergeIntoPrevious(outline: Outline, rows: readonly Row[], key: string): Change | null {
  const i = rows.findIndex((row) => row.key === key);
  const previous = rows[i - 1];
  const item = outline[key];
  if (!previous || item.children.length > 0 || previous.depth > rows[i].depth) return null;
  const target = outline[previous.key];
  const title = target.title + item.title;
  if (item.done || target.done || item.note !== "" || title.length > MAX_TITLE_LENGTH) return null;
  const removed = remove(outline, key);
  return {
    outline: rename(removed.outline, previous.key, title),
    ops: [{ type: "title", key: previous.key, title }, ...removed.ops],
    focus: { key: previous.key, pos: target.title.length },
  };
}

export function append(outline: Outline, parent: string, fresh: string): Change {
  const next = edit(outline, [parent]);
  const index = next[parent].children.length;
  next[fresh] = { key: fresh, title: "", note: "", done: false, collapsed: false, children: [] };
  next[parent].children.push(fresh);
  return { outline: next, ops: [{ type: "create", key: fresh, parent, index, title: "" }], focus: { key: fresh, pos: 0 } };
}

function applyOp(outline: Outline, op: Op): Outline {
  switch (op.type) {
    case "create": {
      const next = edit(outline, [op.parent]);
      next[op.key] = { key: op.key, title: op.title, note: op.note ?? "", done: false, collapsed: false, children: [] };
      next[op.parent].children.splice(op.index, 0, op.key);
      return next;
    }
    case "title":
      return rename(outline, op.key, op.title);
    case "note":
      return setNote(outline, op.key, op.note);
    case "update": {
      const item = outline[op.key];
      const { completed = item.done, collapsed = item.collapsed } = op.fields;
      return { ...outline, [op.key]: { ...item, done: completed, collapsed } };
    }
    case "move": {
      const parent = parentOf(outline, op.key)!;
      const next = edit(outline, [parent]);
      next[parent].children.splice(next[parent].children.indexOf(op.key), 1);
      next[op.parent] = { ...next[op.parent], children: [...next[op.parent].children] };
      next[op.parent].children.splice(op.index, 0, op.key);
      return next;
    }
    case "delete":
      return remove(outline, op.key).outline;
  }
}

export function applyOps(outline: Outline, ops: readonly Op[]): Outline {
  return ops.reduce(applyOp, outline);
}

function recreate(outline: Outline, key: string): Op[] {
  const parent = parentOf(outline, key)!;
  const { title, note, done, collapsed } = outline[key];
  const ops: Op[] = [{ type: "create", key, parent, index: outline[parent].children.indexOf(key), title, note }];
  if (done || collapsed) ops.push({ type: "update", key, fields: { ...(done && { completed: true }), ...(collapsed && { collapsed: true }) } });
  return ops;
}

function inverse(outline: Outline, op: Op): Op[] {
  switch (op.type) {
    case "create":
      return [{ type: "delete", key: op.key, removed: [op.key] }];
    case "title":
      return [{ type: "title", key: op.key, title: outline[op.key].title }];
    case "note":
      return [{ type: "note", key: op.key, note: outline[op.key].note }];
    case "update": {
      const item = outline[op.key];
      const fields = {
        ...(op.fields.completed !== undefined && { completed: item.done }),
        ...(op.fields.collapsed !== undefined && { collapsed: item.collapsed }),
      };
      return [{ type: "update", key: op.key, fields }];
    }
    case "move": {
      const parent = parentOf(outline, op.key)!;
      return [{ type: "move", key: op.key, parent, index: outline[parent].children.indexOf(op.key) }];
    }
    case "delete":
      return op.removed.flatMap((key) => recreate(outline, key));
  }
}

export function undoFor(outline: Outline, ops: readonly Op[]): Op[] {
  const steps: Op[][] = [];
  let current = outline;
  for (const op of ops) {
    steps.unshift(inverse(current, op));
    current = applyOp(current, op);
  }
  return steps.flat();
}

export function sameOutline(a: Outline, b: Outline) {
  const keys = Object.keys(a);
  if (keys.length !== Object.keys(b).length) return false;
  return keys.every((key) => {
    const x = a[key];
    const y = b[key];
    return (
      y !== undefined &&
      x.title === y.title &&
      x.note === y.note &&
      x.done === y.done &&
      x.collapsed === y.collapsed &&
      x.children.length === y.children.length &&
      x.children.every((child, i) => child === y.children[i])
    );
  });
}
