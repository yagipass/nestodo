import { randomUUID } from "node:crypto";
import { describe, expect, it } from "vite-plus/test";
import { archiveDirectly, archivedTodos, reopenDirectly, runCron, Session, setParentDirectly, signedInUser, triggerCron } from "./helpers";

const DAILY = "0 18 * * *";
const HOUR = 3_600_000;
const DAY = 24 * HOUR;

function later(ms: number) {
  return new Date(Date.now() + ms);
}

async function shape(client: Session) {
  return (await client.todos()).map(({ title, parentId, position }) => ({ title, parentId, position }));
}

async function titles(client: Session) {
  return (await client.todos()).map((todo) => todo.title);
}

describe("the daily cleanup of completed todos", () => {
  it("deletes a todo once 8 days have passed since it was completed, so finished work stops piling up", async () => {
    const client = await signedInUser();
    const done = await client.createTodo("done");
    await client.createTodo("open");
    await client.patch(done.id, { completed: true });

    await runCron(DAILY, later(8 * DAY + HOUR));

    expect(await titles(client)).toEqual(["open"]);
  });

  it("keeps a todo completed less than 8 days ago, so it can still be looked back on or reopened", async () => {
    const client = await signedInUser();
    const done = await client.createTodo("done");
    await client.patch(done.id, { completed: true });

    await runCron(DAILY, later(8 * DAY - HOUR));

    expect(await titles(client)).toEqual(["done"]);
  });

  it("never deletes an open todo, however long it has been open", async () => {
    const client = await signedInUser();
    await client.createTodo("someday");

    await runCron(DAILY, later(365 * DAY));

    expect(await titles(client)).toEqual(["someday"]);
  });

  it("keeps a completed todo while a todo under it is still open, so the open one never vanishes with it even in data that breaks the rule that completing a todo completes everything under it", async () => {
    const client = await signedInUser();
    const move = await client.createTodo("move");
    const pack = await client.createTodo("pack boxes", { parentId: move.id });
    await client.patch(move.id, { completed: true });
    reopenDirectly(pack.id);

    await runCron(DAILY, later(30 * DAY));

    expect(await titles(client)).toEqual(["move", "pack boxes"]);
  });

  it("deletes a completed todo with the completed todos under it, and a completed todo under an open one", async () => {
    const client = await signedInUser();
    const trip = await client.createTodo("trip");
    const tickets = await client.createTodo("tickets", { parentId: trip.id });
    const work = await client.createTodo("work");
    const report = await client.createTodo("report", { parentId: work.id });
    for (const todo of [trip, tickets, report]) await client.patch(todo.id, { completed: true });

    await runCron(DAILY, later(30 * DAY));

    expect(await titles(client)).toEqual(["work"]);
  });

  it("closes the gaps the deleted todos leave, so a todo added at an index lands there", async () => {
    const client = await signedInUser();
    const a = await client.createTodo("a");
    const a1 = await client.createTodo("a1", { parentId: a.id });
    await client.createTodo("a2", { parentId: a.id });
    const b = await client.createTodo("b");
    const c = await client.createTodo("c");
    await client.createTodo("d");
    for (const todo of [a1, b, c]) await client.patch(todo.id, { completed: true });

    await runCron(DAILY, later(30 * DAY));
    await client.createTodo("e", { index: 1 });

    expect(await shape(client)).toEqual([
      { title: "a", parentId: null, position: 0 },
      { title: "a2", parentId: a.id, position: 0 },
      { title: "e", parentId: null, position: 1 },
      { title: "d", parentId: null, position: 2 },
    ]);
  });

  it("leaves the stats as they were, so the cleanup does not count as todos deleted before they were done", async () => {
    const client = await signedInUser();
    const done = await client.createTodo("done");
    await client.createTodo("open");
    await client.patch(done.id, { completed: true });
    const before = await client.stats("UTC");

    await runCron(DAILY, later(30 * DAY));

    expect(await titles(client)).toEqual(["open"]);
    expect(await client.stats("UTC")).toEqual(before);
  });

  it("keeps what a todo said and when it was added and completed after it leaves the list, so finished work can be looked back on", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const client = await signedInUser(email);
    const done = await client.createTodo("renew passport", { note: "bring two photos" });
    await client.patch(done.id, { completed: true });
    const [completed] = await client.todos();

    await runCron(DAILY, later(8 * DAY + HOUR));

    expect(await client.todos()).toEqual([]);
    expect(archivedTodos(email)).toEqual([
      { path: [], title: "renew passport", note: "bring two photos", createdAt: completed.createdAt, completedAt: completed.completedAt },
    ]);
  });

  it("keeps the titles a todo was under when it left the list, so it stays in context after they are renamed or deleted", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const client = await signedInUser(email);
    const home = await client.createTodo('home, "sweet" home');
    const move = await client.createTodo("move", { parentId: home.id });
    const pack = await client.createTodo("pack boxes", { parentId: move.id });
    await client.patch(pack.id, { completed: true });

    await runCron(DAILY, later(30 * DAY));
    await client.patch(move.id, { title: "move out" });
    await client.request(`/api/v1/todos/${home.id}?recursive=true`, { method: "DELETE" });

    expect(archivedTodos(email).map(({ path, title }) => ({ path, title }))).toEqual([
      { path: ['home, "sweet" home', "move"], title: "pack boxes" },
    ]);
  });

  it("keeps only the todos the cleanup takes off the list, not open ones or completed ones deleted by hand", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const client = await signedInUser(email);
    await client.createTodo("someday");
    const done = await client.createTodo("done");
    const typo = await client.createTodo("typo");
    for (const todo of [done, typo]) await client.patch(todo.id, { completed: true });
    await client.request(`/api/v1/todos/${typo.id}`, { method: "DELETE" });

    await runCron(DAILY, later(30 * DAY));

    expect(archivedTodos(email).map((todo) => todo.title)).toEqual(["done"]);
  });

  it("keeps cleaning up when two moves that raced left todos under each other, so one loop does not stop finished todos from leaving anyone's list", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const looped = await signedInUser(email);
    const x = await looped.createTodo("x");
    const y = await looped.createTodo("y", { parentId: x.id });
    const z = await looped.createTodo("z", { parentId: x.id });
    await looped.patch(z.id, { completed: true });
    setParentDirectly(x.id, y.id);
    const other = await signedInUser();
    const done = await other.createTodo("done");
    await other.patch(done.id, { completed: true });

    await runCron(DAILY, later(30 * DAY));

    expect(await titles(other)).toEqual([]);
    expect(archivedTodos(email).map(({ path, title }) => ({ path, title }))).toEqual([{ path: ["y", "x"], title: "z" }]);
  });

  it("keeps cleaning up everyone else's list when one member's cleanup fails, and still reports that failure", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const stuck = await signedInUser(email);
    const blocked = await stuck.createTodo("blocked");
    await stuck.patch(blocked.id, { completed: true });
    archiveDirectly(blocked.id, email);
    try {
      const other = await signedInUser();
      const done = await other.createTodo("done");
      await other.patch(done.id, { completed: true });

      expect((await triggerCron(DAILY, later(30 * DAY))).ok).toBe(false);

      expect(await titles(other)).toEqual([]);
      expect(await titles(stuck)).toEqual(["blocked"]);
    } finally {
      await stuck.request(`/api/v1/todos/${blocked.id}`, { method: "DELETE" });
    }
  });
});
