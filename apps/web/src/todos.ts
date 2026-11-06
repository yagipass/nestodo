import { MINUTE, sqliteTime } from "./time";

export type Placed = {
  id: number;
  parentId: number | null;
  position: number;
};

export function outlineOrder<T extends Placed>(rows: readonly T[]): T[] {
  const children = new Map<number | null, T[]>();
  for (const row of rows) {
    const siblings = children.get(row.parentId) ?? [];
    siblings.push(row);
    children.set(row.parentId, siblings);
  }
  for (const siblings of children.values()) {
    siblings.sort((a, b) => a.position - b.position || a.id - b.id);
  }
  const ordered: T[] = [];
  const visit = (parentId: number | null) => {
    for (const row of children.get(parentId) ?? []) {
      ordered.push(row);
      visit(row.id);
    }
  };
  visit(null);
  return ordered;
}

export function clampIndex(index: number | undefined, siblingCount: number) {
  return index === undefined ? siblingCount : Math.min(index, siblingCount);
}

export function isWithin(rows: readonly Placed[], id: number, ancestorId: number) {
  const parents = new Map(rows.map((row) => [row.id, row.parentId]));
  const seen = new Set<number>();
  for (let current: number | null | undefined = id; current != null; current = parents.get(current)) {
    if (current === ancestorId) return true;
    if (seen.has(current)) return false;
    seen.add(current);
  }
  return false;
}

export function depthOf(rows: readonly Placed[], id: number) {
  const parents = new Map(rows.map((row) => [row.id, row.parentId]));
  const seen = new Set<number>();
  for (let current: number | null | undefined = id; current != null && !seen.has(current); current = parents.get(current)) seen.add(current);
  return seen.size;
}

export function heightOf(rows: readonly Placed[], id: number) {
  const children = new Map<number, number[]>();
  for (const row of rows) {
    if (row.parentId === null) continue;
    const siblings = children.get(row.parentId) ?? [];
    siblings.push(row.id);
    children.set(row.parentId, siblings);
  }
  const seen = new Set([id]);
  let level = [id];
  let height = 0;
  while (level.length > 0) {
    height += 1;
    const next: number[] = [];
    for (const parent of level) {
      for (const child of children.get(parent) ?? []) {
        if (seen.has(child)) continue;
        seen.add(child);
        next.push(child);
      }
    }
    level = next;
  }
  return height;
}

function reachingAny(rows: readonly Placed[], ancestorIds: ReadonlySet<number>) {
  const parents = new Map(rows.map((row) => [row.id, row.parentId]));
  const reaches = new Map<number, boolean>();
  for (const row of rows) {
    const path = new Set<number>();
    let found = false;
    for (let current: number | null | undefined = row.id; current != null; current = parents.get(current)) {
      const known = reaches.get(current);
      if (known !== undefined) {
        found = known;
        break;
      }
      if (path.has(current)) break;
      path.add(current);
      if (ancestorIds.has(current)) {
        found = true;
        break;
      }
    }
    for (const id of path) reaches.set(id, found);
  }
  return reaches;
}

type Noted = Placed & { title: string; note: string; completedAt: string | null; createdAt: string };

function lacksNote(note: string) {
  const trimmed = note.trim();
  return trimmed === "" || /^https?:\/\/\S+$/i.test(trimmed);
}

export function awaitingNoteWindow(now: Date) {
  return { since: sqliteTime(now.getTime() - 13 * MINUTE), until: sqliteTime(now.getTime() - 3 * MINUTE) };
}

export function todosAwaitingNote<T extends Noted>(rows: readonly T[], rootTitle: string, now: Date): T[] {
  const { since, until } = awaitingNoteWindow(now);
  const parents = new Set(rows.map((row) => row.parentId));
  const roots = new Set(rows.filter((row) => row.parentId === null && row.title.trim() === rootTitle).map((row) => row.id));
  const underRoot = reachingAny(rows, roots);
  return rows.filter(
    (row) =>
      row.parentId !== null &&
      row.completedAt === null &&
      !parents.has(row.id) &&
      since < row.createdAt &&
      row.createdAt <= until &&
      lacksNote(row.note) &&
      row.title.trim() !== "" &&
      underRoot.get(row.id) === true,
  );
}

export const MAX_TITLE_LENGTH = 2000;

export const MAX_NOTE_LENGTH = 10000;

export const MAX_DEPTH = 64;

export const KEEP_COMPLETED_DAYS = 8;

export const SHOW_DONE_COOKIE = "show-completed";
