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

describe("the migration that moves notes into comments", () => {
  const latest = migrations.indexOf("20261116021242_move_notes_into_comments.sql");

  function noted(database: DatabaseSync, userId: string, id: number, note: string, createdAt: string) {
    database.prepare("INSERT INTO todos (id, user_id, title, note, created_at) VALUES (?, ?, ?, ?, ?)").run(id, userId, `todo ${id}`, note, createdAt);
  }

  it("turns each written note into the first comment of its todo, dated when the todo was added, so nothing written is lost when notes go away", () => {
    const database = new DatabaseSync(":memory:");
    migrate(database, migrations.slice(0, latest));
    noted(database, "a@example.com", 1, "near the station\nhttps://example.com/hotel", "2026-11-01 09:00:00");
    noted(database, "b@example.com", 2, "  bring the receipt  ", "2026-11-02 10:00:00");
    database
      .prepare("INSERT INTO archived_todos (id, user_id, path, title, note, created_at, completed_at) VALUES (?, ?, '[]', ?, ?, ?, ?)")
      .run(3, "a@example.com", "renew passport", "photo booth on 2F", "2026-10-01 08:00:00", "2026-10-03 08:00:00");

    migrate(database, migrations.slice(latest));

    expect(database.prepare("SELECT user_id AS userId, todo_id AS todoId, body, via, created_at AS createdAt FROM todo_comments ORDER BY todo_id").all()).toEqual([
      { userId: "a@example.com", todoId: 1, body: "near the station\nhttps://example.com/hotel", via: "app", createdAt: "2026-11-01 09:00:00" },
      { userId: "b@example.com", todoId: 2, body: "  bring the receipt  ", via: "app", createdAt: "2026-11-02 10:00:00" },
      { userId: "a@example.com", todoId: 3, body: "photo booth on 2F", via: "app", createdAt: "2026-10-01 08:00:00" },
    ]);
  });

  it("adds no comment for an empty or blank note, so a todo nobody wrote about does not show an empty comment", () => {
    const database = new DatabaseSync(":memory:");
    migrate(database, migrations.slice(0, latest));
    noted(database, "a@example.com", 1, "", "2026-11-01 09:00:00");
    noted(database, "a@example.com", 2, " \n\t\r ", "2026-11-01 09:00:00");
    noted(database, "a@example.com", 3, "\u3000\u00a0", "2026-11-01 09:00:00");

    migrate(database, migrations.slice(latest));

    expect(database.prepare("SELECT count(*) AS count FROM todo_comments").get()).toEqual({ count: 0 });
  });

  it("records completing and reopening to the millisecond, so a todo's log can place them against comments written in the same second", () => {
    const database = new DatabaseSync(":memory:");
    migrate(database, migrations);
    insert(database, "a@example.com", 1, null, "call the dentist", false);
    database.prepare("UPDATE todos SET completed_at = datetime('now') WHERE id = 1").run();
    database.prepare("UPDATE todos SET completed_at = NULL WHERE id = 1").run();

    const times = database.prepare("SELECT at FROM todo_events WHERE kind IN ('completed', 'reopened') ORDER BY id").all() as { at: string }[];

    expect(times.map(({ at }) => at)).toEqual([expect.stringMatching(/\.\d{3}$/), expect.stringMatching(/\.\d{3}$/)]);
  });

  it("does not count a todo brought back from the trash as added again, so undoing a delete leaves the stats as they were", () => {
    const database = new DatabaseSync(":memory:");
    migrate(database, migrations);
    insert(database, "a@example.com", 1, null, "call the dentist", false);
    database
      .prepare(
        "INSERT INTO trashed_todos (id, user_id, batch, parent_id, position, title, collapsed, completed_at, created_at, updated_at) VALUES (2, 'a@example.com', 2, NULL, 0, 'renew passport', 0, NULL, '2026-11-01 09:00:00', '2026-11-01 09:00:00')",
      )
      .run();
    insert(database, "a@example.com", 2, null, "renew passport", false);

    expect(database.prepare("SELECT todo_id AS todoId FROM todo_events WHERE kind = 'created'").all()).toEqual([{ todoId: 1 }]);
  });
});
