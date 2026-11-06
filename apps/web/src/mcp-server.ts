import { McpServer, type CallToolResult } from "@modelcontextprotocol/server";
import { HTTPException } from "hono/http-exception";
import { z } from "zod";
import { isTimeZone } from "./stats";
import { activityOf } from "./stats-service";
import { inUtc } from "./time";
import { createTodoBody, patchTodoBody } from "./todo-schemas";
import { createTodo, getTodo, listTodos, updateTodo } from "./todo-service";
import { KEEP_COMPLETED_DAYS, MAX_DEPTH, MAX_NOTE_LENGTH, MAX_TITLE_LENGTH } from "./todos";

const notFound = { error: "not_found", message: "Todo not found" };

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
  const server = new McpServer({ name: "nestodo", version: "0.9.1" });

  server.registerTool(
    "list_todos",
    {
      title: "List todos",
      description: `List every todo of the signed-in user in outline order. parentId is the todo it is nested under, note is what the member wrote under the title, such as details or links, and is empty when there is none, createdAt is when it was added, and completedAt is when it was completed, or null while the todo is open. Both times are in UTC as ISO 8601. Completed todos are removed from this list ${KEEP_COMPLETED_DAYS} days after they were completed, or later while a todo under them is kept, so this is not a full history of finished work; get_stats still counts them on the days they were completed.`,
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
      description: "Add a todo, at the top level or under another todo, optionally with a note under its title.",
      inputSchema: createTodoBody.extend({
        title: createTodoBody.shape.title.describe(`Title, up to ${MAX_TITLE_LENGTH} characters`),
        note: createTodoBody.shape.note.describe(
          `Note shown under the title for details that do not fit in it, such as links, up to ${MAX_NOTE_LENGTH} characters; omit for none`,
        ),
        parentId: createTodoBody.shape.parentId.describe(
          `Id of the todo to nest it under, so that it is at most ${MAX_DEPTH} levels deep; omit for the top level`,
        ),
        index: createTodoBody.shape.index.describe("Position among the todos beside it, from 0; omit to add it last"),
      }),
      annotations: { readOnlyHint: false, destructiveHint: false },
    },
    async (input) => refusingRejections(async () => reply({ todo: withUtcTimes(await createTodo(userId, input)) })),
  );

  server.registerTool(
    "update_todo",
    {
      title: "Edit a todo",
      description: "Rename, complete, reopen, collapse or move a todo, or change its note. Only the fields given change.",
      inputSchema: patchTodoBody.extend({
        id,
        title: patchTodoBody.shape.title.describe("New title"),
        note: patchTodoBody.shape.note.describe("New note shown under the title, replacing the whole note; an empty string removes it"),
        completed: patchTodoBody.shape.completed.describe("true to complete it, false to reopen it"),
        collapsed: patchTodoBody.shape.collapsed.describe("true to hide the todos under it in the outline"),
        parentId: patchTodoBody.shape.parentId.describe(
          `Id of the todo to move it under, so that it and the todos under it are at most ${MAX_DEPTH} levels deep; null to move it to the top level`,
        ),
        index: patchTodoBody.shape.index.describe(
          "New position among the todos beside it, from 0; when moving it under another todo without this, it goes last",
        ),
      }),
      annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: true },
    },
    async ({ id, ...changes }) =>
      refusingRejections(async () => {
        const todo = await updateTodo(userId, id, changes);
        return todo ? reply({ todo: withUtcTimes(todo) }) : refuse(notFound);
      }),
  );

  server.registerTool(
    "get_stats",
    {
      title: "Show daily stats",
      description:
        "Todos added and completed on each of the last 182 days, and todos still open at the end of each day, with days counted in the given time zone.",
      inputSchema: z.object({
        timeZone: z
          .string()
          .refine(isTimeZone, "Not an IANA time zone")
          .describe("The user's IANA time zone, such as Asia/Tokyo; the assistant's requests do not carry it"),
      }),
      annotations: { readOnlyHint: true },
    },
    async ({ timeZone }) => reply({ timeZone, days: await activityOf(userId, timeZone, new Date()) }),
  );

  return server;
}
