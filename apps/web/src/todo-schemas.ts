import { z } from "zod";
import { createInsertSchema } from "void/drizzle-zod";
import { todos } from "@schema";
import { isIcon } from "./todo-icon";
import { MAX_COMMENT_LENGTH, MAX_TITLE_LENGTH } from "./todos";

function maxUtf16Length(schema: z.ZodString, name: string, limit: number) {
  return schema.refine((text) => text.length <= limit, `${name} must be ${limit} characters or fewer`).meta({ maxLength: limit });
}

const todoFields = createInsertSchema(todos, {
  title: (schema) => maxUtf16Length(schema.overwrite((title) => title.replace(/\r\n?|\n/g, " ")), "Title", MAX_TITLE_LENGTH),
  parentId: (schema) => schema.int().positive(),
  icon: (schema) => schema.refine(isIcon, "Icon must be one emoji"),
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

export const commentInput = z.object({ body: commentBody });

export const commentIdParams = z.object({ id: z.coerce.number().int().positive() });

export const searchQuery = z.object({ q: z.string().default("") });
