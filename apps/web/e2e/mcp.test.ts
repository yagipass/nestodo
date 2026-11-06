import { randomUUID } from "node:crypto";
import { Client, StreamableHTTPClientTransport, type VersionNegotiationOptions } from "@modelcontextprotocol/client";
import { afterEach, describe, expect, inject, it } from "vite-plus/test";
import { accessAssertion, json, Session, signedInUser, type Day, type Todo } from "./helpers";

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

describe("MCP endpoint that Claude reaches through its own Access app", () => {
  it.each([
    { protocol: "2025 initialize", versionNegotiation: { mode: "legacy" } },
    { protocol: "2026-07-28", versionNegotiation: { mode: { pin: "2026-07-28" } } },
  ] satisfies { protocol: string; versionNegotiation: VersionNegotiationOptions }[])(
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

  it("lets Claude write, rewrite and remove the note the member sees under a todo's title", async () => {
    const { web, claude } = await member();

    const { todo } = await value<{ todo: Todo }>(claude, "add_todo", {
      title: "book hotel",
      note: "near the station\nhttps://example.com/hotel/4821",
    });
    expect((await web.todos())[0].note).toBe("near the station\nhttps://example.com/hotel/4821");

    await value(claude, "update_todo", { id: todo.id, note: "booked, room 302" });
    expect((await web.todos())[0]).toMatchObject({ title: "book hotel", note: "booked, room 302" });

    await value(claude, "update_todo", { id: todo.id, note: "" });
    expect((await web.todos())[0].note).toBe("");
  });

  it("shows Claude the note the member wrote, so it can use details that are not in the title", async () => {
    const { web, claude } = await member();
    const dentist = await web.createTodo("dentist", { note: "only weekday mornings" });

    const { todos } = await value<{ todos: Todo[] }>(claude, "list_todos");
    const { todo } = await value<{ todo: Todo }>(claude, "get_todo", { id: dentist.id });

    expect([todos[0].note, todo.note]).toEqual(["only weekday mornings", "only weekday mornings"]);
  });

  it("tells Claude what the note is for, so it puts details there instead of stuffing the title", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();
    const noteOf = (name: string) => propertiesOf(tools, name)?.note?.description;

    expect(noteOf("add_todo")).toContain("Note shown under the title");
    expect(noteOf("update_todo")).toContain("an empty string removes it");
    expect(tools.find((tool) => tool.name === "list_todos")?.description).toContain("note is what the member wrote under the title");
  });

  it("tells Claude how long a title and a note may be, so it can shorten them instead of having them refused", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();
    const limitsOf = (name: string) => [propertiesOf(tools, name)?.title?.maxLength, propertiesOf(tools, name)?.note?.maxLength];

    expect(limitsOf("add_todo")).toEqual([2000, 10000]);
    expect(limitsOf("update_todo")).toEqual([2000, 10000]);
  });

  it("refuses a note longer than the web app allows, leaving the outline unchanged", async () => {
    const { web, claude } = await member();
    const { isError } = await call(claude, "add_todo", { title: "book hotel", note: "x".repeat(10001) });
    expect(isError).toBe(true);
    expect(await web.todos()).toEqual([]);
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

  it("offers Claude no way to delete a todo, so it finishes todos by completing them and the outline never loses one", async () => {
    const { claude } = await member();
    const { tools } = await claude.listTools();

    expect(tools.map((tool) => tool.name).sort()).toEqual(["add_todo", "get_stats", "get_todo", "list_todos", "update_todo"]);
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
    expect(hints.add_todo).toMatchObject({ readOnlyHint: false, destructiveHint: false });
    for (const name of ["list_todos", "get_todo", "get_stats"]) expect(hints[name]).toMatchObject({ readOnlyHint: true });
  });

  it("refuses MCP requests that Cloudflare Access did not vouch for", async () => {
    const res = await new Session().request("/mcp", {
      method: "POST",
      body: { jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "list_todos", arguments: {} } },
    });
    expect(res.status).toBe(401);
  });
});
