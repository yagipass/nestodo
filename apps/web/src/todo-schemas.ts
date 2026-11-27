import { z } from "zod";
import { createInsertSchema } from "void/drizzle-zod";
import { todos } from "@schema";
import { isCommentImageId, MAX_COMMENT_IMAGE_BYTES, MAX_COMMENT_IMAGES } from "./comment-images";
import { base64Image } from "./profile-schemas";
import { isTodoIcon } from "./todo-icon";
import { MAX_COMMENT_LENGTH, MAX_TITLE_LENGTH } from "./todos";

function maxUtf16Length(schema: z.ZodString, name: string, limit: number) {
  return schema.refine((text) => text.length <= limit, `${name} must be ${limit} characters or fewer`).meta({ maxLength: limit });
}

const todoFields = createInsertSchema(todos, {
  title: (schema) => maxUtf16Length(schema.overwrite((title) => title.replace(/\r\n?|\n/g, " ")), "Title", MAX_TITLE_LENGTH),
  parentId: (schema) => schema.int().positive(),
  icon: (schema) => schema.refine(isTodoIcon, "Icon must be one emoji or one of your images"),
});

const index = z.number().int().min(0);

export const commentBody = maxUtf16Length(
  z.string().refine((body) => body.trim() !== "", "Comment must not be empty"),
  "Comment",
  MAX_COMMENT_LENGTH,
);

export const createTodoBody = todoFields.pick({ title: true, parentId: true, icon: true }).extend({ index: index.optional(), comment: commentBody.optional() });

export const patchTodoBody = todoFields
  .pick({ title: true, parentId: true, icon: true, collapsed: true })
  .partial()
  .extend({ completed: z.boolean().optional(), index: index.optional() });

export const restoreTodoBody = z.object({ parentId: z.number().int().positive().nullable().optional(), index: index.optional() });

export const todoIdParams = z.object({ id: z.coerce.number().int().positive() });

export const deleteTodoQuery = z.object({ recursive: z.stringbool().default(false) });

const commentText = maxUtf16Length(z.string(), "Comment", MAX_COMMENT_LENGTH).transform((body) => (body.trim() === "" ? "" : body));

const commentImageIds = z
  .array(z.string().refine(isCommentImageId, "Image not found"))
  .max(MAX_COMMENT_IMAGES, `A comment can have up to ${MAX_COMMENT_IMAGES} images`)
  .refine((ids) => new Set(ids).size === ids.length, "Each image can be added once");

export const commentInput = z
  .object({ body: commentText.default(""), images: commentImageIds.default([]) })
  .refine((comment) => comment.body !== "" || comment.images.length > 0, { message: "Comment must not be empty", path: ["body"] });

export const commentEditInput = z.object({ body: commentText });

export const commentImageInput = z.object({ image: base64Image(MAX_COMMENT_IMAGE_BYTES) });

export const commentIdParams = z.object({ id: z.coerce.number().int().positive() });

export const searchQuery = z.object({ q: z.string().default("") });
