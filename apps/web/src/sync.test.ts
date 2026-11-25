import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { ROOT } from "./outline";
import { Sync } from "./sync";

type Call = { method: string; path: string; body?: unknown };

function server(status = (_call: Call) => 200) {
  const calls: Call[] = [];
  let nextId = 100;
  const send = async (method: string, path: string, body?: unknown) => {
    const call = { method, path, body };
    calls.push(call);
    const code = status(call);
    const comment = { id: nextId++, body: (body as { body?: string })?.body, via: "app", at: "2026-11-16T01:00:00.000Z", editedAt: null };
    const payload = path.endsWith("/comments") || path.startsWith("/api/v1/comments/") ? { comment } : method === "POST" ? { todo: { id: comment.id } } : {};
    return new Response(code === 204 ? null : JSON.stringify(payload), { status: code });
  };
  return { calls, send };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("Sync", () => {
  it("sends later edits of a new row to the id the server assigned to it", async () => {
    const { calls, send } = server();
    const sync = new Sync([], send);

    sync.apply([{ type: "create", key: "new1", parent: ROOT, index: 0, title: "" }]);
    sync.apply([{ type: "create", key: "new2", parent: "new1", index: 0, title: "child" }]);
    sync.apply([{ type: "update", key: "new1", fields: { completed: true } }]);
    await vi.runAllTimersAsync();

    expect(calls).toEqual([
      { method: "POST", path: "/api/v1/todos", body: { title: "", parentId: null, index: 0 } },
      { method: "POST", path: "/api/v1/todos", body: { title: "child", parentId: 100, index: 0 } },
      { method: "PATCH", path: "/api/v1/todos/100", body: { completed: true } },
    ]);
    expect(sync.keyOf(101)).toBe("new2");
  });

  it("asks the server to bring a deleted row back under its old id, so undoing a delete keeps the comments written on it", async () => {
    const { calls, send } = server();
    const sync = new Sync([["t1", 1], ["t2", 2]], send);
    const item = { key: "t2", title: "dentist", icon: "🦷", comments: 3, done: false, collapsed: false, children: [] };

    sync.apply([{ type: "delete", key: "t2", removed: ["t2"] }]);
    sync.apply([{ type: "restore", key: "t2", parent: "t1", index: 0, items: [item] }]);
    await vi.runAllTimersAsync();

    expect(calls).toEqual([
      { method: "DELETE", path: "/api/v1/todos/2", body: undefined },
      { method: "POST", path: "/api/v1/todos/2/restore", body: { parentId: 1, index: 0 } },
    ]);
  });

  it("sends an icon change and its removal as the icon alone, so it never overwrites a title another device just changed", async () => {
    const { calls, send } = server();
    const sync = new Sync([["t1", 1]], send);

    sync.apply([{ type: "update", key: "t1", fields: { icon: "🛒" } }]);
    sync.apply([{ type: "update", key: "t1", fields: { icon: null } }]);
    await vi.runAllTimersAsync();

    expect(calls).toEqual([
      { method: "PATCH", path: "/api/v1/todos/1", body: { icon: "🛒" } },
      { method: "PATCH", path: "/api/v1/todos/1", body: { icon: null } },
    ]);
  });

  it("sends a burst of typing as one title update once the user pauses", async () => {
    const { calls, send } = server();
    const sync = new Sync([["t1", 1]], send);

    for (const title of ["B", "Bu", "Buy", "Buy milk"]) {
      sync.title("t1", title);
      await vi.advanceTimersByTimeAsync(200);
    }
    expect(calls).toEqual([]);
    expect(sync.pending).toBe(true);
    await vi.runAllTimersAsync();

    expect(calls).toEqual([{ method: "PATCH", path: "/api/v1/todos/1", body: { title: "Buy milk" } }]);
    expect(sync.pending).toBe(false);
  });

  it("saves typing half a second after the last keystroke, the same pause as the Android app, so other devices see it soon without a request per keystroke", async () => {
    const { calls, send } = server();
    const sync = new Sync([["t1", 1]], send);

    sync.title("t1", "Buy milk");
    await vi.advanceTimersByTimeAsync(499);
    expect(calls).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);

    expect(calls).toEqual([{ method: "PATCH", path: "/api/v1/todos/1", body: { title: "Buy milk" } }]);
  });

  it("sends a comment on a row still being created once the server has given it an id, so a comment written right after Enter is kept", async () => {
    const { calls, send } = server();
    const sync = new Sync([], send);

    sync.apply([{ type: "create", key: "new1", parent: ROOT, index: 0, title: "book hotel" }]);
    const posted = sync.comment("new1", "near the station");
    await vi.runAllTimersAsync();

    expect(calls.map(({ method, path, body }) => [method, path, body])).toEqual([
      ["POST", "/api/v1/todos", { title: "book hotel", parentId: null, index: 0 }],
      ["POST", "/api/v1/todos/100/comments", { body: "near the station" }],
    ]);
    expect(await posted).toMatchObject({ body: "near the station" });
  });

  it("edits and deletes comments by their own ids, in the order they were made", async () => {
    const { calls, send } = server((call) => (call.method === "DELETE" ? 204 : 200));
    const sync = new Sync([["t1", 1]], send);

    const edited = sync.editComment(7, "no answer, try Friday");
    const deleted = sync.deleteComment(8);
    await vi.runAllTimersAsync();

    expect(calls).toEqual([
      { method: "PATCH", path: "/api/v1/comments/7", body: { body: "no answer, try Friday" } },
      { method: "DELETE", path: "/api/v1/comments/8", body: undefined },
    ]);
    expect(await edited).toMatchObject({ body: "no answer, try Friday" });
    await expect(deleted).resolves.toBeUndefined();
  });

  it("rejects a comment the server refused and every comment after it, so the pane can stop showing them as sending", async () => {
    const { send } = server((call) => (call.path === "/api/v1/todos/1/comments" ? 500 : 200));
    let notified = 0;
    const sync = new Sync([["t1", 1], ["t2", 2]], send, () => notified++);

    const refused = expect(sync.comment("t1", "first")).rejects.toThrow();
    const waiting = expect(sync.comment("t2", "second")).rejects.toThrow();
    await vi.runAllTimersAsync();

    await refused;
    await waiting;
    await expect(sync.comment("t2", "third")).rejects.toThrow();
    expect([sync.failed, notified]).toEqual([true, 1]);
  });

  it("sends a pending title before a structural change to the same row, keeping the user's order", async () => {
    const { calls, send } = server();
    const sync = new Sync([["t1", 1]], send);

    sync.title("t1", "renamed");
    sync.apply([{ type: "move", key: "t1", parent: ROOT, index: 3 }]);
    await vi.runAllTimersAsync();

    expect(calls.map((call) => call.body)).toEqual([{ title: "renamed" }, { parentId: null, index: 3 }]);
  });

  it("sends the merged title before deleting the merged row, so a failure in between never loses the text", async () => {
    const { calls, send } = server((call) => (call.method === "PATCH" ? 503 : 204));
    const sync = new Sync([["t1", 1], ["t2", 2]], send);

    sync.title("t1", "buy ");
    sync.apply([
      { type: "title", key: "t1", title: "buy milk" },
      { type: "delete", key: "t2", removed: ["t2"] },
    ]);
    await vi.runAllTimersAsync();

    expect(calls).toEqual([{ method: "PATCH", path: "/api/v1/todos/1", body: { title: "buy milk" } }]);
    expect(sync.failed).toBe(true);
  });

  it("drops pending titles of deleted rows instead of sending them to todos that no longer exist", async () => {
    const { calls, send } = server((call) => (call.method === "DELETE" ? 204 : 200));
    const sync = new Sync([["t1", 1], ["t2", 2]], send);

    sync.title("t1", "typed into the row");
    sync.title("t2", "typed under the parent");
    sync.apply([{ type: "delete", key: "t1", removed: ["t1", "t2"] }]);
    await vi.runAllTimersAsync();

    expect(calls).toEqual([{ method: "DELETE", path: "/api/v1/todos/1?recursive=true", body: undefined }]);
  });

  it("asks for the subtree only when the row had children here, so a row that looked empty never takes unseen todos with it", async () => {
    const { calls, send } = server(() => 204);
    const sync = new Sync([["t1", 1], ["t2", 2], ["t3", 3]], send);

    sync.apply([{ type: "delete", key: "t1", removed: ["t1", "t2"] }]);
    sync.apply([{ type: "delete", key: "t3", removed: ["t3"] }]);
    await vi.runAllTimersAsync();

    expect(calls.map((call) => call.path)).toEqual(["/api/v1/todos/1?recursive=true", "/api/v1/todos/3"]);
  });

  it("stops after a failure so later edits are not applied on top of a state the client no longer matches", async () => {
    const { calls, send } = server((call) => (call.path.endsWith("/1") ? 500 : 200));
    let notified = 0;
    const sync = new Sync([["t1", 1], ["t2", 2]], send, () => notified++);

    sync.apply([{ type: "update", key: "t1", fields: { completed: true } }]);
    await vi.runAllTimersAsync();
    sync.apply([{ type: "update", key: "t2", fields: { completed: true } }]);
    await vi.runAllTimersAsync();

    expect(calls.map((call) => call.path)).toEqual(["/api/v1/todos/1"]);
    expect(sync.failed).toBe(true);
    expect(notified).toBe(1);
  });

  it("keeps counting a change that failed to save as unsaved, so closing the tab still warns before it is lost even when the failure happened out of sight", async () => {
    const { send } = server(() => 500);
    const sync = new Sync([["t1", 1]], send);

    sync.apply([{ type: "update", key: "t1", fields: { completed: true } }]);
    await vi.runAllTimersAsync();

    expect(sync.failed).toBe(true);
    expect(sync.pending).toBe(true);
  });

  it("treats deleting a todo that is already gone as done", async () => {
    const { send } = server(() => 404);
    const sync = new Sync([["t1", 1]], send);

    sync.apply([{ type: "delete", key: "t1", removed: ["t1"] }]);
    await vi.runAllTimersAsync();

    expect(sync.failed).toBe(false);
  });

  it("treats a delete the server refuses as a failure, so editing pauses and the member learns the delete did not happen instead of a row that gained todos on another device vanishing unnoticed", async () => {
    const { send } = server((call) => (call.method === "DELETE" ? 409 : 200));
    let notified = 0;
    const sync = new Sync([["t1", 1]], send, () => notified++);

    sync.apply([{ type: "delete", key: "t1", removed: ["t1"] }]);
    await vi.runAllTimersAsync();

    expect(sync.failed).toBe(true);
    expect(notified).toBe(1);
  });

  it("treats a change to a todo deleted on another device as a failure, so editing pauses until a reload shows it gone instead of the change vanishing unnoticed", async () => {
    const { send } = server(() => 404);
    let notified = 0;
    const sync = new Sync([["t1", 1]], send, () => notified++);

    sync.apply([{ type: "update", key: "t1", fields: { completed: true } }]);
    await vi.runAllTimersAsync();

    expect(sync.failed).toBe(true);
    expect(notified).toBe(1);
  });
});
