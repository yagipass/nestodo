import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { renameImageIconBody } from "../../../../src/image-icon-schemas";
import { deleteImageIcon, renameImageIcon } from "../../../../src/image-icon-service";
import { isImageIconId } from "../../../../src/image-icons";

const notFound = { error: "not_found", message: "Icon not found" } as const;

export const PATCH = defineHandler.withValidator({ body: renameImageIconBody })(async (c, { body }) => {
  const id = c.req.param("id") ?? "";
  const icon = isImageIconId(id) ? await renameImageIcon(requireUser(c).email, id, body.name) : null;
  return icon ? c.json({ icon }) : c.json(notFound, 404);
});

export const DELETE = defineHandler(async (c) => {
  const id = c.req.param("id") ?? "";
  const deleted = isImageIconId(id) && (await deleteImageIcon(requireUser(c).email, id));
  return deleted ? c.body(null, 204) : c.json(notFound, 404);
});
