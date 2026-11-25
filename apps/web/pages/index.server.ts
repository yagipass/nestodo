import { getCookie } from "hono/cookie";
import { defineHandler, defineHead, type InferProps } from "void";
import { requireUser } from "../src/access";
import { listImageIcons } from "../src/image-icon-service";
import { listTodos } from "../src/todo-service";
import { SHOW_DONE_COOKIE } from "../src/todos";

export type Props = InferProps<typeof loader>;

export const loader = defineHandler(async (c) => {
  const user = requireUser(c);
  const [todos, icons] = await Promise.all([listTodos(user.email), listImageIcons(user.email)]);
  return { todos, icons, showDone: getCookie(c, SHOW_DONE_COOKIE) !== "false" };
});

export const head = defineHead(() => ({ title: "Todos" }));
