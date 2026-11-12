import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { avatarOf } from "../../../../src/profile-service";
import { isAvatarId } from "../../../../src/profiles";

export const GET = defineHandler(async (c) => {
  const { email } = requireUser(c);
  const id = c.req.param("id") ?? "";
  const avatar = isAvatarId(id) ? await avatarOf(email, id) : null;
  if (!avatar) return c.json({ error: "not_found", message: "Avatar not found" }, 404);
  const headers = new Headers();
  avatar.writeHttpMetadata(headers);
  headers.set("Cache-Control", "private, max-age=31536000, immutable");
  headers.set("Content-Disposition", "attachment");
  headers.set("X-Content-Type-Options", "nosniff");
  return new Response(avatar.body, { headers });
});
