import { sql } from "void/db";
import { index, integer, sqliteTable, text } from "void/schema-d1";

export const todos = sqliteTable(
  "todos",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    userId: text("user_id").notNull(),
    parentId: integer("parent_id"),
    position: integer("position").notNull().default(0),
    title: text("title").notNull(),
    icon: text("icon"),
    collapsed: integer("collapsed", { mode: "boolean" }).notNull().default(false),
    completedAt: text("completed_at"),
    createdAt: text("created_at").notNull().default(sql`(datetime('now'))`),
    updatedAt: text("updated_at").notNull().default(sql`(datetime('now'))`),
  },
  (table) => [index("todos_user_id_idx").on(table.userId)],
);

export const todoEvents = sqliteTable(
  "todo_events",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    userId: text("user_id").notNull(),
    todoId: integer("todo_id").notNull(),
    kind: text("kind", { enum: ["created", "completed", "reopened", "deleted"] }).notNull(),
    at: text("at").notNull().default(sql`(datetime('now'))`),
  },
  (table) => [index("todo_events_user_id_at_idx").on(table.userId, table.at), index("todo_events_user_id_todo_id_idx").on(table.userId, table.todoId)],
);

export const todoComments = sqliteTable(
  "todo_comments",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    userId: text("user_id").notNull(),
    todoId: integer("todo_id").notNull(),
    body: text("body").notNull(),
    via: text("via", { enum: ["app", "mcp"] }).notNull(),
    createdAt: text("created_at").notNull().default(sql`(strftime('%Y-%m-%d %H:%M:%f', 'now'))`),
    editedAt: text("edited_at"),
  },
  (table) => [index("todo_comments_user_id_todo_id_idx").on(table.userId, table.todoId, table.createdAt)],
);

export const commentImages = sqliteTable(
  "comment_images",
  {
    id: text("id").primaryKey(),
    userId: text("user_id").notNull(),
    commentId: integer("comment_id"),
    position: integer("position").notNull().default(0),
    createdAt: text("created_at").notNull().default(sql`(strftime('%Y-%m-%d %H:%M:%f', 'now'))`),
  },
  (table) => [index("comment_images_comment_id_created_at_idx").on(table.commentId, table.createdAt)],
);

export const trashedTodos = sqliteTable(
  "trashed_todos",
  {
    id: integer("id").primaryKey(),
    userId: text("user_id").notNull(),
    batch: integer("batch").notNull(),
    parentId: integer("parent_id"),
    position: integer("position").notNull(),
    title: text("title").notNull(),
    icon: text("icon"),
    collapsed: integer("collapsed", { mode: "boolean" }).notNull(),
    completedAt: text("completed_at"),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
    trashedAt: text("trashed_at").notNull().default(sql`(datetime('now'))`),
  },
  (table) => [index("trashed_todos_user_id_batch_idx").on(table.userId, table.batch)],
);

export const webhooks = sqliteTable("webhooks", {
  userId: text("user_id").primaryKey(),
  target: text("target").notNull(),
  hasToken: integer("has_token", { mode: "boolean" }).notNull(),
  headerNames: text("header_names", { mode: "json" }).$type<string[]>().notNull(),
  project: text("project").notNull(),
  lastSentAt: text("last_sent_at"),
  lastError: text("last_error"),
});

export const archivedTodos = sqliteTable(
  "archived_todos",
  {
    id: integer("id").primaryKey(),
    userId: text("user_id").notNull(),
    path: text("path", { mode: "json" }).$type<string[]>().notNull(),
    title: text("title").notNull(),
    createdAt: text("created_at").notNull(),
    completedAt: text("completed_at").notNull(),
  },
  (table) => [index("archived_todos_user_id_completed_at_idx").on(table.userId, table.completedAt)],
);

export const analyticsConsents = sqliteTable("analytics_consents", {
  userId: text("user_id").primaryKey(),
});

export const customIcons = sqliteTable(
  "custom_icons",
  {
    id: text("id").primaryKey(),
    userId: text("user_id").notNull(),
    name: text("name").notNull(),
    createdAt: text("created_at").notNull().default(sql`(strftime('%Y-%m-%d %H:%M:%f', 'now'))`),
  },
  (table) => [index("custom_icons_user_id_created_at_idx").on(table.userId, table.createdAt)],
);

export const profiles = sqliteTable("profiles", {
  userId: text("user_id").primaryKey(),
  name: text("name"),
  avatarId: text("avatar_id"),
});
