import { z } from "zod";
import { createInsertSchema } from "void/drizzle-zod";
import { todos } from "@schema";
import { MAX_NOTE_LENGTH, MAX_TITLE_LENGTH } from "./todos";

function maxUtf16Length(schema: z.ZodString, name: string, limit: number) {
  return schema.refine((text) => text.length <= limit, `${name} must be ${limit} characters or fewer`).meta({ maxLength: limit });
}

const todoFields = createInsertSchema(todos, {
  title: (schema) => maxUtf16Length(schema.overwrite((title) => title.replace(/\r\n?|\n/g, " ")), "Title", MAX_TITLE_LENGTH),
  note: (schema) => maxUtf16Length(schema, "Note", MAX_NOTE_LENGTH),
  parentId: (schema) => schema.int().positive(),
});

const index = z.number().int().min(0);

export const createTodoBody = todoFields.pick({ title: true, note: true, parentId: true }).extend({ index: index.optional() });

export const patchTodoBody = todoFields
  .pick({ title: true, note: true, parentId: true, collapsed: true })
  .partial()
  .extend({ completed: z.boolean().optional(), index: index.optional() });

export const todoIdParams = z.object({ id: z.coerce.number().int().positive() });

export const deleteTodoQuery = z.object({ recursive: z.stringbool().default(false) });
