import { z } from "zod";
import { imageNameProblem, MAX_IMAGE_BYTES } from "./image-icons";
import { base64Image } from "./profile-schemas";
import { normalizeName } from "./profiles";

const imageName = z
  .string()
  .transform(normalizeName)
  .transform((name, ctx) => {
    const problem = imageNameProblem(name);
    if (problem || name === null) {
      ctx.addIssue({ code: "custom", message: problem ?? "Enter a name" });
      return z.NEVER;
    }
    return name;
  });

export const addImageIconBody = z.object({ name: imageName, image: base64Image(MAX_IMAGE_BYTES) });

export const renameImageIconBody = z.object({ name: imageName });
