import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { deleteTodoQuery, patchTodoBody, todoIdParams } from "../../../../src/todo-schemas";
import { deleteTodo, getTodo, updateTodo } from "../../../../src/todo-service";

const notFound = { error: "not_found", message: "Todo not found" } as const;

export const GET = defineHandler.withValidator({ params: todoIdParams })(async (c, { params }) => {
  const user = requireUser(c);
  const todo = await getTodo(user.email, params.id);
  return todo ? c.json({ todo }) : c.json(notFound, 404);
});

export const PATCH = defineHandler.withValidator({ params: todoIdParams, body: patchTodoBody })(async (c, { params, body }) => {
  const user = requireUser(c);
  const updated = await updateTodo(user.email, params.id, body);
  return updated ? c.json({ todo: updated.todo }) : c.json(notFound, 404);
});

export const DELETE = defineHandler.withValidator({ params: todoIdParams, query: deleteTodoQuery })(async (c, { params, query }) => {
  const user = requireUser(c);
  return (await deleteTodo(user.email, params.id, query.recursive)) ? c.body(null, 204) : c.json(notFound, 404);
});
