import { getCookie } from "hono/cookie";
import { defineHandler, defineHead, type InferProps } from "void";
import { requireUser } from "../src/access";
import { listTodos } from "../src/todo-service";
import { SHOW_DONE_COOKIE } from "../src/todos";

export type Props = InferProps<typeof loader>;

export const loader = defineHandler(async (c) => {
  const user = requireUser(c);
  return { todos: await listTodos(user.email), showDone: getCookie(c, SHOW_DONE_COOKIE) !== "false" };
});

export const head = defineHead(() => ({ title: "Todos" }));
