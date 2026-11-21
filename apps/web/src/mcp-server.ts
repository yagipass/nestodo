import { registerAppResource, registerAppTool, RESOURCE_MIME_TYPE } from "@modelcontextprotocol/ext-apps/server";
import { McpServer, type CallToolResult } from "@modelcontextprotocol/server";
import { HTTPException } from "hono/http-exception";
import statsApp from "virtual:stats-app";
import { z } from "zod";
import { isTimeZone } from "./stats";
import { activityOf } from "./stats-service";
import { inUtc } from "./time";
import { commentBody, createTodoBody, patchTodoBody } from "./todo-schemas";
import { addComment, createTodo, getTodo, listTodos, timelineOf, updateTodo } from "./todo-service";
import { KEEP_COMPLETED_DAYS, MAX_COMMENT_LENGTH, MAX_DEPTH, MAX_TITLE_LENGTH } from "./todos";

const notFound = { error: "not_found", message: "Todo not found" };

const STATS_APP_URI = "ui://nestodo/stats.html";

const COMMENT_FORMAT = "shown as Markdown with every line break kept. Tables, images and HTML are shown as written, so use a list in place of a table";

const id = z.number().int().positive().describe("The todo's id, as list_todos shows it");

function withUtcTimes<T extends { completedAt: string | null; createdAt: string }>(todo: T): T {
  return { ...todo, completedAt: todo.completedAt && inUtc(todo.completedAt), createdAt: inUtc(todo.createdAt) };
}

function reply(value: unknown): CallToolResult {
  return { content: [{ type: "text", text: JSON.stringify(value) }] };
}

function refuse(value: unknown): CallToolResult {
  return { ...reply(value), isError: true };
}

async function refusingRejections(action: () => Promise<CallToolResult>) {
  try {
    return await action();
  } catch (error) {
    if (error instanceof HTTPException) return refuse(await error.getResponse().json());
    throw error;
  }
}

export function nestodoMcpServer(userId: string) {
  const server = new McpServer({ name: "nestodo", version: "0.14.1" });

  server.registerTool(
    "list_todos",
    {
      title: "List todos",
      description: `List every todo of the signed-in user in outline order. parentId is the todo it is nested under, commentCount is how many comments the member's work log on it has, which get_timeline shows, createdAt is when it was added, and completedAt is when it was completed, or null while the todo is open. Both times are in UTC as ISO 8601. Completed todos are removed from this list ${KEEP_COMPLETED_DAYS} days after they were completed, or later while a todo under them is kept, so this is not a full history of finished work. get_stats still counts them on the days they were completed.`,
      annotations: { readOnlyHint: true },
    },
    async () => reply({ todos: (await listTodos(userId)).map(withUtcTimes) }),
  );

  server.registerTool(
    "get_todo",
    {
      title: "Show a todo",
      description: "Show one todo of the signed-in user.",
      inputSchema: z.object({ id }),
      annotations: { readOnlyHint: true },
    },
    async ({ id }) => {
      const todo = await getTodo(userId, id);
      return todo ? reply({ todo: withUtcTimes(todo) }) : refuse(notFound);
    },
  );

  server.registerTool(
    "add_todo",
    {
      title: "Add a todo",
      description:
        "Add a todo, at the top level or under another todo, optionally with a first comment in its work log. Adding it under a completed todo reopens that todo and the completed todos above it, since a completed todo never has open todos under it.",
      inputSchema: createTodoBody.extend({
        title: createTodoBody.shape.title.describe(`Title, up to ${MAX_TITLE_LENGTH} characters`),
        comment: createTodoBody.shape.comment.describe(
          `Optional first comment in the todo's work log, for details that do not fit in the title, such as links, up to ${MAX_COMMENT_LENGTH} characters, ${COMMENT_FORMAT}`,
        ),
        parentId: createTodoBody.shape.parentId.describe(
          `Id of the todo to nest it under, so that it is at most ${MAX_DEPTH} levels deep, or omit it for the top level`,
        ),
        index: createTodoBody.shape.index.describe("Position among the todos beside it, from 0, or omit it to add the todo last"),
      }),
      annotations: { readOnlyHint: false, destructiveHint: false },
    },
    async (input) => refusingRejections(async () => reply({ todo: withUtcTimes(await createTodo(userId, input, "mcp")) })),
  );

  server.registerTool(
    "update_todo",
    {
      title: "Edit a todo",
      description:
        "Rename, complete, reopen, collapse or move a todo. Only the fields given change. To write down progress, use add_comment instead. A completed todo never has open todos under it, so completing a todo also completes every open todo under it and returns their ids as completedUnder, and reopening a todo, or moving an open todo into a completed todo, reopens the completed todos above it.",
      inputSchema: patchTodoBody.extend({
        id,
        title: patchTodoBody.shape.title.describe("New title"),
        completed: patchTodoBody.shape.completed.describe(
          "true to complete it and every open todo under it, false to reopen it and the completed todos above it",
        ),
        collapsed: patchTodoBody.shape.collapsed.describe("true to hide the todos under it in the outline"),
        parentId: patchTodoBody.shape.parentId.describe(
          `Id of the todo to move it under, so that it and the todos under it are at most ${MAX_DEPTH} levels deep, or null to move it to the top level`,
        ),
        index: patchTodoBody.shape.index.describe(
          "New position among the todos beside it, from 0. A todo moved under another todo without this goes last.",
        ),
      }),
      annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: true },
    },
    async ({ id, ...changes }) =>
      refusingRejections(async () => {
        const updated = await updateTodo(userId, id, changes);
        if (!updated) return refuse(notFound);
        const todo = withUtcTimes(updated.todo);
        return reply(changes.completed === true ? { todo, completedUnder: updated.completedUnder } : { todo });
      }),
  );

  server.registerTool(
    "get_timeline",
    {
      title: "Show a todo's work log",
      description:
        "Show the work log of one todo, oldest first. It lists when the todo was created, completed and reopened, and the comments written on it. via is mcp for comments an assistant added with add_comment and app for the member's own. Times are in UTC as ISO 8601, and editedAt is when a comment was last changed, or null.",
      inputSchema: z.object({ id }),
      annotations: { readOnlyHint: true },
    },
    async ({ id }) => {
      const items = await timelineOf(userId, id);
      return items ? reply({ items }) : refuse(notFound);
    },
  );

  server.registerTool(
    "add_comment",
    {
      title: "Comment on a todo",
      description:
        "Add a comment to the end of a todo's work log, such as what was done, what was found or a summary of a link. The member sees it marked as added through MCP.",
      inputSchema: z.object({
        id,
        body: commentBody.describe(`The comment, up to ${MAX_COMMENT_LENGTH} characters, ${COMMENT_FORMAT}`),
      }),
      annotations: { readOnlyHint: false, destructiveHint: false },
    },
    async ({ id, body }) => {
      const comment = await addComment(userId, id, body, "mcp");
      return comment ? reply({ comment }) : refuse(notFound);
    },
  );

  registerAppTool(
    server,
    "get_stats",
    {
      title: "Show daily stats",
      description:
        "Todos added and completed on each of the last 182 days, and todos still open at the end of each day, with days counted in the given time zone.",
      inputSchema: z.object({
        timeZone: z
          .string()
          .refine(isTimeZone, "Not an IANA time zone")
          .describe("The user's IANA time zone, such as Asia/Tokyo, which the assistant's requests do not carry"),
      }),
      annotations: { readOnlyHint: true },
      _meta: { ui: { resourceUri: STATS_APP_URI } },
    },
    async ({ timeZone }) => reply({ timeZone, days: await activityOf(userId, timeZone, new Date()) }),
  );

  registerAppResource(server, "Stats", STATS_APP_URI, {}, () => ({
    contents: [{ uri: STATS_APP_URI, mimeType: RESOURCE_MIME_TYPE, text: statsApp }],
  }));

  return server;
}
