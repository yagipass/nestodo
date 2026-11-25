import { env } from "cloudflare:workers";
import { HTTPException } from "hono/http-exception";
import { and, db, eq, gt, gte, inArray, isNotNull, isNull, lte, ne, notInArray, sql } from "void/db";
import { archivedTodos, todoComments, todoEvents, todos, trashedTodos } from "@schema";
import { hasImageIcon } from "./image-icon-service";
import { DAY, inUtc, sqliteTime } from "./time";
import { imageIdOf } from "./todo-icon";
import { awaitingCommentWindow, clampIndex, depthOf, heightOf, isWithin, KEEP_COMPLETED_DAYS, MAX_COMMENT_LENGTH, MAX_DEPTH, outlineOrder, type Placed } from "./todos";

export type TodoChanges = {
  title?: string;
  icon?: string | null;
  completed?: boolean;
  collapsed?: boolean;
  parentId?: number | null;
  index?: number;
};

export type Via = (typeof todoComments.via.enumValues)[number];

const commentCount = sql<number>`(SELECT count(*) FROM todo_comments WHERE todo_comments.user_id = todos.user_id AND todo_comments.todo_id = todos.id)`;

const insertColumns = {
  id: todos.id,
  parentId: todos.parentId,
  position: todos.position,
  title: todos.title,
  icon: todos.icon,
  completedAt: todos.completedAt,
  collapsed: todos.collapsed,
  createdAt: todos.createdAt,
};

const columns = { ...insertColumns, commentCount };

type Todo = Placed & { title: string; icon: string | null; completedAt: string | null; collapsed: boolean; createdAt: string; commentCount: number };

const now = sql`(datetime('now'))`;

function rejected(status: 400 | 409, error: string, message: string) {
  return new HTTPException(status, { res: Response.json({ error, message }, { status }) });
}

function invalidPlacement(message: string) {
  return rejected(400, "invalid_placement", message);
}

async function assertIcon(userId: string, icon: string | null | undefined) {
  const id = imageIdOf(icon ?? null);
  if (id !== null && !(await hasImageIcon(userId, id))) throw rejected(400, "invalid_icon", "Icon not found");
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
  "SELECT id, parent_id AS parentId, position, title, icon, completed_at AS completedAt, collapsed, created_at AS createdAt, (SELECT count(*) FROM todo_comments WHERE todo_comments.user_id = todos.user_id AND todo_comments.todo_id = todos.id) AS commentCount FROM todos WHERE user_id = ?";

const todoSql = `${todosSql} AND id = ?`;

const placementsSql = "SELECT id, parent_id AS parentId, position FROM todos WHERE user_id = ?";

const trashedSql = "SELECT id, parent_id AS parentId, position FROM trashed_todos WHERE user_id = ? AND batch = ?";

const inAwaitingWindow = "(created_at > ? AND created_at <= ?)";

const webhookCandidatesSql = `SELECT id, parent_id AS parentId, position, CASE WHEN parent_id IS NULL OR ${inAwaitingWindow} THEN title ELSE '' END AS title, completed_at AS completedAt, created_at AS createdAt FROM todos WHERE user_id = ?`;

const awaitingCommentsSql =
  "SELECT todo.id AS todoId, comment.body FROM todos AS todo JOIN todo_comments AS comment ON comment.user_id = todo.user_id AND comment.todo_id = todo.id WHERE todo.user_id = ? AND todo.created_at > ? AND todo.created_at <= ? ORDER BY comment.id";

const timelineSql = `SELECT 'event' AS type, 'created' AS kind, NULL AS id, NULL AS body, NULL AS via, created_at AS at, NULL AS editedAt, 0 AS rank, 0 AS seq FROM todos WHERE user_id = ?1 AND id = ?2
UNION ALL
SELECT 'event', kind, NULL, NULL, NULL, at, NULL, 1, id FROM todo_events WHERE user_id = ?1 AND todo_id = ?2 AND kind IN ('completed', 'reopened')
UNION ALL
SELECT 'comment', NULL, id, body, via, created_at, edited_at, 2, id FROM todo_comments WHERE user_id = ?1 AND todo_id = ?2
ORDER BY at, rank, seq`;

const searchSql = `SELECT comment.todo_id AS id, comment.body AS comment, min(comment.id) AS first
FROM todos AS todo CROSS JOIN todo_comments AS comment ON comment.user_id = todo.user_id AND comment.todo_id = todo.id
WHERE todo.user_id = ? AND comment.body LIKE ? ESCAPE '\\'
GROUP BY comment.todo_id`;

function fromRow(row: TodoRow): Todo {
  return { ...row, collapsed: row.collapsed !== 0 };
}

function primary() {
  return env.DB.withSession("first-primary");
}

async function placementsOf(userId: string): Promise<Placed[]> {
  const { results } = await primary().prepare(placementsSql).bind(userId).all<Placed>();
  return results;
}

async function exists(userId: string, id: number) {
  return (await primary().prepare("SELECT 1 AS found FROM todos WHERE user_id = ? AND id = ?").bind(userId, id).first()) !== null;
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
  const { results } = await primary().prepare(todosSql).bind(userId).all<TodoRow>();
  return outlineOrder(results.map(fromRow));
}

export async function webhookCandidatesOf(userId: string, now: Date) {
  const { since, until } = awaitingCommentWindow(now);
  const session = primary();
  const { results: candidates } = await session
    .prepare(webhookCandidatesSql)
    .bind(since, until, userId)
    .all<Placed & { title: string; completedAt: string | null; createdAt: string }>();
  const { results: comments } = await session.prepare(awaitingCommentsSql).bind(userId, since, until).all<{ todoId: number; body: string }>();
  const bodies = new Map<number, string[]>();
  for (const { todoId, body } of comments) bodies.set(todoId, [...(bodies.get(todoId) ?? []), body]);
  return outlineOrder(candidates.map((row) => ({ ...row, comments: bodies.get(row.id) ?? [] })));
}

export async function getTodo(userId: string, id: number) {
  const row = await primary().prepare(todoSql).bind(userId, id).first<TodoRow>();
  return row ? fromRow(row) : null;
}

export async function createTodo(
  userId: string,
  input: { title: string; icon?: string | null; parentId?: number | null; index?: number; comment?: string },
  via: Via = "app",
): Promise<Todo> {
  const parentId = input.parentId ?? null;
  await assertIcon(userId, input.icon);
  const rows = await placementsOf(userId);
  assertParent(rows, parentId);
  assertDepth(rows, parentId, 1);
  const position = clampIndex(input.index, rows.filter((row) => row.parentId === parentId).length);
  const gap = openGap(userId, parentId, position);
  const insert = db.insert(todos).values({ userId, title: input.title, icon: input.icon ?? null, parentId, position }).returning(insertColumns);
  const comment = input.comment === undefined ? null : db.insert(todoComments).values({ userId, todoId: sql`last_insert_rowid()`, body: input.comment, via });
  const reopen = parentId === null ? null : reopenAbove(userId, parentId, false);
  const [, [created]] =
    comment && reopen
      ? await db.batch([gap, insert, comment, reopen])
      : comment
        ? await db.batch([gap, insert, comment])
        : reopen
          ? await db.batch([gap, insert, reopen])
          : await db.batch([gap, insert]);
  return { ...created, commentCount: comment ? 1 : 0 };
}

export async function updateTodo(userId: string, id: number, changes: TodoChanges) {
  await assertIcon(userId, changes.icon);
  const completedAt =
    changes.completed === undefined ? undefined : changes.completed ? sql`coalesce(${todos.completedAt}, datetime('now'))` : null;
  const fields = { title: changes.title, icon: changes.icon, collapsed: changes.collapsed, completedAt, updatedAt: now };
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

  const rows = await placementsOf(userId);
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
  const rows = await placementsOf(userId);
  const target = rows.find((row) => row.id === id);
  if (!target) return false;
  if (!recursive && rows.some((row) => row.parentId === id)) {
    throw rejected(409, "has_children", "Todo has todos under it");
  }
  await db.batch([
    db.insert(trashedTodos).select(sql`
      SELECT id, user_id, ${id}, parent_id, position, title, icon, collapsed, completed_at, created_at, updated_at, datetime('now')
      FROM ${todos} WHERE user_id = ${userId} AND id IN ${subtreeOf(userId, id)}
    `),
    db.delete(todos).where(and(eq(todos.userId, userId), inArray(todos.id, subtreeOf(userId, id)))),
    closeGap(userId, target),
  ]);
  return true;
}

export async function restoreTodo(userId: string, id: number, placement: { parentId?: number | null; index?: number }) {
  const { results: trashed } = await primary().prepare(trashedSql).bind(userId, id).all<Placed>();
  const root = trashed.find((row) => row.id === id);
  if (!root) return null;
  const parentId = placement.parentId === undefined ? root.parentId : placement.parentId;
  const rows = await placementsOf(userId);
  assertParent(rows, parentId);
  assertDepth(rows, parentId, heightOf(trashed, id));
  const position = clampIndex(placement.index ?? root.position, rows.filter((row) => row.parentId === parentId).length);
  const restored = sql`(SELECT id FROM ${trashedTodos} WHERE user_id = ${userId} AND batch = ${id})`;
  await db.batch([
    openGap(userId, parentId, position),
    db.insert(todos).select(sql`
      SELECT id, user_id,
        CASE WHEN id = ${id} THEN ${parentId} ELSE parent_id END,
        CASE WHEN id = ${id} THEN ${position} ELSE position END,
        title, icon, collapsed, completed_at, created_at, updated_at
      FROM ${trashedTodos} WHERE user_id = ${userId} AND batch = ${id}
    `),
    db.delete(todoEvents).where(and(eq(todoEvents.userId, userId), eq(todoEvents.kind, "deleted"), inArray(todoEvents.todoId, restored))),
    db.delete(trashedTodos).where(and(eq(trashedTodos.userId, userId), eq(trashedTodos.batch, id))),
    reopenAbove(userId, id, true),
  ]);
  return getTodo(userId, id);
}

type TimelineRow = {
  type: "event" | "comment";
  kind: "created" | "completed" | "reopened" | null;
  id: number | null;
  body: string | null;
  via: Via | null;
  at: string;
  editedAt: string | null;
};

export type TimelineItem =
  | { type: "event"; kind: "created" | "completed" | "reopened"; at: string }
  | { type: "comment"; id: number; body: string; via: Via; at: string; editedAt: string | null };

function presentComment(row: { id: number; body: string; via: Via; at: string; editedAt: string | null }) {
  return { id: row.id, body: row.body, via: row.via, at: inUtc(row.at), editedAt: row.editedAt && inUtc(row.editedAt) };
}

export async function timelineOf(userId: string, id: number): Promise<TimelineItem[] | null> {
  const { results } = await primary().prepare(timelineSql).bind(userId, id).all<TimelineRow>();
  if (results.length === 0) return null;
  return results.map((row) =>
    row.type === "event"
      ? { type: "event", kind: row.kind!, at: inUtc(row.at) }
      : { type: "comment", ...presentComment({ id: row.id!, body: row.body!, via: row.via!, at: row.at, editedAt: row.editedAt }) },
  );
}

const commentColumns = {
  id: todoComments.id,
  body: todoComments.body,
  via: todoComments.via,
  at: todoComments.createdAt,
  editedAt: todoComments.editedAt,
};

export async function addComment(userId: string, todoId: number, body: string, via: Via) {
  if (!(await exists(userId, todoId))) return null;
  const [comment] = await db.insert(todoComments).values({ userId, todoId, body, via }).returning(commentColumns);
  return presentComment(comment);
}

function onListedTodo(userId: string, id: number) {
  return and(
    eq(todoComments.id, id),
    eq(todoComments.userId, userId),
    inArray(todoComments.todoId, db.select({ id: todos.id }).from(todos).where(eq(todos.userId, userId))),
  );
}

export async function editComment(userId: string, id: number, body: string) {
  const [comment] = await db
    .update(todoComments)
    .set({ body, editedAt: sql`CASE WHEN ${todoComments.body} = ${body} THEN ${todoComments.editedAt} ELSE strftime('%Y-%m-%d %H:%M:%f', 'now') END` })
    .where(onListedTodo(userId, id))
    .returning(commentColumns);
  return comment ? presentComment(comment) : null;
}

export async function deleteComment(userId: string, id: number) {
  const deleted = await db
    .delete(todoComments)
    .where(onListedTodo(userId, id))
    .returning({ id: todoComments.id });
  return deleted.length > 0;
}

export async function searchComments(userId: string, query: string) {
  const text = query.trim();
  if (text === "" || text.length > MAX_COMMENT_LENGTH) return [];
  const pattern = `%${text.replace(/[\\%_]/g, (char) => `\\${char}`)}%`;
  const { results } = await primary().prepare(searchSql).bind(userId, pattern).all<{ id: number; comment: string }>();
  return results.map(({ id, comment }) => ({ id, comment }));
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
  const expired = sql`(SELECT id FROM ${trashedTodos} WHERE user_id = ${userId} AND trashed_at <= ${cutoff})`;
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
      SELECT todo.id, todo.user_id, ancestry.path, todo.title, todo.created_at, todo.completed_at
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
    db.delete(todoComments).where(and(eq(todoComments.userId, userId), inArray(todoComments.todoId, expired))),
    db.delete(trashedTodos).where(and(eq(trashedTodos.userId, userId), lte(trashedTodos.trashedAt, cutoff))),
  ]);
}

export async function purgeCompletedTodos(now: Date) {
  const cutoff = sqliteTime(now.getTime() - KEEP_COMPLETED_DAYS * DAY);
  const { results: members } = await primary()
    .prepare("SELECT user_id AS userId FROM todos UNION SELECT user_id FROM trashed_todos")
    .all<{ userId: string }>();
  const results = await Promise.allSettled(members.map(({ userId }) => purgeCompletedTodosOf(userId, cutoff)));
  const failures = results.flatMap((result) => (result.status === "rejected" ? [String(result.reason)] : []));
  if (failures.length > 0) throw new Error(`Cleanup failed for ${failures.length} of ${members.length} members: ${failures.join(", ")}`);
}
