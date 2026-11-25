import { z } from "zod";
import { imageProblem, MAX_AVATAR_BYTES, nameProblem, normalizeName } from "./profiles";

const BASE64 = /^[A-Za-z0-9+/]*={0,2}$/;

export function base64Image(maxBytes: number) {
  return z
    .string()
    .max(Math.ceil(maxBytes / 3) * 4, `The image must be ${maxBytes / 1024} KB or smaller`)
    .regex(BASE64, "Send the image as base64")
    .transform((value, ctx) => {
      try {
        return Uint8Array.from(atob(value), (char) => char.charCodeAt(0));
      } catch {
        ctx.addIssue({ code: "custom", message: "Send the image as base64" });
        return z.NEVER;
      }
    })
    .superRefine((bytes, ctx) => {
      const problem = imageProblem(bytes, maxBytes);
      if (problem) ctx.addIssue({ code: "custom", message: problem });
    });
}

export const saveNameBody = z.object({
  name: z
    .string()
    .transform(normalizeName)
    .superRefine((name, ctx) => {
      const problem = nameProblem(name);
      if (problem) ctx.addIssue({ code: "custom", message: problem });
    }),
});

export const saveAvatarBody = z.object({ image: base64Image(MAX_AVATAR_BYTES) });
