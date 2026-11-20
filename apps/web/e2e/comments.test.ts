import { randomUUID } from "node:crypto";
import { describe, expect, it } from "vite-plus/test";
import { json, runCron, signedInUser, storedComments, trashedTodoIds, type Comment, type Todo } from "./helpers";

const DAILY = "0 18 * * *";
const DAY = 24 * 60 * 60 * 1000;
const UTC = /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d{3})?Z$/;

function later(ms: number) {
  return new Date(Date.now() + ms);
}

describe("a todo's work log", () => {
  it("lists when the todo was added, completed and reopened among its comments, oldest first, so it reads as a log of the work", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("get kitchen quotes");
    await client.comment(todo.id, "asked two companies");
    await client.patch(todo.id, { completed: true });
    await client.patch(todo.id, { completed: false });
    await client.comment(todo.id, "asking a third one too");

    const items = await client.timeline(todo.id);

    expect(items.map((item) => (item.type === "comment" ? item.body : item.kind))).toEqual([
      "created",
      "asked two companies",
      "completed",
      "reopened",
      "asking a third one too",
    ]);
    expect(items.every((item) => UTC.test(item.at))).toBe(true);
  });

  it("keeps a comment's line breaks and spaces as written, so a pasted log reads the same in every app", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("fix the build");

    const comment = await client.comment(todo.id, "  step 1: clean\n  step 2: rebuild\n");

    expect(comment).toMatchObject({ body: "  step 1: clean\n  step 2: rebuild\n", via: "app", editedAt: null });
    expect(comment.at).toMatch(UTC);
  });

  it("refuses an empty comment and one over 10000 UTF-16 units, the limit the editors count in, leaving the log unchanged", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("book hotel");
    const longest = "😀".repeat(5000);
    expect((await client.comment(todo.id, longest)).body).toBe(longest);

    const blank = await client.request(`/api/v1/todos/${todo.id}/comments`, { method: "POST", body: { body: " \n\t" } });
    const long = await client.request(`/api/v1/todos/${todo.id}/comments`, { method: "POST", body: { body: "😀".repeat(5001) } });

    expect([blank.status, long.status]).toEqual([400, 400]);
    expect(JSON.stringify(await blank.json())).toContain("Comment must not be empty");
    expect(JSON.stringify(await long.json())).toContain("Comment must be 10000 characters or fewer");
    expect((await client.todos())[0].commentCount).toBe(1);
  });

  it("marks a comment as edited only when its text changes, so saving it untouched does not claim an edit", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("call the dentist");
    const comment = await client.comment(todo.id, "no answer");

    const same = await json<{ comment: Comment }>(await client.editComment(comment.id, "no answer"));
    expect(same.comment.editedAt).toBeNull();

    const changed = await json<{ comment: Comment }>(await client.editComment(comment.id, "no answer, try Friday"));
    expect(changed.comment).toMatchObject({ id: comment.id, body: "no answer, try Friday", at: comment.at });
    expect(changed.comment.editedAt).toMatch(UTC);
  });

  it("deletes a comment from the log for good", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("call the dentist");
    const comment = await client.comment(todo.id, "wrong number");

    expect((await client.deleteComment(comment.id)).status).toBe(204);

    expect((await client.timeline(todo.id)).filter((item) => item.type === "comment")).toEqual([]);
    expect((await client.deleteComment(comment.id)).status).toBe(404);
  });

  it("keeps each member's log to themselves, so nobody can read or change another member's comments by guessing ids", async () => {
    const owner = await signedInUser();
    const other = await signedInUser();
    const todo = await owner.createTodo("secret plan");
    const comment = await owner.comment(todo.id, "step one");

    expect((await other.request(`/api/v1/todos/${todo.id}/timeline`)).status).toBe(404);
    expect((await other.request(`/api/v1/todos/${todo.id}/comments`, { method: "POST", body: { body: "hi" } })).status).toBe(404);
    expect((await other.editComment(comment.id, "changed")).status).toBe(404);
    expect((await other.deleteComment(comment.id)).status).toBe(404);
    expect((await owner.timeline(todo.id)).filter((item) => item.type === "comment")).toMatchObject([{ body: "step one" }]);
  });
});

describe("undoing a delete", () => {
  it("brings a deleted todo back with the todos under it, their comments and their ids, so undoing a delete loses nothing", async () => {
    const client = await signedInUser();
    await client.createTodo("before");
    const trip = await client.createTodo("trip", { comment: "late May" });
    const hotel = await client.createTodo("hotel", { parentId: trip.id, comment: "near the station" });
    await client.patch(hotel.id, { completed: true });
    await client.patch(trip.id, { collapsed: true });
    await client.createTodo("after");
    const before = await client.todos();
    const log = await client.timeline(hotel.id);

    await client.request(`/api/v1/todos/${trip.id}?recursive=true`, { method: "DELETE" });
    expect((await client.todos()).map((todo) => todo.title)).toEqual(["before", "after"]);

    const res = await client.restore(trip.id, { parentId: null, index: 1 });

    expect(res.status).toBe(200);
    expect((await json<{ todo: Todo }>(res)).todo).toEqual(before[1]);
    expect(await client.todos()).toEqual(before);
    expect(await client.timeline(hotel.id)).toEqual(log);
  });

  it("leaves the stats as they were before the delete, so an undone delete is not counted as removed and added again", async () => {
    const client = await signedInUser();
    await client.createTodo("keep");
    const doomed = await client.createTodo("doomed");
    const before = await client.stats("UTC");

    await client.request(`/api/v1/todos/${doomed.id}`, { method: "DELETE" });
    await client.restore(doomed.id);

    expect(await client.stats("UTC")).toEqual(before);
  });

  it("puts it back where it was when the client gives no place, so another client can undo without knowing the outline", async () => {
    const client = await signedInUser();
    const first = await client.createTodo("first");
    const middle = await client.createTodo("middle");
    const last = await client.createTodo("last");

    await client.request(`/api/v1/todos/${middle.id}`, { method: "DELETE" });
    await client.restore(middle.id);

    expect((await client.todos()).map((todo) => todo.id)).toEqual([first.id, middle.id, last.id]);
  });

  it("reopens a completed todo it goes back under when it comes back open, since a completed todo never has open todos under it", async () => {
    const client = await signedInUser();
    const errands = await client.createTodo("errands");
    const letter = await client.createTodo("post letter", { parentId: errands.id });
    await client.request(`/api/v1/todos/${letter.id}`, { method: "DELETE" });
    await client.patch(errands.id, { completed: true });

    await client.restore(letter.id, { parentId: errands.id, index: 0 });

    expect((await client.todos()).map(({ title, completedAt }) => [title, completedAt])).toEqual([
      ["errands", null],
      ["post letter", null],
    ]);
  });

  it("refuses to bring back a todo that was never deleted or was already brought back, so a repeated undo cannot duplicate it", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("dentist");

    expect((await client.restore(todo.id)).status).toBe(404);
    await client.request(`/api/v1/todos/${todo.id}`, { method: "DELETE" });
    expect((await client.restore(todo.id)).status).toBe(200);
    expect((await client.restore(todo.id)).status).toBe(404);
    expect((await client.todos()).map((t) => t.id)).toEqual([todo.id]);
  });

  it("refuses to put it under a todo that is gone, keeping it deleted so the client can stop instead of losing track of it", async () => {
    const client = await signedInUser();
    const errands = await client.createTodo("errands");
    const letter = await client.createTodo("post letter", { parentId: errands.id });
    await client.request(`/api/v1/todos/${letter.id}`, { method: "DELETE" });
    await client.request(`/api/v1/todos/${errands.id}`, { method: "DELETE" });

    const res = await client.restore(letter.id, { parentId: errands.id });

    expect(res.status).toBe(400);
    expect(await json<{ error: string }>(res)).toMatchObject({ error: "invalid_placement" });
    expect((await client.restore(errands.id)).status).toBe(200);
    expect((await client.restore(letter.id, { parentId: errands.id })).status).toBe(200);
  });

  it("leaves the comments of a deleted todo as they were, so undoing the delete brings back the same log", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("dentist", { comment: "call on Monday" });
    const [, comment] = await client.timeline(todo.id);
    await client.request(`/api/v1/todos/${todo.id}`, { method: "DELETE" });

    expect((await client.editComment((comment as Comment).id, "changed while deleted")).status).toBe(404);
    expect((await client.deleteComment((comment as Comment).id)).status).toBe(404);

    await client.restore(todo.id);
    expect((await client.timeline(todo.id)).filter((item) => item.type === "comment")).toMatchObject([{ body: "call on Monday" }]);
  });

  it("forgets a deleted todo and its comments 8 days after the delete, once no undo can reach it", async () => {
    const email = `${randomUUID()}@e2e.test`;
    const client = await signedInUser(email);
    const kept = await client.createTodo("kept", { comment: "stays" });
    const doomed = await client.createTodo("doomed", { comment: "goes" });
    await client.request(`/api/v1/todos/${doomed.id}`, { method: "DELETE" });

    await runCron(DAILY, later(7 * DAY));
    expect(trashedTodoIds(email)).toEqual([doomed.id]);

    await runCron(DAILY, later(9 * DAY));
    expect(trashedTodoIds(email)).toEqual([]);
    expect(storedComments(email)).toEqual([{ todoId: kept.id, body: "stays" }]);
    expect((await client.restore(doomed.id)).status).toBe(404);
  });
});

describe("searching comments", () => {
  it("finds todos by what their comments say, with the first comment that matches, so the search covers the work log too", async () => {
    const client = await signedInUser();
    const kitchen = await client.createTodo("kitchen");
    await client.comment(kitchen.id, "asked for a quote");
    await client.comment(kitchen.id, "the Quote from B is lower");
    const bath = await client.createTodo("bath", { comment: "pick a floor" });

    expect(await client.search("quote")).toEqual([{ id: kitchen.id, comment: "asked for a quote" }]);
    expect(await client.search("見積")).toEqual([]);
    await client.comment(bath.id, "見積もりはキッチンと一緒");
    expect(await client.search("見積")).toEqual([{ id: bath.id, comment: "見積もりはキッチンと一緒" }]);
  });

  it("matches % and _ as the characters typed, not as wildcards", async () => {
    const client = await signedInUser();
    const sale = await client.createTodo("sale", { comment: "50% off" });
    await client.createTodo("other", { comment: "500 yen" });
    await client.createTodo("file", { comment: "a_b.txt" });
    await client.createTodo("letters", { comment: "abc" });

    expect((await client.search("0%")).map((todo) => todo.id)).toEqual([sale.id]);
    expect(await client.search("a_c")).toEqual([]);
  });

  it("finds nothing for an empty query and leaves out deleted todos and other members' comments", async () => {
    const client = await signedInUser();
    const other = await signedInUser();
    const gone = await client.createTodo("gone", { comment: "dentist on Friday" });
    await other.createTodo("theirs", { comment: "dentist on Monday" });
    await client.request(`/api/v1/todos/${gone.id}`, { method: "DELETE" });

    expect(await client.search("   ")).toEqual([]);
    expect(await client.search("dentist")).toEqual([]);
    expect(await client.search("d".repeat(10001))).toEqual([]);
  });
});
