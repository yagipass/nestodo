import { defineHandler } from "void";
import { requireUser } from "../../../../../src/access";
import { imageOf } from "../../../../../src/image-icon-service";
import { isImageIconId } from "../../../../../src/image-icons";

export const GET = defineHandler(async (c) => {
  const { email } = requireUser(c);
  const id = c.req.param("id") ?? "";
  const image = isImageIconId(id) ? await imageOf(email, id) : null;
  if (!image) return c.json({ error: "not_found", message: "Icon not found" }, 404);
  const headers = new Headers();
  image.writeHttpMetadata(headers);
  headers.set("Cache-Control", "private, max-age=31536000, immutable");
  headers.set("Content-Disposition", "attachment");
  headers.set("X-Content-Type-Options", "nosniff");
  return new Response(image.body, { headers });
});
