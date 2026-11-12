import { defineHandler } from "void";
import { requireUser } from "../../../../src/access";
import { saveNameBody } from "../../../../src/profile-schemas";
import { profileOf, saveName } from "../../../../src/profile-service";

export const GET = defineHandler((c) => profileOf(requireUser(c).email));

export const PUT = defineHandler.withValidator({ body: saveNameBody })(async (c, { body }) => {
  const { email } = requireUser(c);
  await saveName(email, body.name);
  return profileOf(email);
});
