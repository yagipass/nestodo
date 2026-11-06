import { describe, expect, it } from "vite-plus/test";
import { json, Session, signedInUser, type Todo } from "./helpers";

async function shape(client: Session) {
  return (await client.todos()).map(({ title, parentId, position }) => ({ title, parentId, position }));
}

describe("todo API for the web, CLI and Android apps", () => {
  it("rejects requests without credentials", async () => {
    const res = await new Session().request("/api/v1/todos");
    expect(res.status).toBe(401);
  });

  it("lets a signed-in client create, read, update and delete its todos", async () => {
    const client = await signedInUser();
    const created = await client.createTodo("write report");
    expect(created).toMatchObject({ title: "write report", parentId: null, position: 0, completedAt: null, collapsed: false });

    expect(await client.todos()).toEqual([created]);

    const patched = await client.patch(created.id, { completed: true, title: "report sent", collapsed: true });
    expect(patched.status).toBe(200);
    const { todo } = await json<{ todo: Todo }>(patched);
    expect(todo).toMatchObject({ id: created.id, title: "report sent", collapsed: true });
    expect(todo.completedAt).not.toBeNull();

    expect((await client.request(`/api/v1/todos/${created.id}`, { method: "DELETE" })).status).toBe(204);
    expect((await client.request(`/api/v1/todos/${created.id}`)).status).toBe(404);
  });

  it("keeps an empty todo so an outline row can exist before anything is typed into it", async () => {
    const client = await signedInUser();
    expect((await client.createTodo("")).title).toBe("");
  });

  it("stores a title without trimming it, so a space typed before switching away is still there on return", async () => {
    const client = await signedInUser();
    expect((await client.createTodo("Call ")).title).toBe("Call ");
  });

  it("turns line breaks in a title into spaces, so a title pasted from several lines stays one line that every client edits the same way", async () => {
    const client = await signedInUser();
    const created = await client.createTodo("buy\nmilk\r\nand eggs\r");
    expect(created.title).toBe("buy milk and eggs ");

    const patched = await client.patch(created.id, { title: "call\rthe dentist" });
    expect((await json<{ todo: Todo }>(patched)).todo.title).toBe("call the dentist");
  });

  it("keeps a note under a todo, line breaks and all, so details that do not fit in the title travel with it", async () => {
    const client = await signedInUser();
    const created = await client.createTodo("book hotel", { note: "near the station\nhttps://example.com/hotel/4821" });
    expect(created.note).toBe("near the station\nhttps://example.com/hotel/4821");

    const patched = await client.patch(created.id, { note: "near the station\nbudget 20000 yen" });
    expect((await json<{ todo: Todo }>(patched)).todo.note).toBe("near the station\nbudget 20000 yen");
    expect((await client.todos()).map((todo) => todo.note)).toEqual(["near the station\nbudget 20000 yen"]);
  });

  it("gives a todo added without a note an empty one, so clients can show every note without a null check", async () => {
    const client = await signedInUser();
    expect((await client.createTodo("buy milk")).note).toBe("");
  });

  it("leaves the note alone when only the title changes, so renaming from a client that knows nothing of notes keeps it", async () => {
    const client = await signedInUser();
    const created = await client.createTodo("book hotel", { note: "near the station" });

    await client.patch(created.id, { title: "book the inn", completed: true });

    expect((await client.todos())[0]).toMatchObject({ title: "book the inn", note: "near the station" });
  });

  it("rejects a note over 10000 characters with the validation message", async () => {
    const client = await signedInUser();
    const created = await client.createTodo("book hotel", { note: "near the station" });

    const res = await client.patch(created.id, { note: "x".repeat(10001) });

    expect(res.status).toBe(400);
    expect(JSON.stringify(await res.json())).toContain("Note must be 10000 characters or fewer");
    expect((await client.todos())[0].note).toBe("near the station");
  });

  it("rejects a title over 2000 characters with the validation message", async () => {
    const client = await signedInUser();
    const res = await client.request("/api/v1/todos", { method: "POST", body: { title: "x".repeat(2001) } });
    expect(res.status).toBe(400);
    expect(JSON.stringify(await res.json())).toContain("Title must be 2000 characters or fewer");
    expect(await client.todos()).toEqual([]);
  });

  it("measures a title after its line breaks become spaces, so a pasted title that fits once it is one line is not refused", async () => {
    const client = await signedInUser();
    const created = await client.createTodo(`${"x".repeat(1998)}\r\ny`);
    expect(created.title).toBe(`${"x".repeat(1998)} y`);
  });

  it("counts a title in UTF-16 units like the web and Android editors, so a title of emoji saved from the CLI or Claude is never too long to edit there", async () => {
    const client = await signedInUser();
    const longest = "😀".repeat(1000);
    expect((await client.createTodo(longest)).title).toBe(longest);

    const res = await client.request("/api/v1/todos", { method: "POST", body: { title: "😀".repeat(1001) } });

    expect(res.status).toBe(400);
    expect(JSON.stringify(await res.json())).toContain("Title must be 2000 characters or fewer");
    expect((await client.todos()).map((todo) => todo.title)).toEqual([longest]);
  });

  it("counts a note in UTF-16 units like the web and Android editors, so a note with emoji saved from the CLI or Claude is never too long to edit there", async () => {
    const client = await signedInUser();
    const created = await client.createTodo("book hotel");
    const longest = "😀".repeat(5000);
    expect((await client.patch(created.id, { note: longest })).status).toBe(200);

    const res = await client.patch(created.id, { note: "😀".repeat(5001) });

    expect(res.status).toBe(400);
    expect(JSON.stringify(await res.json())).toContain("Note must be 10000 characters or fewer");
    expect((await client.todos())[0].note).toBe(longest);
  });

  it("inserts a todo at the requested index so a new row can appear in the middle of a list", async () => {
    const client = await signedInUser();
    await client.createTodo("first");
    await client.createTodo("last");
    await client.createTodo("middle", { index: 1 });

    expect(await shape(client)).toEqual([
      { title: "first", parentId: null, position: 0 },
      { title: "middle", parentId: null, position: 1 },
      { title: "last", parentId: null, position: 2 },
    ]);
  });

  it("lists every todo right after its parent so clients can draw the outline without sorting", async () => {
    const client = await signedInUser();
    const work = await client.createTodo("work");
    await client.createTodo("home");
    const release = await client.createTodo("release", { parentId: work.id });
    await client.createTodo("notes", { parentId: release.id });
    await client.createTodo("expenses", { parentId: work.id });

    expect((await client.todos()).map((t) => t.title)).toEqual(["work", "release", "notes", "expenses", "home"]);
  });

  it("moves a todo under another parent, closing the gap it left and opening one where it lands", async () => {
    const client = await signedInUser();
    const a = await client.createTodo("a");
    const b = await client.createTodo("b");
    await client.createTodo("c");
    const b1 = await client.createTodo("b1", { parentId: b.id });

    expect((await client.patch(a.id, { parentId: b.id, index: 0 })).status).toBe(200);

    expect(await shape(client)).toEqual([
      { title: "b", parentId: null, position: 0 },
      { title: "a", parentId: b.id, position: 0 },
      { title: "b1", parentId: b.id, position: 1 },
      { title: "c", parentId: null, position: 1 },
    ]);

    const outdented = await json<{ todo: Todo }>(await client.patch(b1.id, { parentId: null, index: 1 }));
    expect(outdented.todo).toMatchObject({ parentId: null, position: 1 });
    expect((await client.todos()).map((t) => t.title)).toEqual(["b", "a", "b1", "c"]);
  });

  it("puts a todo last under its new parent when the move gives no index, as Claude's update_todo tool tells it", async () => {
    const client = await signedInUser();
    const a = await client.createTodo("a");
    const b = await client.createTodo("b");
    await client.createTodo("b1", { parentId: b.id });
    await client.createTodo("b2", { parentId: b.id });

    expect((await client.patch(a.id, { parentId: b.id })).status).toBe(200);

    expect(await shape(client)).toEqual([
      { title: "b", parentId: null, position: 0 },
      { title: "b1", parentId: b.id, position: 0 },
      { title: "b2", parentId: b.id, position: 1 },
      { title: "a", parentId: b.id, position: 2 },
    ]);
  });

  it("reorders a todo among its siblings when only an index is given", async () => {
    const client = await signedInUser();
    const a = await client.createTodo("a");
    await client.createTodo("b");
    await client.createTodo("c");

    await client.patch(a.id, { index: 2 });
    expect((await client.todos()).map((t) => t.title)).toEqual(["b", "c", "a"]);
  });

  it("leaves a todo in place when a change names its current parent without an index, so a client echoing parentId back does not send it last", async () => {
    const client = await signedInUser();
    const a = await client.createTodo("a");
    const b = await client.createTodo("b");
    await client.createTodo("c");
    const b1 = await client.createTodo("b1", { parentId: b.id });
    await client.createTodo("b2", { parentId: b.id });

    expect((await client.patch(a.id, { parentId: null, completed: true })).status).toBe(200);
    expect((await client.patch(b1.id, { parentId: b.id, title: "b1 renamed" })).status).toBe(200);

    expect((await client.todos()).map((t) => t.title)).toEqual(["a", "b", "b1 renamed", "b2", "c"]);
  });

  it("moves a todo among its siblings when a change names its current parent with a new index, which is what every client sends to move a row up or down", async () => {
    const client = await signedInUser();
    const a = await client.createTodo("a");
    await client.createTodo("b");
    await client.createTodo("c");

    expect((await client.patch(a.id, { parentId: null, index: 2 })).status).toBe(200);

    expect((await client.todos()).map((t) => t.title)).toEqual(["b", "c", "a"]);
  });

  it("refuses to move a todo under itself or its own descendant, which would cut the subtree out of the outline", async () => {
    const client = await signedInUser();
    const parent = await client.createTodo("parent");
    const child = await client.createTodo("child", { parentId: parent.id });

    for (const target of [parent.id, child.id]) {
      const res = await client.patch(parent.id, { parentId: target });
      expect(res.status).toBe(400);
      expect(await json<{ message: string }>(res)).toMatchObject({ message: "A todo cannot be moved under itself" });
    }
    expect(await shape(client)).toEqual([
      { title: "parent", parentId: null, position: 0 },
      { title: "child", parentId: parent.id, position: 0 },
    ]);
  });

  it("keeps every todo within 64 levels, so a member's outline can always be listed and archived", async () => {
    const client = await signedInUser();
    const levels = [await client.createTodo("level 1")];
    while (levels.length < 64) levels.push(await client.createTodo(`level ${levels.length + 1}`, { parentId: levels.at(-1)!.id }));
    const branch = await client.createTodo("branch");
    await client.createTodo("leaf", { parentId: branch.id });

    const tooDeep = [
      await client.request("/api/v1/todos", { method: "POST", body: { title: "level 65", parentId: levels[63].id } }),
      await client.patch(branch.id, { parentId: levels[62].id }),
    ];
    for (const res of tooDeep) {
      expect(res.status).toBe(400);
      expect(await json<{ message: string }>(res)).toMatchObject({ message: "Todos cannot be nested more than 64 levels deep" });
    }
    expect((await client.todos()).find((t) => t.title === "branch")!.parentId).toBeNull();

    expect((await client.patch(branch.id, { parentId: levels[61].id })).status).toBe(200);
    const titles = (await client.todos()).map((t) => t.title);
    expect(titles).toHaveLength(66);
    expect(titles.slice(-5)).toEqual(["level 62", "level 63", "level 64", "branch", "leaf"]);
  });

  it("refuses another user's todo as a parent without revealing it", async () => {
    const owner = await signedInUser();
    const other = await signedInUser();
    const secret = await owner.createTodo("secret");

    const res = await other.request("/api/v1/todos", { method: "POST", body: { title: "sneaky", parentId: secret.id } });
    expect(res.status).toBe(400);
    expect(await json<{ message: string }>(res)).toMatchObject({ message: "Parent todo not found" });
    expect(await owner.todos()).toEqual([secret]);
  });

  it("deletes a todo together with everything under it and closes the gap among its siblings", async () => {
    const client = await signedInUser();
    const doomed = await client.createTodo("doomed");
    await client.createTodo("kept");
    const child = await client.createTodo("child", { parentId: doomed.id });
    const grandchild = await client.createTodo("grandchild", { parentId: child.id });

    expect((await client.request(`/api/v1/todos/${doomed.id}?recursive=true`, { method: "DELETE" })).status).toBe(204);

    expect(await shape(client)).toEqual([{ title: "kept", parentId: null, position: 0 }]);
    for (const gone of [child, grandchild]) {
      expect((await client.request(`/api/v1/todos/${gone.id}`)).status).toBe(404);
    }
  });

  it("refuses to delete a todo with todos under it unless asked to, so a client that never saw them cannot remove them", async () => {
    const client = await signedInUser();
    const parent = await client.createTodo("parent");
    await client.createTodo("added on another device", { parentId: parent.id });

    const res = await client.request(`/api/v1/todos/${parent.id}`, { method: "DELETE" });

    expect(res.status).toBe(409);
    expect(await json<{ error: string }>(res)).toMatchObject({ error: "has_children" });
    expect(await shape(client)).toEqual([
      { title: "parent", parentId: null, position: 0 },
      { title: "added on another device", parentId: parent.id, position: 0 },
    ]);
  });

  it.each([
    { method: "GET", body: undefined },
    { method: "PATCH", body: { title: "hijacked" } },
    { method: "DELETE", body: undefined },
  ])("answers $method on another user's todo with 404 and leaves it untouched", async ({ method, body }) => {
    const owner = await signedInUser();
    const other = await signedInUser();
    const todo = await owner.createTodo("private");

    expect((await other.request(`/api/v1/todos/${todo.id}`, { method, body })).status).toBe(404);
    const ownerView = await owner.request(`/api/v1/todos/${todo.id}`);
    expect((await json<{ todo: Todo }>(ownerView)).todo).toEqual(todo);

    expect((await owner.request(`/api/v1/todos/${todo.id}`, { method, body })).status).not.toBe(404);
  });
});
