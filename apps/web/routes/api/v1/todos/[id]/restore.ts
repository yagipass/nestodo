import { defineHandler } from "void";
import { requireUser } from "../../../../../src/access";
import { restoreTodoBody, todoIdParams } from "../../../../../src/todo-schemas";
import { restoreTodo } from "../../../../../src/todo-service";

export const POST = defineHandler.withValidator({ params: todoIdParams, body: restoreTodoBody })(async (c, { params, body }) => {
  const user = requireUser(c);
  const todo = await restoreTodo(user.email, params.id, body);
  return todo ? c.json({ todo }) : c.json({ error: "not_found", message: "Deleted todo not found" }, 404);
});
