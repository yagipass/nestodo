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
    note: text("note").notNull().default(""),
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
  (table) => [index("todo_events_user_id_at_idx").on(table.userId, table.at)],
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
    note: text("note").notNull(),
    createdAt: text("created_at").notNull(),
    completedAt: text("completed_at").notNull(),
  },
  (table) => [index("archived_todos_user_id_completed_at_idx").on(table.userId, table.completedAt)],
);

export const analyticsConsents = sqliteTable("analytics_consents", {
  userId: text("user_id").primaryKey(),
});

export const profiles = sqliteTable("profiles", {
  userId: text("user_id").primaryKey(),
  name: text("name"),
  avatarId: text("avatar_id"),
});
