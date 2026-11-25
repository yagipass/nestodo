import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { addImageIconBody } from "../../../../src/image-icon-schemas";
import { addImageIcon, listImageIcons } from "../../../../src/image-icon-service";

export const GET = defineHandler(async (c) => {
  return { icons: await listImageIcons(requireUser(c).email) };
});

export const POST = defineHandler.withValidator({ body: addImageIconBody })(async (c, { body }) => {
  const icon = await addImageIcon(requireUser(c).email, body.name, body.image);
  return c.json({ icon }, 201);
});
