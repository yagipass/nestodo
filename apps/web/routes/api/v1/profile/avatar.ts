import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { saveAvatarBody } from "../../../../src/profile-schemas";
import { profileOf, removeAvatar, saveAvatar } from "../../../../src/profile-service";

export const PUT = defineHandler.withValidator({ body: saveAvatarBody })(async (c, { body }) => {
  const { email } = requireUser(c);
  await saveAvatar(email, body.image);
  return profileOf(email);
});

export const DELETE = defineHandler(async (c) => {
  await removeAvatar(requireUser(c).email);
  return c.body(null, 204);
});
