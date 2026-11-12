import { z } from "zod";
import { avatarProblem, MAX_AVATAR_BYTES, nameProblem, normalizeName } from "./profiles";

const MAX_BASE64_LENGTH = Math.ceil(MAX_AVATAR_BYTES / 3) * 4;
const BASE64 = /^[A-Za-z0-9+/]*={0,2}$/;

export const saveNameBody = z.object({
  name: z
    .string()
    .transform(normalizeName)
    .superRefine((name, ctx) => {
      const problem = nameProblem(name);
      if (problem) ctx.addIssue({ code: "custom", message: problem });
    }),
});

export const saveAvatarBody = z.object({
  image: z
    .string()
    .max(MAX_BASE64_LENGTH, `The image must be ${MAX_AVATAR_BYTES / 1024} KB or smaller`)
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
      const problem = avatarProblem(bytes);
      if (problem) ctx.addIssue({ code: "custom", message: problem });
    }),
});
