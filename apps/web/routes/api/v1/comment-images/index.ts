import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { addCommentImage } from "../../../../src/comment-image-service";
import { commentImageInput } from "../../../../src/todo-schemas";

export const POST = defineHandler.withValidator({ body: commentImageInput })(async (c, { body }) => {
  const image = await addCommentImage(requireUser(c).email, body.image);
  return c.json({ image }, 201);
});
