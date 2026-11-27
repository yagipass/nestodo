import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { commentEditInput, commentIdParams } from "../../../../src/todo-schemas";
import { deleteComment, editComment } from "../../../../src/todo-service";

const notFound = { error: "not_found", message: "Comment not found" } as const;

export const PATCH = defineHandler.withValidator({ params: commentIdParams, body: commentEditInput })(async (c, { params, body }) => {
  const user = requireUser(c);
  const comment = await editComment(user.email, params.id, body.body);
  return comment ? c.json({ comment }) : c.json(notFound, 404);
});

export const DELETE = defineHandler.withValidator({ params: commentIdParams })(async (c, { params }) => {
  const user = requireUser(c);
  return (await deleteComment(user.email, params.id)) ? c.body(null, 204) : c.json(notFound, 404);
});
