import { env } from "cloudflare:workers";
import { HTTPException } from "hono/http-exception";
import { and, db, eq, gt, gte, inArray, isNotNull, isNull, ne, notInArray, sql } from "void/db";
import { archivedTodos, todos } from "@schema";
import { DAY, sqliteTime } from "./time";
import { awaitingNoteWindow, clampIndex, depthOf, heightOf, isWithin, KEEP_COMPLETED_DAYS, MAX_DEPTH, outlineOrder, type Placed } from "./todos";

export type TodoChanges = {
  title?: string;
  note?: string;
  completed?: boolean;
  collapsed?: boolean;
  parentId?: number | null;
  index?: number;
};

const columns = {
  id: todos.id,
  parentId: todos.parentId,
  position: todos.position,
  title: todos.title,
  note: todos.note,
  completedAt: todos.completedAt,
  collapsed: todos.collapsed,
  createdAt: todos.createdAt,
};

type Todo = Placed & { title: string; note: string; completedAt: string | null; collapsed: boolean; createdAt: string };

const now = sql`(datetime('now'))`;

function rejected(status: 400 | 409, error: string, message: string) {
  return new HTTPException(status, { res: Response.json({ error, message }, { status }) });
}

function invalidPlacement(message: string) {
  return rejected(400, "invalid_placement", message);
}

function ownedBy(userId: string, id: number) {
  return and(eq(todos.id, id), eq(todos.userId, userId));
}

function siblingsUnder(userId: string, parentId: number | null) {
  return and(eq(todos.userId, userId), parentId === null ? isNull(todos.parentId) : eq(todos.parentId, parentId));
}

function openGap(userId: string, parentId: number | null, position: number) {
  return db
    .update(todos)
    .set({ position: sql`${todos.position} + 1` })
    .where(and(siblingsUnder(userId, parentId), gte(todos.position, position)));
}

function closeGap(userId: string, { parentId, position }: Placed) {
  return db
    .update(todos)
    .set({ position: sql`${todos.position} - 1` })
    .where(and(siblingsUnder(userId, parentId), gt(todos.position, position)));
}

function subtreeOf(userId: string, id: number) {
  return sql`(
    WITH RECURSIVE subtree(id) AS (
      VALUES (${id})
      UNION
      SELECT child.id FROM ${todos} AS child JOIN subtree ON child.parent_id = subtree.id WHERE child.user_id = ${userId}
    )
    SELECT id FROM subtree
  )`;
}

function lineageOf(userId: string, id: number) {
  return sql`(
    WITH RECURSIVE lineage(id) AS (
      VALUES (${id})
      UNION
      SELECT todo.parent_id FROM ${todos} AS todo JOIN lineage ON todo.id = lineage.id WHERE todo.user_id = ${userId} AND todo.parent_id IS NOT NULL
    )
    SELECT id FROM lineage
  )`;
}

function completeUnder(userId: string, id: number) {
  return db
    .update(todos)
    .set({ completedAt: now, updatedAt: now })
    .where(and(eq(todos.userId, userId), isNull(todos.completedAt), ne(todos.id, id), inArray(todos.id, subtreeOf(userId, id))))
    .returning({ id: todos.id });
}

function reopenAbove(userId: string, id: number, onlyIfOpenBelow: boolean) {
  const open = sql`EXISTS (SELECT 1 FROM ${todos} AS below WHERE below.user_id = ${userId} AND below.completed_at IS NULL AND below.id IN ${subtreeOf(userId, id)})`;
  return db
    .update(todos)
    .set({ completedAt: null, updatedAt: now })
    .where(and(eq(todos.userId, userId), isNotNull(todos.completedAt), inArray(todos.id, lineageOf(userId, id)), onlyIfOpenBelow ? open : undefined))
    .returning({ id: todos.id });
}

type TodoRow = Omit<Todo, "collapsed"> & { collapsed: number };

const todosSql =
  "SELECT id, parent_id AS parentId, position, title, note, completed_at AS completedAt, collapsed, created_at AS createdAt FROM todos WHERE user_id = ?";

const todoSql = `${todosSql} AND id = ?`;

const inAwaitingWindow = "(created_at > ? AND created_at <= ?)";

const webhookCandidatesSql = `SELECT id, parent_id AS parentId, position, CASE WHEN parent_id IS NULL OR ${inAwaitingWindow} THEN title ELSE '' END AS title, CASE WHEN ${inAwaitingWindow} THEN note ELSE '' END AS note, completed_at AS completedAt, collapsed, created_at AS createdAt FROM todos WHERE user_id = ?`;

function fromRow(row: TodoRow): Todo {
  return { ...row, collapsed: row.collapsed !== 0 };
}

async function todosOf(userId: string): Promise<Todo[]> {
  const { results } = await env.DB.withSession("first-primary").prepare(todosSql).bind(userId).all<TodoRow>();
  return results.map(fromRow);
}

function assertParent(rows: readonly Placed[], parentId: number | null) {
  if (parentId !== null && !rows.some((row) => row.id === parentId)) throw invalidPlacement("Parent todo not found");
}

function assertDepth(rows: readonly Placed[], parentId: number | null, height: number) {
  if (parentId !== null && depthOf(rows, parentId) + height > MAX_DEPTH) {
    throw invalidPlacement(`Todos cannot be nested more than ${MAX_DEPTH} levels deep`);
  }
}

export async function listTodos(userId: string) {
  return outlineOrder(await todosOf(userId));
}

export async function webhookCandidatesOf(userId: string, now: Date) {
  const { since, until } = awaitingNoteWindow(now);
  const { results } = await env.DB.withSession("first-primary")
    .prepare(webhookCandidatesSql)
    .bind(since, until, since, until, userId)
    .all<TodoRow>();
  return outlineOrder(results.map(fromRow));
}

export async function getTodo(userId: string, id: number) {
  const row = await env.DB.withSession("first-primary").prepare(todoSql).bind(userId, id).first<TodoRow>();
  return row ? fromRow(row) : null;
}

export async function createTodo(userId: string, input: { title: string; note?: string; parentId?: number | null; index?: number }) {
  const parentId = input.parentId ?? null;
  const rows = await todosOf(userId);
  assertParent(rows, parentId);
  assertDepth(rows, parentId, 1);
  const position = clampIndex(input.index, rows.filter((row) => row.parentId === parentId).length);
  const insert = db.insert(todos).values({ userId, title: input.title, note: input.note, parentId, position }).returning(columns);
  if (parentId === null) {
    const [, [created]] = await db.batch([openGap(userId, parentId, position), insert]);
    return created;
  }
  const [, [created]] = await db.batch([openGap(userId, parentId, position), insert, reopenAbove(userId, parentId, false)]);
  return created;
}

export async function updateTodo(userId: string, id: number, changes: TodoChanges) {
  const completedAt =
    changes.completed === undefined ? undefined : changes.completed ? sql`coalesce(${todos.completedAt}, datetime('now'))` : null;
  const fields = { title: changes.title, note: changes.note, collapsed: changes.collapsed, completedAt, updatedAt: now };
  const update = (placement: { parentId?: number | null; position?: number } = {}) =>
    db
      .update(todos)
      .set({ ...fields, ...placement })
      .where(ownedBy(userId, id))
      .returning(columns);
  const settle = (moved: boolean) =>
    changes.completed === true ? completeUnder(userId, id) : changes.completed === false || moved ? reopenAbove(userId, id, true) : null;
  const result = (row: Todo | undefined, completed: { id: number }[] = []) =>
    row ? { todo: row, completedUnder: changes.completed === true ? completed.map((todo) => todo.id) : [] } : null;
  const updateInPlace = async () => {
    const after = settle(false);
    if (!after) return result((await update())[0]);
    const [[row], completed] = await db.batch([update(), after]);
    return result(row, completed);
  };

  if (changes.parentId === undefined && changes.index === undefined) return updateInPlace();

  const rows = await todosOf(userId);
  const current = rows.find((row) => row.id === id);
  if (!current) return null;
  const parentId = changes.parentId === undefined ? current.parentId : changes.parentId;
  if (parentId === current.parentId && changes.index === undefined) return updateInPlace();
  assertParent(rows, parentId);
  if (parentId !== null && isWithin(rows, parentId, id)) throw invalidPlacement("A todo cannot be moved under itself");
  if (parentId !== current.parentId) assertDepth(rows, parentId, heightOf(rows, id));
  const position = clampIndex(changes.index, rows.filter((row) => row.parentId === parentId && row.id !== id).length);
  const placed = [closeGap(userId, current), openGap(userId, parentId, position), update({ parentId, position })] as const;
  const after = settle(parentId !== current.parentId);
  if (!after) {
    const [, , [row]] = await db.batch(placed);
    return result(row);
  }
  const [, , [row], completed] = await db.batch([...placed, after]);
  return result(row, completed);
}

export async function deleteTodo(userId: string, id: number, recursive: boolean) {
  const rows = await todosOf(userId);
  const target = rows.find((row) => row.id === id);
  if (!target) return false;
  if (!recursive && rows.some((row) => row.parentId === id)) {
    throw rejected(409, "has_children", "Todo has todos under it");
  }
  await db.batch([
    db.delete(todos).where(and(eq(todos.userId, userId), inArray(todos.id, subtreeOf(userId, id)))),
    closeGap(userId, target),
  ]);
  return true;
}

async function purgeCompletedTodosOf(userId: string, cutoff: string) {
  const kept = sql`(
    WITH RECURSIVE kept(id) AS (
      SELECT id FROM ${todos} WHERE user_id = ${userId} AND (completed_at IS NULL OR completed_at > ${cutoff})
      UNION
      SELECT parent_id FROM ${todos} JOIN kept USING (id) WHERE parent_id IS NOT NULL
    )
    SELECT id FROM kept
  )`;
  const ranked = db
    .select({
      id: todos.id,
      position: sql<number>`row_number() OVER (PARTITION BY ${todos.parentId} ORDER BY ${todos.position}, ${todos.id}) - 1`.as("ranked_position"),
    })
    .from(todos)
    .where(eq(todos.userId, userId))
    .as("ranked");
  await db.batch([
    db.insert(archivedTodos).select(sql`
      WITH RECURSIVE ancestry(id, ancestor_id, path, seen) AS (
        SELECT id, parent_id, json_array(), json_array(id) FROM ${todos} WHERE user_id = ${userId} AND id NOT IN ${kept}
        UNION ALL
        SELECT ancestry.id, ancestor.parent_id, '[' || json_quote(ancestor.title) || iif(ancestry.path = '[]', ']', ',' || substr(ancestry.path, 2)),
          json_insert(ancestry.seen, '$[#]', ancestor.id)
        FROM ancestry JOIN ${todos} AS ancestor ON ancestor.id = ancestry.ancestor_id AND ancestor.user_id = ${userId}
        WHERE ancestor.id NOT IN (SELECT value FROM json_each(ancestry.seen))
      )
      SELECT todo.id, todo.user_id, ancestry.path, todo.title, todo.note, todo.created_at, todo.completed_at
      FROM ancestry JOIN ${todos} AS todo USING (id)
      WHERE NOT EXISTS (SELECT 1 FROM ${todos} WHERE id = ancestry.ancestor_id AND user_id = ${userId})
        OR ancestry.ancestor_id IN (SELECT value FROM json_each(ancestry.seen))
    `),
    db.delete(todos).where(and(eq(todos.userId, userId), notInArray(todos.id, kept))),
    db
      .update(todos)
      .set({ position: sql`${ranked.position}` })
      .from(ranked)
      .where(and(eq(todos.id, ranked.id), ne(todos.position, ranked.position))),
  ]);
}

export async function purgeCompletedTodos(now: Date) {
  const cutoff = sqliteTime(now.getTime() - KEEP_COMPLETED_DAYS * DAY);
  const { results: members } = await env.DB.withSession("first-primary").prepare("SELECT DISTINCT user_id AS userId FROM todos").all<{ userId: string }>();
  const results = await Promise.allSettled(members.map(({ userId }) => purgeCompletedTodosOf(userId, cutoff)));
  const failures = results.flatMap((result) => (result.status === "rejected" ? [String(result.reason)] : []));
  if (failures.length > 0) throw new Error(`Cleanup failed for ${failures.length} of ${members.length} members: ${failures.join("; ")}`);
}
