import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { createTodoBody } from "../../../../src/todo-schemas";
import { createTodo, listTodos } from "../../../../src/todo-service";

export const GET = defineHandler(async (c) => {
  const user = requireUser(c);
  return { todos: await listTodos(user.email) };
});

export const POST = defineHandler.withValidator({ body: createTodoBody })(async (c, { body }) => {
  const user = requireUser(c);
  return c.json({ todo: await createTodo(user.email, body) }, 201);
});
