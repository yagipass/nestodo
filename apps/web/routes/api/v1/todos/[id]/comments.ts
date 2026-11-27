import { defineHandler } from "void";
import { requireUser } from "../../../../../src/access";
import { commentInput, todoIdParams } from "../../../../../src/todo-schemas";
import { addComment } from "../../../../../src/todo-service";

export const POST = defineHandler.withValidator({ params: todoIdParams, body: commentInput })(async (c, { params, body }) => {
  const user = requireUser(c);
  const comment = await addComment(user.email, params.id, body.body, "app", body.images);
  return comment ? c.json({ comment }, 201) : c.json({ error: "not_found", message: "Todo not found" }, 404);
});
