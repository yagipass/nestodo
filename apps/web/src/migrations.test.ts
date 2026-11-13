import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { describe, expect, it } from "vite-plus/test";

const dir = join(import.meta.dirname, "..", "db", "migrations");
const migrations = readdirSync(dir)
  .filter((name) => name.endsWith(".sql"))
  .sort();

function migrate(database: DatabaseSync, names: readonly string[]) {
  for (const name of names) {
    for (const statement of readFileSync(join(dir, name), "utf8").split("--> statement-breakpoint")) {
      if (statement.trim() !== "") database.exec(statement);
    }
  }
}

function insert(database: DatabaseSync, userId: string, id: number, parentId: number | null, title: string, done: boolean) {
  database
    .prepare("INSERT INTO todos (id, user_id, parent_id, title, completed_at) VALUES (?, ?, ?, ?, ?)")
    .run(id, userId, parentId, title, done ? "2026-11-01 09:00:00" : null);
}

describe("the migration that reopens completed todos with open todos under them", () => {
  const latest = migrations.indexOf("20261113090000_reopen_done_parents_with_open_todos.sql");

  it("reopens every completed todo above an open one, the state completing a parent used to leave, so nothing open stays hidden under a completed todo", () => {
    const database = new DatabaseSync(":memory:");
    migrate(database, migrations.slice(0, latest));
    insert(database, "a@example.com", 1, null, "trip", true);
    insert(database, "a@example.com", 2, 1, "hotel", true);
    insert(database, "a@example.com", 3, 2, "check in online", false);
    insert(database, "a@example.com", 4, null, "tickets", true);
    insert(database, "a@example.com", 5, 4, "seat", true);
    insert(database, "b@example.com", 6, null, "errands", true);
    insert(database, "b@example.com", 7, 6, "post letter", false);

    migrate(database, migrations.slice(latest));

    const done = database.prepare("SELECT title FROM todos WHERE completed_at IS NOT NULL ORDER BY id").all();
    expect(done.map((row) => row.title)).toEqual(["tickets", "seat"]);
  });

  it("records each reopening as one on the day it runs, so the stats' open count for every earlier day stays right", () => {
    const database = new DatabaseSync(":memory:");
    migrate(database, migrations.slice(0, latest));
    insert(database, "a@example.com", 1, null, "trip", true);
    insert(database, "a@example.com", 2, 1, "hotel", false);
    database.exec("DELETE FROM todo_events");

    migrate(database, migrations.slice(latest));

    expect(database.prepare("SELECT todo_id AS todoId, kind FROM todo_events").all()).toEqual([{ todoId: 1, kind: "reopened" }]);
  });
});
