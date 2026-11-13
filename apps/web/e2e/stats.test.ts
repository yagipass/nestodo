import { describe, expect, it } from "vite-plus/test";
import { json, Session, signedInUser, type Day } from "./helpers";

async function today(client: Session, timeZone = "UTC") {
  return (await client.stats(timeZone)).days.at(-1);
}

async function statsHtml(client: Session) {
  const res = await client.request("/stats?tz=UTC");
  expect(res.status).toBe(200);
  return res.text();
}

describe("the stats page", () => {
  it("counts the todos added, completed and deleted today across every client", async () => {
    const client = await signedInUser();
    const done = await client.createTodo("done");
    await client.createTodo("still open");
    const dropped = await client.createTodo("dropped");
    await client.patch(done.id, { completed: true });
    await client.request(`/api/v1/todos/${dropped.id}`, { method: "DELETE" });

    expect(await today(client)).toMatchObject({ added: 3, completed: 1, open: 1 });
  });

  it("shades a day on which todos were only added, so days spent planning show up in the calendar", async () => {
    const client = await signedInUser();
    await client.createTodo("plan the week");
    await client.createTodo("plan the month");

    const levels = [...(await statsHtml(client)).matchAll(/class="cell" data-level="(\d)" style=/g)].map((match) => match[1]);
    expect(levels).toHaveLength(182);
    expect(levels.at(-1)).toBe("4");
  });

  it("says how many todos were added and how many were completed under the calendar, not one total", async () => {
    const client = await signedInUser();
    const done = await client.createTodo("done");
    await client.createTodo("still open");
    await client.patch(done.id, { completed: true });

    expect(await statsHtml(client)).toContain("2 added · 1 completed in the past 26 weeks");
  });

  it("does not count a todo as done twice when it is completed again", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("twice");
    await client.patch(todo.id, { completed: true });
    await client.patch(todo.id, { completed: true, title: "twice, renamed" });

    expect(await today(client)).toMatchObject({ completed: 1, open: 0 });
  });

  it("takes back a completion that was undone, and counts the todo as open again", async () => {
    const client = await signedInUser();
    const todo = await client.createTodo("oops");
    await client.patch(todo.id, { completed: true });
    await client.patch(todo.id, { completed: false });

    expect(await today(client)).toMatchObject({ completed: 0, open: 1 });
  });

  it("takes open todos deleted along with their parent off the open count", async () => {
    const client = await signedInUser();
    const parent = await client.createTodo("parent");
    const child = await client.createTodo("child", { parentId: parent.id });
    await client.createTodo("grandchild", { parentId: child.id });
    const done = await client.createTodo("done", { parentId: parent.id });
    await client.patch(done.id, { completed: true });
    await client.request(`/api/v1/todos/${parent.id}?recursive=true`, { method: "DELETE" });

    expect(await today(client)).toMatchObject({ added: 4, completed: 1, open: 0 });
  });

  it("counts every todo completed along with its parent, and takes back a completion the server undid when an open todo went under it", async () => {
    const client = await signedInUser();
    const trip = await client.createTodo("trip");
    await client.createTodo("tickets", { parentId: trip.id });
    await client.createTodo("hotel", { parentId: trip.id });
    await client.patch(trip.id, { completed: true });
    expect(await today(client)).toMatchObject({ completed: 3, open: 0 });

    await client.createTodo("unpack", { parentId: trip.id });

    expect(await today(client)).toMatchObject({ added: 4, completed: 2, open: 2 });
  });

  it("puts today's activity on the viewer's own date, whatever their time zone", async () => {
    const client = await signedInUser();
    await client.createTodo("now");

    for (const timeZone of ["Pacific/Kiritimati", "Etc/GMT+12"]) {
      const localDate = new Intl.DateTimeFormat("en-CA", { timeZone }).format(new Date());
      const { timeZone: shown, days } = await client.stats(timeZone);
      expect(shown).toBe(timeZone);
      expect(days.at(-1)).toMatchObject({ date: localDate, added: 1 });
    }
  });

  it("does not show another user's activity", async () => {
    const owner = await signedInUser();
    const other = await signedInUser();
    await owner.createTodo("owner only");

    expect(await today(other)).toMatchObject({ added: 0, completed: 0, open: 0 });
  });

  it("draws the todos added each day as columns on the same scale as completed ones, so adding faster than completing shows", async () => {
    const client = await signedInUser();
    const done = await client.createTodo("done");
    await client.createTodo("still open");
    await client.patch(done.id, { completed: true });

    const html = await (await client.request("/stats?tz=UTC")).text();
    const heights = (kind: string) =>
      [...html.matchAll(new RegExp(`class="bar ${kind}" style="height:([\\d.]+)%"`, "g"))].map((match) => Number(match[1]));
    expect(heights("added")).toHaveLength(30);
    expect(heights("added").at(-1)).toBe(100);
    expect(heights("completed").at(-1)).toBe(50);
  });

  it("is not shown to signed-out visitors", async () => {
    const res = await new Session().request("/stats", { page: true });
    expect(res.status).toBe(401);
  });
});

describe("the stats API", () => {
  it("gives the Android app the same days as the stats page, dated in the phone's time zone", async () => {
    const client = await signedInUser();
    const done = await client.createTodo("done");
    await client.createTodo("still open");
    await client.patch(done.id, { completed: true });

    const timeZone = "Etc/GMT+12";
    const res = await client.request(`/api/v1/stats?tz=${encodeURIComponent(timeZone)}`);
    expect(res.status).toBe(200);
    const stats = await json<{ timeZone: string; days: Day[] }>(res);

    expect(stats).toEqual(await client.stats(timeZone));
    expect(stats.days.at(-1)).toMatchObject({
      date: new Intl.DateTimeFormat("en-CA", { timeZone }).format(new Date()),
      added: 2,
      completed: 1,
      open: 1,
    });
  });

  it("rejects requests without credentials", async () => {
    const res = await new Session().request("/api/v1/stats");
    expect(res.status).toBe(401);
  });
});
