import { randomUUID } from "node:crypto";
import { Client, StreamableHTTPClientTransport, type VersionNegotiationOptions } from "@modelcontextprotocol/client";
import { afterEach, describe, expect, inject, it } from "vite-plus/test";
import { accessAssertion, json, Session, signedInUser, type Comment, type Day, type TimelineItem, type Todo } from "./helpers";

const baseUrl = inject("baseUrl");
const access = inject("access");

const clients: Client[] = [];

afterEach(async () => {
  await Promise.all(clients.splice(0).map((client) => client.close()));
});

async function connect(email: string, versionNegotiation: VersionNegotiationOptions = { mode: "legacy" }) {
  const assertion = await accessAssertion({ email, audience: access.mcpAudience });
  const client = new Client({ name: "e2e", version: "1.0.0" }, { versionNegotiation });
  clients.push(client);
  await client.connect(
    new StreamableHTTPClientTransport(new URL("/mcp", baseUrl), {
      requestInit: { headers: { "Cf-Access-Jwt-Assertion": assertion } },
    }),
  );
  return client;
}

async function member() {
  const email = `${randomUUID()}@e2e.test`;
  return { web: await signedInUser(email), claude: await connect(email) };
}

async function call(client: Client, name: string, args: Record<string, unknown> = {}) {
  const result = await client.callTool({ name, arguments: args });
  const [block] = result.content;
  if (block?.type !== "text") throw new Error(`${name} returned ${JSON.stringify(result)}`);
  return { isError: result.isError === true, text: block.text };
}

async function value<T>(client: Client, name: string, args: Record<string, unknown> = {}) {
  const { isError, text } = await call(client, name, args);
  if (isError) throw new Error(`${name} failed: ${text}`);
  return JSON.parse(text) as T;
}

async function refusal(client: Client, name: string, args: Record<string, unknown>) {
  const { isError, text } = await call(client, name, args);
  expect(isError, `${name} should have been refused, but returned ${text}`).toBe(true);
  return JSON.parse(text) as { error: string; message: string };
}

function propertiesOf(tools: Awaited<ReturnType<Client["listTools"]>>["tools"], name: string) {
  return tools.find((tool) => tool.name === name)?.inputSchema.properties as Record<string, { description?: string; maxLength?: number }> | undefined;
}

const protocols = [
  { protocol: "2025 initialize", versionNegotiation: { mode: "legacy" } },
  { protocol: "2026-07-28", versionNegotiation: { mode: { pin: "2026-07-28" } } },
] satisfies { protocol: string; versionNegotiation: VersionNegotiationOptions }[];

describe("MCP endpoint that Claude reaches through its own Access app", () => {
  it.each(protocols)(
    "shows Claude the same todos the family member sees on the web, over the $protocol protocol",
    async ({ versionNegotiation }) => {
      const email = `${randomUUID()}@e2e.test`;
      await (await signedInUser(email)).createTodo("buy milk");
      await (await signedInUser()).createTodo("someone else's todo");

      const claude = await connect(email, versionNegotiation);
      const { todos } = await value<{ todos: Todo[] }>(claude, "list_todos");
      expect(todos.map((todo) => todo.title)).toEqual(["buy milk"]);
    },
  );

  it("puts the todos Claude adds, nests, renames and completes into the member's web outline", async () => {
    const { web, claude } = await member();

    const { todo: trip } = await value<{ todo: Todo }>(claude, "add_todo", { title: "plan trip" });
    const { todo: hotel } = await value<{ todo: Todo }>(claude, "add_todo", { title: "book hotel", parentId: trip.id });
    await value(claude, "add_todo", { title: "pack", parentId: trip.id, index: 0 });
    await value(claude, "update_todo", { id: hotel.id, title: "book the inn", completed: true });

    expect((await web.todos()).map(({ title, parentId, completedAt }) => ({ title, parentId, done: completedAt !== null }))).toEqual([
      { title: "plan trip", parentId: null, done: false },
      { title: "pack", parentId: trip.id, done: false },
      { title: "book the inn", parentId: trip.id, done: true },
    ]);
    expect(await value(claude, "get_todo", { id: hotel.id })).toEqual({
      todo: { ...(await web.todos())[2], completedAt: expect.stringMatching(/Z$/), createdAt: expect.stringMatching(/Z$/) },
    });
  });

  it("marks the completion times Claude reads as UTC, since a bare time reads as the member's own and puts early-morning work on the day before", async () => {
    const { web, claude } = await member();
    const taxes = await web.createTodo("file taxes");
    await web.patch(taxes.id, { completed: true });
    const [{ completedAt: stored }] = await web.todos();
    const utc = `${stored?.replace(" ", "T")}Z`;

    const { todos } = await value<{ todos: Todo[] }>(claude, "list_todos");
    const { todo: shown } = await value<{ todo: Todo }>(claude, "get_todo", { id: taxes.id });
    const { todo: renamed } = await value<{ todo: Todo }>(claude, "update_todo", { id: taxes.id, title: "file tax return" });

    expect(shown.completedAt).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
    expect([todos[0].completedAt, shown.completedAt, renamed.completedAt]).toEqual([utc, utc, utc]);
  });

  it("tells Claude when each todo was added, in UTC, so a daily report can list what the member added that day", async () => {
    const { web, claude } = await member();
    const milk = await web.createTodo("buy milk");
    const utc = `${milk.createdAt.replace(" ", "T")}Z`;

    const { todos } = await value<{ todos: Todo[] }>(claude, "list_todos");
    const { todo: shown } = await value<{ todo: Todo }>(claude, "get_todo", { id: milk.id });
    const { todo: completed } = await value<{ todo: Todo }>(claude, "update_todo", { id: milk.id, completed: true });
    const { todo: added } = await value<{ todo: Todo }>(claude, "add_todo", { title: "buy eggs" });

    expect(shown.createdAt).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
    expect([todos[0].createdAt, shown.createdAt, completed.createdAt]).toEqual([utc, utc, utc]);
    expect(added.createdAt).toMatch(/Z$/);
  });

  it("lets Claude start a todo's work log and add to it, marked as from MCP so the member can tell its comments from their own", async () => {
    const { web, claude } = await member();

    const { todo } = await value<{ todo: Todo }>(claude, "add_todo", {
      title: "book hotel",
      comment: "near the station\nhttps://example.com/hotel/4821",
    });
    const { comment } = await value<{ comment: Comment }>(claude, "add_comment", { id: todo.id, body: "two rooms left for May 3" });

    expect(todo.commentCount).toBe(1);
    expect(comment).toMatchObject({ body: "two rooms left for May 3", via: "mcp" });
    expect((await web.timeline(todo.id)).filter((item) => item.type === "comment")).toMatchObject([
      { body: "near the station\nhttps://example.com/hotel/4821", via: "mcp" },
      { body: "two rooms left for May 3", via: "mcp" },
    ]);
    expect((await web.todos())[0].commentCount).toBe(2);
  });

  it("shows Claude the work log the member kept, so it can use details that are not in the title", async () => {
    const { web, claude } = await member();
    const dentist = await web.createTodo("dentist", { comment: "only weekday mornings" });
    await web.patch(dentist.id, { completed: true });

    const { items } = await value<{ items: TimelineItem[] }>(claude, "get_timeline", { id: dentist.id });

    expect(items.map((item) => (item.type === "comment" ? `${item.via}: ${item.body}` : item.kind))).toEqual([
      "created",
      "app: only weekday mornings",
      "completed",
    ]);
    expect((await value<{ todos: Todo[] }>(claude, "list_todos")).todos[0].commentCount).toBe(1);
  });

  it("tells Claude that comments are a work log and that update_todo no longer edits text under the title", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();

    expect(propertiesOf(tools, "add_todo")?.comment?.description).toContain("work log");
    expect(propertiesOf(tools, "update_todo")?.note).toBeUndefined();
    expect(tools.find((tool) => tool.name === "update_todo")?.description).toContain("use add_comment");
    expect(tools.find((tool) => tool.name === "list_todos")?.description).toContain("commentCount");
  });

  it("tells Claude how long a title and a comment may be, so it can shorten them instead of having them refused", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();

    expect([propertiesOf(tools, "add_todo")?.title?.maxLength, propertiesOf(tools, "add_todo")?.comment?.maxLength]).toEqual([2000, 10000]);
    expect(propertiesOf(tools, "update_todo")?.title?.maxLength).toBe(2000);
    expect(propertiesOf(tools, "add_comment")?.body?.maxLength).toBe(10000);
  });

  it("refuses a comment longer than the web app allows or one on another member's todo, leaving everything unchanged", async () => {
    const victim = await signedInUser();
    const secret = await victim.createTodo("victim's secret");
    const { web, claude } = await member();
    const own = await web.createTodo("book hotel");

    expect((await call(claude, "add_todo", { title: "book hotel", comment: "x".repeat(10001) })).isError).toBe(true);
    expect((await call(claude, "add_comment", { id: own.id, body: "x".repeat(10001) })).isError).toBe(true);
    expect(await refusal(claude, "add_comment", { id: secret.id, body: "planted" })).toMatchObject({ error: "not_found" });
    expect(await refusal(claude, "get_timeline", { id: secret.id })).toMatchObject({ error: "not_found" });

    expect(await web.todos()).toEqual([own]);
    expect(await victim.todos()).toEqual([secret]);
  });

  it("tells Claude which open todos were completed along with the one it completed, so it can report them to the member", async () => {
    const { web, claude } = await member();
    const parent = await web.createTodo("errands");
    const letter = await web.createTodo("post letter", { parentId: parent.id });
    const stamps = await web.createTodo("buy stamps", { parentId: letter.id });

    const { completedUnder } = await value<{ completedUnder: number[] }>(claude, "update_todo", { id: parent.id, completed: true });

    expect(new Set(completedUnder)).toEqual(new Set([letter.id, stamps.id]));
  });

  it("lets Claude reopen a todo and move it back to the top level", async () => {
    const { web, claude } = await member();
    const parent = await web.createTodo("errands");
    const child = await web.createTodo("post letter", { parentId: parent.id });
    await web.patch(child.id, { completed: true });

    await value(claude, "update_todo", { id: child.id, completed: false, parentId: null, index: 0 });

    expect((await web.todos()).map(({ title, parentId, completedAt }) => ({ title, parentId, completedAt }))).toEqual([
      { title: "post letter", parentId: null, completedAt: null },
      { title: "errands", parentId: null, completedAt: null },
    ]);
  });

  it("never lets Claude read, change or nest under another member's todo, even by its id", async () => {
    const victim = await signedInUser();
    const secret = await victim.createTodo("victim's secret");
    const { web, claude } = await member();
    const own = await web.createTodo("mine");

    expect(await refusal(claude, "get_todo", { id: secret.id })).toMatchObject({ error: "not_found" });
    expect(await refusal(claude, "update_todo", { id: secret.id, title: "changed by someone else" })).toMatchObject({ error: "not_found" });
    expect(await refusal(claude, "add_todo", { title: "planted", parentId: secret.id })).toMatchObject({ error: "invalid_placement" });
    expect(await refusal(claude, "update_todo", { id: own.id, parentId: secret.id })).toMatchObject({ error: "invalid_placement" });

    expect(await victim.todos()).toEqual([secret]);
    expect(await web.todos()).toEqual([own]);
  });

  it("turns line breaks in the titles Claude writes into spaces, so the member's outline shows and edits them as one line", async () => {
    const { web, claude } = await member();
    const { todo } = await value<{ todo: Todo }>(claude, "add_todo", { title: "buy\nmilk" });
    expect(todo.title).toBe("buy milk");

    await value(claude, "update_todo", { id: todo.id, title: "buy\r\noat milk" });

    expect((await web.todos()).map((t) => t.title)).toEqual(["buy oat milk"]);
  });

  it("refuses a title longer than the web app allows, leaving the outline unchanged", async () => {
    const { web, claude } = await member();
    const { isError } = await call(claude, "add_todo", { title: "x".repeat(2001) });
    expect(isError).toBe(true);
    expect(await web.todos()).toEqual([]);
  });

  it("gives Claude the stats dated in the time zone it is told, since its requests do not come from where the member is", async () => {
    const { web, claude } = await member();
    await web.createTodo("still open");
    const timeZone = new Date().getUTCHours() < 12 ? "Etc/GMT+12" : "Etc/GMT-14";

    const res = await web.request(`/api/v1/stats?tz=${encodeURIComponent(timeZone)}`);
    const fromWeb = await json<{ timeZone: string; days: Day[] }>(res);

    expect(await value(claude, "get_stats", { timeZone })).toEqual(fromWeb);
    expect(fromWeb.days.at(-1)).toMatchObject({ date: new Intl.DateTimeFormat("en-CA", { timeZone }).format(new Date()), added: 1 });
    expect((await call(claude, "get_stats", { timeZone: "Mars/Olympus_Mons" })).isError).toBe(true);
  });

  it.each(protocols)(
    "hands Claude a view that draws the stats as the stats page does, whole in one document since the conversation showing it loads nothing from Nestodo, over the $protocol protocol",
    async ({ versionNegotiation }) => {
      const claude = await connect(`${randomUUID()}@e2e.test`, versionNegotiation);
      const { tools } = await claude.listTools();
      const { resourceUri } = tools.find((tool) => tool.name === "get_stats")?._meta?.ui as { resourceUri: string };

      const { contents } = await claude.readResource({ uri: resourceUri });
      expect(contents).toHaveLength(1);
      const [view] = contents;
      if (!("text" in view)) throw new Error(`${resourceUri} is not text`);
      expect(view.mimeType).toBe("text/html;profile=mcp-app");
      expect(view.text).toContain("Past 26 weeks");
      expect(view.text.slice(0, view.text.indexOf("<script"))).not.toMatch(/src=|href=|url\(|@import/);
    },
  );

  it("offers Claude no way to delete a todo, so it finishes todos by completing them and the outline never loses one", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();

    expect(tools.map((tool) => tool.name).sort()).toEqual(["add_comment", "add_todo", "get_stats", "get_timeline", "get_todo", "list_todos", "update_todo"]);
  });

  it("tells Claude that completed todos leave the list after 8 days, so it does not take the list for everything the member finished", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();

    expect(tools.find((tool) => tool.name === "list_todos")?.description).toContain(
      "Completed todos are removed from this list 8 days after they were completed",
    );
  });

  it("marks editing destructive and reading read-only, since an edit can overwrite a title while reading changes nothing", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();
    const hints = Object.fromEntries(tools.map((tool) => [tool.name, tool.annotations]));

    expect(hints.update_todo).toMatchObject({ readOnlyHint: false, destructiveHint: true });
    for (const name of ["add_todo", "add_comment"]) expect(hints[name]).toMatchObject({ readOnlyHint: false, destructiveHint: false });
    for (const name of ["list_todos", "get_todo", "get_timeline", "get_stats"]) expect(hints[name]).toMatchObject({ readOnlyHint: true });
  });

  it("refuses MCP requests that Cloudflare Access did not vouch for", async () => {
    const res = await new Session().request("/mcp", {
      method: "POST",
      body: { jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "list_todos", arguments: {} } },
    });
    expect(res.status).toBe(401);
  });
});
